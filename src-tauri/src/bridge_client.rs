use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signer, SigningKey};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
    MaybeTlsStream, WebSocketStream,
};

use crate::models::resolve_paths;
use crate::node_client::GatewayCredentials;

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

/// Commands that this node advertises to the gateway.
/// Must use standard OpenClaw node command names so the gateway
/// exposes them as tools to the agent.
const NODE_COMMANDS: &[&str] = &[
    "system.run",
];

/// Maximum number of pending invoke requests kept in memory.
const MAX_PENDING_INVOKES: usize = 50;

struct BridgeClientInner {
    tx: WsSink,
    req_counter: u64,
    pending: HashMap<String, oneshot::Sender<Value>>,
    challenge_nonce: Option<String>,
    node_id: String,
}

/// WebSocket-based node client that connects to the gateway with `role: "node"`.
///
/// This registers ClawPal as a node so the gateway's doctor agent can invoke
/// commands (read_file, run_command, etc.) on the local or remote machine.
/// Uses the same WebSocket port as the operator connection (18789) but with
/// a different role.
pub struct BridgeClient {
    inner: Arc<Mutex<Option<BridgeClientInner>>>,
    pending_invokes: Arc<Mutex<HashMap<String, Value>>>,
    credentials: Arc<Mutex<Option<GatewayCredentials>>>,
}

