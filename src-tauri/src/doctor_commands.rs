use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, State};

use crate::doctor_runtime_bridge::emit_runtime_event;
use crate::models::resolve_paths;
use crate::runtime::types::{RuntimeAdapter, RuntimeDomain, RuntimeEvent, RuntimeSessionKey};
use crate::runtime::zeroclaw::adapter::ZeroclawDoctorAdapter;
use crate::runtime::zeroclaw::install_adapter::ZeroclawInstallAdapter;
use crate::ssh::SshConnectionPool;

fn zeroclaw_pending_invokes() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_runtime_invoke(event: &RuntimeEvent) {
    if let RuntimeEvent::Invoke { payload } = event {
        if let Some(id) = payload.get("id").and_then(|v| v.as_str()) {
            if let Ok(mut guard) = zeroclaw_pending_invokes().lock() {
                guard.insert(id.to_string(), payload.clone());
            }
        }
    }
}

fn take_zeroclaw_invoke(invoke_id: &str) -> Option<Value> {
    if let Ok(mut guard) = zeroclaw_pending_invokes().lock() {
        return guard.remove(invoke_id);
    }
    None
}

#[tauri::command]
pub async fn doctor_connect(app: AppHandle) -> Result<(), String> {
    let _ = app.emit("doctor:connected", json!({ "engine": "zeroclaw" }));
    Ok(())
}

#[tauri::command]
pub async fn doctor_disconnect() -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn doctor_start_diagnosis(
    app: AppHandle,
    context: String,
    session_key: String,
    agent_id: String,
    instance_id: Option<String>,
) -> Result<(), String> {
    let instance = instance_id.unwrap_or_else(|| "local".to_string());
    let key = RuntimeSessionKey::new(
        "zeroclaw",
        RuntimeDomain::Doctor,
        instance,
        agent_id.clone(),
        session_key.clone(),
    );
    let adapter = ZeroclawDoctorAdapter;
    match adapter.start(&key, &context) {
        Ok(events) => {
            for ev in events {
                register_runtime_invoke(&ev);
                emit_runtime_event(&app, ev);
            }
            Ok(())
        }
        Err(e) => {
            let code = e.code.as_str();
            emit_runtime_event(&app, RuntimeEvent::Error { error: e });
            Err(format!("zeroclaw start failed [{code}]"))
        }
    }
}

#[tauri::command]
pub async fn doctor_send_message(
    app: AppHandle,
    message: String,
    session_key: String,
    agent_id: String,
    instance_id: Option<String>,
) -> Result<(), String> {
    let instance = instance_id.unwrap_or_else(|| "local".to_string());
    let key = RuntimeSessionKey::new(
        "zeroclaw",
        RuntimeDomain::Doctor,
        instance,
        agent_id.clone(),
        session_key.clone(),
    );
    let adapter = ZeroclawDoctorAdapter;
    match adapter.send(&key, &message) {
        Ok(events) => {
            for ev in events {
                register_runtime_invoke(&ev);
                emit_runtime_event(&app, ev);
            }
            Ok(())
        }
        Err(e) => {
            let code = e.code.as_str();
            emit_runtime_event(&app, RuntimeEvent::Error { error: e });
            Err(format!("zeroclaw send failed [{code}]"))
        }
    }
}

