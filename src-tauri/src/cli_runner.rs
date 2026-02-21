use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::models::resolve_paths;
use crate::ssh::SshConnectionPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub fn run_openclaw(args: &[&str]) -> Result<CliOutput, String> {
    run_openclaw_with_env(args, None)
}

pub fn run_openclaw_with_env(
    args: &[&str],
    env: Option<&HashMap<String, String>>,
) -> Result<CliOutput, String> {
    let mut cmd = Command::new("openclaw");
    cmd.args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if let Some(env_vars) = env {
        for (k, v) in env_vars {
            cmd.env(k, v);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| format!("failed to run openclaw: {e}"))?;

    let exit_code = output.status.code().unwrap_or(-1);
    Ok(CliOutput {
        stdout: String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string(),
        stderr: String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_string(),
        exit_code,
    })
}

pub async fn run_openclaw_remote(
    pool: &SshConnectionPool,
    host_id: &str,
    args: &[&str],
) -> Result<CliOutput, String> {
    run_openclaw_remote_with_env(pool, host_id, args, None).await
}

pub async fn run_openclaw_remote_with_env(
    pool: &SshConnectionPool,
    host_id: &str,
    args: &[&str],
    env: Option<&HashMap<String, String>>,
) -> Result<CliOutput, String> {
    let mut cmd_str = String::new();

    if let Some(env_vars) = env {
        for (k, v) in env_vars {
            cmd_str.push_str(&format!("{}='{}' ", k, v.replace('\'', "'\\''")));
        }
    }

    cmd_str.push_str("openclaw");
    for arg in args {
        cmd_str.push_str(&format!(" '{}'", arg.replace('\'', "'\\''")));
    }

    let result = pool.exec_login(host_id, &cmd_str).await?;
    Ok(CliOutput {
        stdout: result.stdout,
        stderr: result.stderr,
        exit_code: result.exit_code as i32,
    })
}

pub fn parse_json_output(output: &CliOutput) -> Result<Value, String> {
    if output.exit_code != 0 {
        let details = if !output.stderr.is_empty() {
            &output.stderr
        } else {
            &output.stdout
        };
        return Err(format!(
            "openclaw command failed ({}): {}",
            output.exit_code, details
        ));
    }

    let raw = &output.stdout;
    let start = raw
        .find('{')
        .or_else(|| raw.find('['))
        .ok_or_else(|| format!("No JSON found in output: {raw}"))?;
    let json_str = &raw[start..];
    serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {e}"))
}

// ---------------------------------------------------------------------------
// CommandQueue — Task 2
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingCommand {
    pub id: String,
    pub label: String,
    pub command: Vec<String>,
    pub created_at: String,
}

pub struct CommandQueue {
    commands: Mutex<Vec<PendingCommand>>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            commands: Mutex::new(Vec::new()),
        }
    }

    pub fn enqueue(&self, label: String, command: Vec<String>) -> PendingCommand {
        let cmd = PendingCommand {
            id: Uuid::new_v4().to_string(),
            label,
            command,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.commands.lock().unwrap().push(cmd.clone());
        cmd
    }

    pub fn remove(&self, id: &str) -> bool {
        let mut cmds = self.commands.lock().unwrap();
        let before = cmds.len();
        cmds.retain(|c| c.id != id);
        cmds.len() < before
    }

    pub fn list(&self) -> Vec<PendingCommand> {
        self.commands.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.commands.lock().unwrap().clear();
    }

    pub fn is_empty(&self) -> bool {
        self.commands.lock().unwrap().is_empty()
    }

    pub fn len(&self) -> usize {
        self.commands.lock().unwrap().len()
    }
}

impl Default for CommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tauri commands — Task 3
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn queue_command(
    queue: tauri::State<CommandQueue>,
    label: String,
    command: Vec<String>,
) -> Result<PendingCommand, String> {
    if command.is_empty() {
        return Err("command cannot be empty".into());
    }
    Ok(queue.enqueue(label, command))
}

#[tauri::command]
pub fn remove_queued_command(
    queue: tauri::State<CommandQueue>,
    id: String,
) -> Result<bool, String> {
    Ok(queue.remove(&id))
}

#[tauri::command]
pub fn list_queued_commands(
    queue: tauri::State<CommandQueue>,
) -> Result<Vec<PendingCommand>, String> {
    Ok(queue.list())
}

#[tauri::command]
pub fn discard_queued_commands(
    queue: tauri::State<CommandQueue>,
) -> Result<bool, String> {
    queue.clear();
    Ok(true)
}

#[tauri::command]
pub fn queued_commands_count(
    queue: tauri::State<CommandQueue>,
) -> Result<usize, String> {
    Ok(queue.len())
}

// ---------------------------------------------------------------------------
// Preview — sandbox execution with OPENCLAW_HOME
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewQueueResult {
    pub commands: Vec<PendingCommand>,
    pub config_before: String,
    pub config_after: String,
    pub errors: Vec<String>,
}