impl BridgeClient {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            pending_invokes: Arc::new(Mutex::new(HashMap::new())),
            credentials: Arc::new(Mutex::new(None)),
        }
    }

    /// Connect to the gateway as a node via WebSocket.
    /// Uses the same URL as the operator connection but with `role: "node"`.
    pub async fn connect(&self, url: &str, app: AppHandle, creds: Option<GatewayCredentials>) -> Result<(), String> {
        self.disconnect().await?;

        // Store credentials for use in handshake
        *self.credentials.lock().await = creds;

        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| format!("Node WebSocket connection failed: {e}"))?;

        let (tx, mut rx) = ws_stream.split();

        let node_id = hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "clawpal-unknown".into());

        let inner = BridgeClientInner {
            tx,
            req_counter: 0,
            pending: HashMap::new(),
            challenge_nonce: None,
            node_id: node_id.clone(),
        };

        {
            let mut guard = self.inner.lock().await;
            *guard = Some(inner);
        }

        // Spawn reader task
        let inner_ref = Arc::clone(&self.inner);
        let invokes_ref = Arc::clone(&self.pending_invokes);
        let app_clone = app.clone();

        tokio::spawn(async move {
            while let Some(msg) = rx.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(frame) = serde_json::from_str::<Value>(&text) {
                            Self::handle_frame(frame, &inner_ref, &invokes_ref, &app_clone)
                                .await;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        let _ = app_clone.emit(
                            "doctor:bridge-disconnected",
                            json!({"reason": "server closed"}),
                        );
                        let mut guard = inner_ref.lock().await;
                        *guard = None;
                        break;
                    }
                    Err(e) => {
                        let _ = app_clone.emit(
                            "doctor:error",
                            json!({"message": format!("Node WS error: {e}")}),
                        );
                        let _ = app_clone.emit(
                            "doctor:bridge-disconnected",
                            json!({"reason": format!("{e}")}),
                        );
                        let mut guard = inner_ref.lock().await;
                        *guard = None;
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Handshake: wait for connect.challenge, then send connect with role=node
        self.do_handshake(&app).await?;

        // Reject stale invokes received during handshake (from previous sessions).
        // These arrive before authentication completes, so the frontend can't reject
        // them — the gateway would ignore unauthenticated frames. Now that we're
        // authenticated, reject them so the agent session can unblock.
        let stale_invokes: Vec<(String, String)> = {
            self.pending_invokes.lock().await.drain().map(|(id, inv)| {
                let nid = inv.get("nodeId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                (id, nid)
            }).collect()
        };
        for (id, nid) in &stale_invokes {
            eprintln!("[bridge] rejecting stale invoke: {id}");
            let _ = self.send_invoke_error(id, nid, "STALE", "Node reconnected, rejecting stale invoke").await;
        }

        eprintln!("[bridge] connected as node, id={node_id}");
        let _ = app.emit("doctor:bridge-connected", json!({}));
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().await;
        if let Some(mut inner) = guard.take() {
            let _ = inner.tx.close().await;
        }
        self.pending_invokes.lock().await.clear();
        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        self.inner.lock().await.is_some()
    }

    /// Send a successful invoke result back to the gateway via `node.invoke.result`.
    /// `node_id` should be the gateway-assigned nodeId from the original invoke request.
    pub async fn send_invoke_result(&self, invoke_id: &str, node_id: &str, result: Value) -> Result<(), String> {
        eprintln!("[bridge] sending invoke result: id={invoke_id}, nodeId={node_id}, ok=true");
        self.send_request_fire("node.invoke.result", json!({
            "id": invoke_id,
            "nodeId": node_id,
            "ok": true,
            "payload": result,
        })).await
    }

    /// Send an error invoke result back to the gateway via `node.invoke.result`.
    /// `node_id` should be the gateway-assigned nodeId from the original invoke request.
    pub async fn send_invoke_error(
        &self,
        invoke_id: &str,
        node_id: &str,
        code: &str,
        message: &str,
    ) -> Result<(), String> {
        eprintln!("[bridge] sending invoke error: id={invoke_id}, nodeId={node_id}, code={code}");
        self.send_request_fire("node.invoke.result", json!({
            "id": invoke_id,
            "nodeId": node_id,
            "ok": false,
            "error": {
                "code": code,
                "message": message,
            },
        })).await
    }

    /// Take a pending invoke request by ID (removes it from the map).
    pub async fn take_invoke(&self, id: &str) -> Option<Value> {
        self.pending_invokes.lock().await.remove(id)
    }

    // ── Private helpers ──────────────────────────────────────────────

    /// Send a request and wait for the response.
    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let (id, rx) = {
            let mut guard = self.inner.lock().await;
            let inner = guard.as_mut().ok_or("Node not connected")?;
            inner.req_counter += 1;
            let id = format!("n{}", inner.req_counter);

            let (tx, rx) = oneshot::channel::<Value>();
            inner.pending.insert(id.clone(), tx);

            let frame = json!({
                "type": "req",
                "id": id,
                "method": method,
                "params": params,
            });

            if let Err(e) = inner.tx.send(Message::Text(frame.to_string())).await {
                inner.pending.remove(&id);
                return Err(format!("Failed to send node request: {e}"));
            }

            (id, rx)
        };

        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(val)) => {
                let ok = val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !ok {
                    if let Some(err) = val.get("error") {
                        Err(format!("Node connect error: {}", err))
                    } else {
                        Err("Node connect failed".into())
                    }
                } else {
                    Ok(val.get("payload").cloned().unwrap_or(Value::Null))
                }
            }
            Ok(Err(_)) => {
                let mut guard = self.inner.lock().await;
                if let Some(inner) = guard.as_mut() {
                    inner.pending.remove(&id);
                }
                Err("Connection lost during node handshake".into())
            }
            Err(_) => {
                let mut guard = self.inner.lock().await;
                if let Some(inner) = guard.as_mut() {
                    inner.pending.remove(&id);
                }
                Err("Node handshake timed out (30s)".into())
            }
        }
    }

    /// Send a request without waiting for the response.
    async fn send_request_fire(&self, method: &str, params: Value) -> Result<(), String> {
        let mut guard = self.inner.lock().await;
        let inner = guard.as_mut().ok_or("Node not connected")?;
        inner.req_counter += 1;
        let id = format!("n{}", inner.req_counter);

        let frame = json!({
            "type": "req",
            "id": id,
            "method": method,
            "params": params,
        });

        inner
            .tx
            .send(Message::Text(frame.to_string()))
            .await
            .map_err(|e| format!("Failed to send node request: {e}"))
    }

    /// Perform the connect handshake as a node.
    async fn do_handshake(&self, _app: &AppHandle) -> Result<(), String> {
        let creds = self.credentials.lock().await.clone();

        let (token, device_id, signing_key, public_key_b64) = if let Some(c) = creds {
            // Use remote gateway credentials (connecting via SSH tunnel)
            let signing_key = SigningKey::from_pkcs8_pem(&c.private_key_pem)
                .map_err(|e| format!("Failed to parse remote Ed25519 private key: {e}"))?;
            let raw_public = signing_key.verifying_key().to_bytes();
            let public_key_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw_public);
            (c.token, c.device_id, signing_key, public_key_b64)
        } else {
            // Use local credentials
            let paths = resolve_paths();
            let token = std::fs::read_to_string(&paths.config_path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|config| {
                    config.get("gateway")?
                        .get("auth")?
                        .get("token")?
                        .as_str()
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            let (device_id, signing_key, public_key_b64) =
                load_device_identity(&paths.openclaw_dir)?;
            (token, device_id, signing_key, public_key_b64)
        };

        // Wait for challenge nonce from the reader task
        let mut nonce = None;
        for _ in 0..30 {
            {
                let mut guard = self.inner.lock().await;
                if let Some(inner) = guard.as_mut() {
                    if let Some(n) = inner.challenge_nonce.take() {
                        nonce = Some(n);
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let nonce = nonce.unwrap_or_default();

        // Sign the challenge for node role
        let signed_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let signature_b64 = sign_node_challenge(
            &signing_key,
            &device_id,
            signed_at,
            &token,  // gateway auth token
            &nonce,
        );

        let version = env!("CARGO_PKG_VERSION");
        let node_id = {
            let guard = self.inner.lock().await;
            guard.as_ref().map(|i| i.node_id.clone()).unwrap_or_default()
        };

        let mut device = json!({
            "id": device_id,
            "publicKey": public_key_b64,
            "signature": signature_b64,
            "signedAt": signed_at,
        });
        if !nonce.is_empty() {
            device["nonce"] = json!(nonce);
        }

        // Send connect with role=node and wait for hello-ok
        let result = self.send_request("connect", json!({
            "minProtocol": 3,
            "maxProtocol": 3,
            "auth": { "token": token },
            "role": "node",
            "scopes": [],
            "caps": ["system"],
            "commands": NODE_COMMANDS,
            "device": device,
            "client": {
                "id": "node-host",
                "displayName": "ClawPal",
                "platform": std::env::consts::OS,
                "mode": "node",
                "version": version,
                "instanceId": node_id,
            },
        })).await?;

        let _ = result;  // handshake response consumed

        Ok(())
    }

    /// Handle a single parsed JSON frame from the gateway.
    async fn handle_frame(
        frame: Value,
        inner_ref: &Arc<Mutex<Option<BridgeClientInner>>>,
        invokes_ref: &Arc<Mutex<HashMap<String, Value>>>,
        app: &AppHandle,
    ) {
        let frame_type = frame.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match frame_type {
            "res" => {
                // Response to a request we sent (e.g. connect handshake)
                if let Some(id) = frame.get("id").and_then(|v| v.as_str()) {
                    let mut guard = inner_ref.lock().await;
                    if let Some(inner) = guard.as_mut() {
                        if let Some(sender) = inner.pending.remove(id) {
                            let _ = sender.send(frame.clone());
                        }
                    }
                }
            }
            "event" => {
                let event_name = frame.get("event").and_then(|v| v.as_str()).unwrap_or("");
                let payload = frame.get("payload").cloned().unwrap_or(Value::Null);

                match event_name {
                    "connect.challenge" => {
                        if let Some(nonce) = payload.get("nonce").and_then(|v| v.as_str()) {
                            let mut guard = inner_ref.lock().await;
                            if let Some(inner) = guard.as_mut() {
                                inner.challenge_nonce = Some(nonce.to_string());
                            }
                        }
                    }
                    "node.invoke.request" => {
                        eprintln!("[bridge] invoke request payload: {payload}");
                        // Agent wants to invoke a command on this node
                        let id = payload.get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let command = payload.get("command")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        // Capture the gateway-assigned nodeId from the request.
                        // We must echo this back in the result — using our hostname
                        // instead would cause a mismatch and the gateway would ignore the result.
                        let request_node_id = payload.get("nodeId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        // Params arrive as a JSON string in paramsJSON
                        let args = payload.get("paramsJSON")
                            .and_then(|v| v.as_str())
                            .and_then(|s| serde_json::from_str::<Value>(s).ok())
                            .or_else(|| payload.get("params").cloned())
                            .unwrap_or(Value::Null);

                        // Determine type: read-only commands vs write/exec
                        let cmd_type = if command == "system.run" {
                            // Gateway sends command as either a string or array
                            // e.g. "ls -la" or ["/bin/sh", "-lc", "ls -la"]
                            let shell_cmd = extract_shell_command(&args);
                            if shell_cmd.starts_with("cat ")
                                || shell_cmd.starts_with("ls ")
                                || shell_cmd.starts_with("head ")
                                || shell_cmd.starts_with("tail ")
                                || shell_cmd.starts_with("wc ")
                                || shell_cmd.starts_with("grep ")
                                || shell_cmd.starts_with("find ")
                                || shell_cmd.starts_with("which ")
                                || shell_cmd.starts_with("echo ")
                                || shell_cmd.starts_with("ps ")
                                || shell_cmd.starts_with("df ")
                                || shell_cmd.starts_with("free ")
                                || ["date", "uname", "uptime", "hostname"]
                                    .contains(&shell_cmd.trim())
                            {
                                "read"
                            } else {
                                "write"
                            }
                        } else {
                            "write"
                        };

                        let invoke_payload = json!({
                            "id": id,
                            "command": command,
                            "args": args,
                            "type": cmd_type,
                            "nodeId": request_node_id,
                        });

                        // Store for later approval/rejection (bounded, deduplicated)
                        let is_dup = {
                            let mut map = invokes_ref.lock().await;
                            if map.contains_key(&id) {
                                true
                            } else {
                                if map.len() >= MAX_PENDING_INVOKES {
                                    let keys: Vec<String> = map.keys().take(10).cloned().collect();
                                    for k in keys {
                                        map.remove(&k);
                                    }
                                }
                                map.insert(id.clone(), invoke_payload.clone());
                                false
                            }
                        };
                        if is_dup {
                            // Duplicate invoke — gateway sent the same request twice.
                            // Skip emitting to frontend to avoid duplicate UI entries.
                            return;
                        }

                        let _ = app.emit("doctor:invoke", invoke_payload);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Default for BridgeClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the actual shell command string from system.run args.
/// The gateway sends `command` as either:
/// - a plain string: `"ls -la"`
/// - an array: `["/bin/sh", "-lc", "ls -la"]`
/// For arrays, the last element is the actual shell command.
pub fn extract_shell_command(args: &Value) -> String {
    let cmd_val = match args.get("command") {
        Some(v) => v,
        None => return String::new(),
    };
    if let Some(s) = cmd_val.as_str() {
        return s.to_string();
    }
    if let Some(arr) = cmd_val.as_array() {
        // Last element is the actual command in ["/bin/sh", "-lc", "actual command"]
        if let Some(last) = arr.last().and_then(|v| v.as_str()) {
            return last.to_string();
        }
    }
    String::new()
}

// ── Device identity helpers ─────────────────────────────────────────

/// Load device identity from ~/.openclaw/identity/device.json.
fn load_device_identity(
    openclaw_dir: &std::path::Path,
) -> Result<(String, SigningKey, String), String> {
    let device_path = openclaw_dir.join("identity").join("device.json");
    let device_json: Value = std::fs::read_to_string(&device_path)
        .map_err(|e| format!("Failed to read device.json: {e}"))?
        .parse()
        .map_err(|e| format!("Failed to parse device.json: {e}"))?;

    let device_id = device_json
        .get("deviceId")
        .and_then(|v| v.as_str())
        .ok_or("Missing deviceId in device.json")?
        .to_string();

    let private_key_pem = device_json
        .get("privateKeyPem")
        .and_then(|v| v.as_str())
        .ok_or("Missing privateKeyPem in device.json")?;

    let signing_key = SigningKey::from_pkcs8_pem(private_key_pem)
        .map_err(|e| format!("Failed to parse Ed25519 private key: {e}"))?;

    let raw_public = signing_key.verifying_key().to_bytes();
    let public_key_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw_public);

    Ok((device_id, signing_key, public_key_b64))
}

/// Sign the challenge payload for node role.
/// Payload: `v2|<deviceId>|node-host|node|node||<signedAt>|<token>|<nonce>`
fn sign_node_challenge(
    signing_key: &SigningKey,
    device_id: &str,
    signed_at: u64,
    token: &str,
    nonce: &str,
) -> String {
    let payload = format!(
        "v2|{device_id}|node-host|node|node||{signed_at}|{token}|{nonce}"
    );
    let signature = signing_key.sign(payload.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes())
}
