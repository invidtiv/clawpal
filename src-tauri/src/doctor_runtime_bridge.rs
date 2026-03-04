use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::runtime::types::RuntimeEvent;

pub fn map_runtime_event_name(event: &RuntimeEvent) -> &'static str {
    match event {
        RuntimeEvent::ChatDelta { .. } => "doctor:chat-delta",
        RuntimeEvent::ChatFinal { .. } => "doctor:chat-final",
        RuntimeEvent::Invoke { .. } => "doctor:invoke",
        RuntimeEvent::DiagnosisReport { .. } => "doctor:diagnosis-report",
        RuntimeEvent::Error { .. } => "doctor:error",
        RuntimeEvent::Status { .. } => "doctor:status",
    }
}

pub fn emit_runtime_event(app: &AppHandle, event: RuntimeEvent) {
    let name = map_runtime_event_name(&event);
    match event {
        RuntimeEvent::ChatDelta { text } => {
            crate::commands::logs::log_dev(format!(
                "[dev][doctor_runtime_bridge] emit event={} kind=chat-delta text_len={}",
                name,
                text.len()
            ));
            let _ = app.emit(name, json!({ "text": text }));
        }
        RuntimeEvent::ChatFinal { text } => {
            let preview: String = text.chars().take(120).collect();
            let preview = preview.replace('\n', "\\n");
            crate::commands::logs::log_dev(format!(
                "[dev][doctor_runtime_bridge] emit event={} kind=chat-final text_len={} preview={}",
                name,
                text.len(),
                preview
            ));
            let _ = app.emit(name, json!({ "text": text }));
        }
        RuntimeEvent::Invoke { payload } => {
            crate::commands::logs::log_dev(format!(
                "[dev][doctor_runtime_bridge] emit event={} kind=invoke",
                name
            ));
            let _ = app.emit(name, payload);
        }
        RuntimeEvent::Error { error } => {
            crate::commands::logs::log_dev(format!(
                "[dev][doctor_runtime_bridge] emit event={} kind=error code={}",
                name,
                error.code.as_str()
            ));
            let _ = app.emit(
                name,
                json!({
                    "code": error.code.as_str(),
                    "message": error.message,
                    "actionHint": error.action_hint,
                }),
            );
        }
        RuntimeEvent::DiagnosisReport { items } => {
            crate::commands::logs::log_dev(format!(
                "[dev][doctor_runtime_bridge] emit event={} kind=diagnosis-report items={}",
                name,
                items.as_array().map(|arr| arr.len()).unwrap_or(0)
            ));
            let _ = app.emit(name, json!({ "items": items }));
        }
        RuntimeEvent::Status { text } => {
            crate::commands::logs::log_dev(format!(
                "[dev][doctor_runtime_bridge] emit event={} kind=status text_len={}",
                name,
                text.len()
            ));
            let _ = app.emit(name, json!({ "text": text }));
        }
    }
}