#[tauri::command]
pub fn preview_queued_commands(
    queue: tauri::State<CommandQueue>,
) -> Result<PreviewQueueResult, String> {
    let commands = queue.list();
    if commands.is_empty() {
        return Err("No pending commands to preview".into());
    }

    let paths = resolve_paths();

    // Read current config
    let config_before = crate::config_io::read_text(&paths.config_path)?;

    // Set up sandbox directory
    let preview_dir = paths.clawpal_dir.join("preview").join(".openclaw");
    std::fs::create_dir_all(&preview_dir).map_err(|e| e.to_string())?;

    // Copy current config to sandbox
    let preview_config = preview_dir.join("openclaw.json");
    std::fs::copy(&paths.config_path, &preview_config).map_err(|e| e.to_string())?;

    let mut env = HashMap::new();
    env.insert(
        "OPENCLAW_HOME".to_string(),
        preview_dir.to_string_lossy().to_string(),
    );

    // Execute each command in sandbox
    let mut errors = Vec::new();
    for cmd in &commands {
        let args: Vec<&str> = cmd.command.iter().skip(1).map(|s| s.as_str()).collect();
        let result = run_openclaw_with_env(&args, Some(&env));
        match result {
            Ok(output) if output.exit_code != 0 => {
                let detail = if !output.stderr.is_empty() {
                    output.stderr.clone()
                } else {
                    output.stdout.clone()
                };
                errors.push(format!("{}: {}", cmd.label, detail));
                break;
            }
            Err(e) => {
                errors.push(format!("{}: {}", cmd.label, e));
                break;
            }
            _ => {}
        }
    }

    // Read result config from sandbox
    let config_after = if errors.is_empty() {
        crate::config_io::read_text(&preview_config)?
    } else {
        config_before.clone()
    };

    // Cleanup sandbox
    let _ = std::fs::remove_dir_all(paths.clawpal_dir.join("preview"));

    Ok(PreviewQueueResult {
        commands,
        config_before,
        config_after,
        errors,
    })
}

// ---------------------------------------------------------------------------
// Apply — execute queue for real, rollback on failure
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyQueueResult {
    pub ok: bool,
    pub applied_count: usize,
    pub total_count: usize,
    pub error: Option<String>,
    pub rolled_back: bool,
}

#[tauri::command]
pub fn apply_queued_commands(
    queue: tauri::State<CommandQueue>,
) -> Result<ApplyQueueResult, String> {
    let commands = queue.list();
    if commands.is_empty() {
        return Err("No pending commands to apply".into());
    }

    let paths = resolve_paths();
    let total_count = commands.len();

    // Save snapshot before applying (for rollback)
    let config_before = crate::config_io::read_text(&paths.config_path)?;
    let _ = crate::history::add_snapshot(
        &paths.history_dir,
        &paths.metadata_path,
        Some("pre-apply".to_string()),
        "queue-apply",
        true,
        &config_before,
        None,
    );

    // Execute each command for real
    let mut applied_count = 0;
    for cmd in &commands {
        let args: Vec<&str> = cmd.command.iter().skip(1).map(|s| s.as_str()).collect();
        let result = run_openclaw(&args);
        match result {
            Ok(output) if output.exit_code != 0 => {
                let detail = if !output.stderr.is_empty() {
                    output.stderr.clone()
                } else {
                    output.stdout.clone()
                };

                // Rollback: restore config from snapshot
                let _ = crate::config_io::write_text(&paths.config_path, &config_before);

                queue.clear();
                return Ok(ApplyQueueResult {
                    ok: false,
                    applied_count,
                    total_count,
                    error: Some(format!(
                        "Step {} failed ({}): {}",
                        applied_count + 1,
                        cmd.label,
                        detail
                    )),
                    rolled_back: true,
                });
            }
            Err(e) => {
                let _ = crate::config_io::write_text(&paths.config_path, &config_before);
                queue.clear();
                return Ok(ApplyQueueResult {
                    ok: false,
                    applied_count,
                    total_count,
                    error: Some(format!(
                        "Step {} failed ({}): {}",
                        applied_count + 1,
                        cmd.label,
                        e
                    )),
                    rolled_back: true,
                });
            }
            Ok(_) => {
                applied_count += 1;
            }
        }
    }

    // All succeeded — clear queue and restart gateway
    queue.clear();

    // Restart gateway (best effort, don't fail the whole apply)
    let gateway_result = run_openclaw(&["gateway", "restart"]);
    if let Err(e) = &gateway_result {
        eprintln!("Warning: gateway restart failed after apply: {e}");
    }

    Ok(ApplyQueueResult {
        ok: true,
        applied_count,
        total_count,
        error: None,
        rolled_back: false,
    })
}
