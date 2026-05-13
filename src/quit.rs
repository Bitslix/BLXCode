//! Beenden der Anwendung (Tauri `invoke` oder Fallback `window.close`).
use crate::tauri_bridge::{exit_app_ipc, is_tauri_shell};

pub fn request_app_quit() {
    leptos::task::spawn_local(async move {
        if is_tauri_shell() {
            let _ = exit_app_ipc().await;
            return;
        }
        if let Some(w) = web_sys::window() {
            let _ = w.close();
        }
    });
}
