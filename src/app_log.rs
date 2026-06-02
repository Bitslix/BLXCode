//! Fire-and-forget frontend helper for metadata-only app log events.

use serde_json::Value;

use crate::tauri_bridge::{app_log_event, is_tauri_shell};

pub fn info(source: &'static str, event: &'static str, metadata: Value) {
    send("info", source, event, metadata);
}

pub fn warn(source: &'static str, event: &'static str, metadata: Value) {
    send("warn", source, event, metadata);
}

pub fn error(source: &'static str, event: &'static str, metadata: Value) {
    send("error", source, event, metadata);
}

pub fn send(level: &'static str, source: &'static str, event: &'static str, metadata: Value) {
    if !is_tauri_shell() {
        return;
    }
    leptos::task::spawn_local(async move {
        let _ = app_log_event(level.into(), source.into(), event.into(), metadata).await;
    });
}
