mod agent;
mod agent_hooks;
mod browser_host;
mod commands;
mod git_info;
mod pty_host;
mod workbench_state;

use agent::AgentEngineState;
use agent_hooks::{agent_hooks_status, install_agent_hooks, uninstall_agent_hooks};
use browser_host::BrowserHost;
use commands::*;
use pty_host::PtyManager;
use workbench_state::{
    agent_session_exists, workbench_drop_sessions, workbench_load_sessions, workbench_load_state,
    workbench_save_state, workbench_sessions_path,
};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
fn open_external_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AgentEngineState::new())
        .manage(BrowserHost::default())
        .manage(PtyManager::default())
        .invoke_handler(tauri::generate_handler![
            open_external_url,
            greet,
            exit_app,
            agent_submit_turn,
            agent_poll_events,
            agent_abort,
            agent_provider_status,
            browser_sync_bounds,
            browser_navigate,
            browser_run_js,
            browser_embedding_kind,
            browser_check_iframable,
            browser_close_tab,
            path_nav_exec_cmd,
            pty_spawn,
            pty_write,
            pty_resize,
            pty_kill,
            pty_drain,
            git_branch,
            install_agent_hooks,
            agent_hooks_status,
            uninstall_agent_hooks,
            workbench_save_state,
            workbench_load_state,
            workbench_sessions_path,
            workbench_load_sessions,
            workbench_drop_sessions,
            agent_session_exists,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