#[tauri::command]
pub async fn doctor_approve_invoke(
    app: AppHandle,
    invoke_id: String,
    target: String,
    session_key: String,
    agent_id: String,
    domain: Option<String>,
) -> Result<Value, String> {
    let invoke = take_zeroclaw_invoke(&invoke_id)
        .ok_or_else(|| format!("No pending invoke with id: {invoke_id}"))?;

    let command = invoke.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let args = invoke.get("args").cloned().unwrap_or(Value::Null);
    // Map standard node commands to internal execution.
    // Security: commands reach here only after user approval in the UI
    // (write → "Execute" button, read → "Allow" button).
    // User approval is the security boundary, not command validation.
    let result = match command {
        "clawpal" => run_clawpal_tool(&args).await?,
        "openclaw" => run_openclaw_tool(&args, &target).await?,
        _ => {
            return Err(format!(
                "unsupported tool '{command}', expected 'clawpal' or 'openclaw'"
            ))
        }
    };

    // Emit tool result first so UI can render it directly under the tool call
    // before any zeroclaw follow-up assistant message arrives.
    let _ = app.emit(
        "doctor:invoke-result",
        json!({
            "id": invoke_id,
            "result": result,
        }),
    );

    // Feed execution result back into zeroclaw session so it can continue the diagnosis.
    let command = command.to_string();
    let result_text = if let Some(stdout) = result.get("stdout").and_then(|v| v.as_str()) {
        let stderr = result.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
        let exit_code = result
            .get("exitCode")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        let mut msg = format!("[Command executed: `{command}`]\n");
        if !stdout.is_empty() {
            msg.push_str(&format!("stdout:\n```\n{stdout}\n```\n"));
        }
        if !stderr.is_empty() {
            msg.push_str(&format!("stderr:\n```\n{stderr}\n```\n"));
        }
        msg.push_str(&format!("exitCode: {exit_code}"));
        msg
    } else {
        format!("[Command executed: `{command}`]\nResult: {result}")
    };
    let is_install = domain.as_deref() == Some("install");
    let rt_domain = if is_install {
        RuntimeDomain::Install
    } else {
        RuntimeDomain::Doctor
    };
    let key = RuntimeSessionKey::new(
        "zeroclaw",
        rt_domain,
        target.clone(),
        agent_id.clone(),
        session_key.clone(),
    );
    let send_result = if is_install {
        ZeroclawInstallAdapter.send(&key, &result_text)
    } else {
        ZeroclawDoctorAdapter.send(&key, &result_text)
    };
    if let Ok(events) = send_result {
        for ev in events {
            register_runtime_invoke(&ev);
            emit_runtime_event(&app, ev);
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn doctor_reject_invoke(invoke_id: String, _reason: String) -> Result<(), String> {
    if take_zeroclaw_invoke(&invoke_id).is_some() {
        // zeroclaw local pending invoke: just drop from pending queue.
        return Ok(());
    }
    Err(format!("No pending invoke with id: {invoke_id}"))
}

#[tauri::command]
pub async fn collect_doctor_context() -> Result<String, String> {
    let paths = resolve_paths();

    let config_content = std::fs::read_to_string(&paths.config_path)
        .unwrap_or_else(|_| "(unable to read config)".into());

    let doctor_report = crate::doctor::run_doctor(&paths);

    let version = crate::cli_runner::run_openclaw(&["--version"])
        .map(|o| o.stdout)
        .unwrap_or_else(|_| "unknown".into());

    // Collect recent error log
    let error_log = crate::logging::read_log_tail("error.log", 100).unwrap_or_default();

    // Check if gateway process is running
    let gateway_running = std::process::Command::new("pgrep")
        .args(["-f", "openclaw-gateway"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let context = json!({
        "openclawVersion": version.trim(),
        "configPath": paths.config_path.to_string_lossy(),
        "configContent": config_content,
        "doctorReport": {
            "ok": doctor_report.ok,
            "score": doctor_report.score,
            "issues": doctor_report.issues.iter().map(|i| json!({
                "id": i.id,
                "severity": i.severity,
                "message": i.message,
            })).collect::<Vec<_>>(),
        },
        "gatewayProcessRunning": gateway_running,
        "errorLog": error_log,
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    });

    serde_json::to_string(&context).map_err(|e| format!("Failed to serialize context: {e}"))
}

#[tauri::command]
pub async fn collect_doctor_context_remote(
    pool: State<'_, SshConnectionPool>,
    host_id: String,
) -> Result<String, String> {
    // Collect openclaw version
    let version_result = pool
        .exec_login(&host_id, "openclaw --version 2>/dev/null || echo unknown")
        .await?;
    let version = version_result.stdout.trim().to_string();

    // Resolve config path: check OPENCLAW_STATE_DIR / OPENCLAW_HOME, fallback to ~/.openclaw
    let config_path_result = pool
        .exec_login(
            &host_id,
            "echo \"${OPENCLAW_STATE_DIR:-${OPENCLAW_HOME:-$HOME/.openclaw}}/openclaw.json\"",
        )
        .await?;
    let config_path = config_path_result.stdout.trim().to_string();
    validate_not_sensitive(&config_path)?;
    let config_content = pool
        .sftp_read(&host_id, &config_path)
        .await
        .unwrap_or_else(|_| "(unable to read remote config)".into());

    // Use `openclaw gateway status` — always returns useful text even when gateway is stopped.
    // `openclaw health --json` requires a running gateway + auth token and returns empty otherwise.
    let status_result = pool
        .exec_login(&host_id, "openclaw gateway status 2>&1")
        .await?;
    let gateway_status = status_result.stdout.trim().to_string();

    // Check if gateway process is running (reliable even when health RPC fails)
    // Bracket trick: [o]penclaw-gateway prevents pgrep from matching its own sh -c process
    let pgrep_result = pool
        .exec(&host_id, "pgrep -f '[o]penclaw-gateway' >/dev/null 2>&1")
        .await;
    let gateway_running = matches!(pgrep_result, Ok(r) if r.exit_code == 0);

    // Collect recent error log (logs live under $OPENCLAW_STATE_DIR/logs/)
    let error_log_result = pool.exec_login(&host_id,
        "tail -100 \"${OPENCLAW_STATE_DIR:-${OPENCLAW_HOME:-$HOME/.openclaw}}/logs/gateway.err.log\" 2>/dev/null || echo ''"
    ).await?;
    let error_log = error_log_result.stdout;

    // System info
    let platform_result = pool.exec(&host_id, "uname -s").await?;
    let arch_result = pool.exec(&host_id, "uname -m").await?;

    let context = json!({
        "openclawVersion": version,
        "configPath": config_path,
        "configContent": config_content,
        "gatewayStatus": gateway_status,
        "gatewayProcessRunning": gateway_running,
        "errorLog": error_log,
        "platform": platform_result.stdout.trim().to_lowercase(),
        "arch": arch_result.stdout.trim(),
        "remote": true,
        "hostId": host_id,
    });

    serde_json::to_string(&context).map_err(|e| format!("Failed to serialize context: {e}"))
}

/// Sensitive paths that are ALWAYS blocked for both read and write.
/// Checked after tilde expansion, before any other path validation.
const SENSITIVE_PATH_PATTERNS: &[&str] = &[
    "/.ssh/",
    "/.ssh",
    "/.gnupg/",
    "/.gnupg",
    "/.aws/",
    "/.aws",
    "/.config/gcloud/",
    "/.azure/",
    "/.kube/config",
    "/.docker/config.json",
    "/.netrc",
    "/.npmrc",
    "/.env",
    "/.bash_history",
    "/.zsh_history",
    "/etc/shadow",
    "/etc/sudoers",
];

fn validate_not_sensitive(path: &str) -> Result<(), String> {
    let expanded = shellexpand::tilde(path).to_string();
    for pattern in SENSITIVE_PATH_PATTERNS {
        if expanded.contains(pattern) {
            return Err(format!(
                "Access to {path} is blocked — matches sensitive path pattern: {pattern}"
            ));
        }
    }
    Ok(())
}

async fn run_clawpal_tool(args: &Value) -> Result<Value, String> {
    let raw = args
        .get("args")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if raw.is_empty() {
        return Err("clawpal: missing args".to_string());
    }
    if raw == "instance list" {
        let registry =
            clawpal_core::instance::InstanceRegistry::load().map_err(|e| e.to_string())?;
        return Ok(json!({
            "stdout": serde_json::to_string(&registry.list()).unwrap_or_else(|_| "[]".to_string()),
            "stderr": "",
            "exitCode": 0
        }));
    }
    if raw == "ssh list" {
        let hosts = clawpal_core::ssh::registry::list_ssh_hosts().map_err(|e| e.to_string())?;
        return Ok(json!({
            "stdout": serde_json::to_string(&hosts).unwrap_or_else(|_| "[]".to_string()),
            "stderr": "",
            "exitCode": 0
        }));
    }
    if raw == "profile list" {
        let openclaw = clawpal_core::openclaw::OpenclawCli::new();
        let profiles =
            clawpal_core::profile::list_profiles(&openclaw).map_err(|e| e.to_string())?;
        return Ok(json!({
            "stdout": serde_json::to_string(&profiles).unwrap_or_else(|_| "[]".to_string()),
            "stderr": "",
            "exitCode": 0
        }));
    }
    if raw.starts_with("health check") {
        let openclaw = clawpal_core::openclaw::OpenclawCli::new();
        let registry =
            clawpal_core::instance::InstanceRegistry::load().map_err(|e| e.to_string())?;
        if raw.contains("--all") {
            let mut output = Vec::new();
            for instance in registry.list() {
                let status =
                    clawpal_core::health::check_instance(&instance).map_err(|e| e.to_string())?;
                output.push(json!({"id": instance.id, "status": status}));
            }
            return Ok(json!({
                "stdout": serde_json::to_string(&output).unwrap_or_else(|_| "[]".to_string()),
                "stderr": "",
                "exitCode": 0
            }));
        }
        let target_id = raw
            .split_whitespace()
            .nth(2)
            .filter(|v| !v.is_empty() && *v != "check")
            .unwrap_or("local");
        let instance = if target_id == "local" {
            clawpal_core::instance::Instance {
                id: "local".to_string(),
                instance_type: clawpal_core::instance::InstanceType::Local,
                label: "Local".to_string(),
                openclaw_home: None,
                clawpal_data_dir: None,
                ssh_host_config: None,
            }
        } else {
            registry
                .get(target_id)
                .cloned()
                .ok_or_else(|| format!("instance '{target_id}' not found"))?
        };
        let status = clawpal_core::health::check_instance(&instance).map_err(|e| e.to_string())?;
        let _ = openclaw; // keeps symmetry with CLI execution context
        return Ok(json!({
            "stdout": serde_json::to_string(&json!({"id": instance.id, "status": status})).unwrap_or_else(|_| "{}".to_string()),
            "stderr": "",
            "exitCode": 0
        }));
    }
    Err(format!("unsupported clawpal args: {raw}"))
}

async fn run_openclaw_tool(args: &Value, target: &str) -> Result<Value, String> {
    if target != "local" {
        return Err("openclaw tool currently supports local target only".to_string());
    }
    let raw = args
        .get("args")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if raw.is_empty() {
        return Err("openclaw: missing args".to_string());
    }
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let output = clawpal_core::openclaw::OpenclawCli::new()
        .run(&parts)
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "stdout": output.stdout,
        "stderr": output.stderr,
        "exitCode": output.exit_code,
    }))
}
