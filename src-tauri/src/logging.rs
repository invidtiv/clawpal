use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use dirs::home_dir;

const MAX_LINES: usize = 5000;
const TRIM_TO: usize = 3000;

fn logs_dir() -> PathBuf {
    let home = home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".clawpal").join("logs");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Size threshold in bytes before we check for trimming (~500KB).
const SIZE_THRESHOLD: u64 = 500_000;

fn append_line(filename: &str, line: &str) {
    let path = logs_dir().join(filename);

    // Only check for trimming if file is large enough to warrant it
    if let Ok(metadata) = fs::metadata(&path) {
        if metadata.len() > SIZE_THRESHOLD {
            if let Ok(content) = fs::read_to_string(&path) {
                let lines: Vec<&str> = content.lines().collect();
                if lines.len() >= MAX_LINES {
                    let trimmed = lines[lines.len() - TRIM_TO..].join("\n") + "\n";
                    let _ = fs::write(&path, trimmed);
                }
            }
        }
    }

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let _ = writeln!(f, "[{ts}] {line}");
    }
}

pub fn log_info(msg: &str) {
    append_line("app.log", msg);
}

pub fn log_error(msg: &str) {
    append_line("app.log", &format!("ERROR: {msg}"));
    append_line("error.log", msg);
}

pub fn read_log_tail(filename: &str, lines: usize) -> Result<String, String> {
    // Prevent path traversal
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("Invalid filename".into());
    }
    let path = logs_dir().join(filename);
    if !path.exists() {
        return Ok(String::new());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(lines);
    Ok(all_lines[start..].join("\n"))
}
