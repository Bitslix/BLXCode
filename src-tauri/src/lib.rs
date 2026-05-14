mod agent;
mod agent_hooks;
mod agent_settings;
mod browser_host;
mod commands;
mod git_info;
mod memory;
mod pty_host;
mod tasks;
mod workbench_state;

use agent::AgentEngineState;
use agent_hooks::{agent_hooks_status, install_agent_hooks, uninstall_agent_hooks};
use agent_settings::{
    agent_api_key_delete, agent_api_key_set, agent_provider_models, agent_settings_get,
    agent_settings_save,
};
use browser_host::BrowserHost;
use commands::*;
use pty_host::PtyManager;
use tauri_plugin_opener::OpenerExt;
use workbench_state::{
    agent_session_exists, workbench_drop_sessions, workbench_load_sessions, workbench_load_state,
    workbench_save_state, workbench_sessions_path,
};

#[tauri::command]
fn open_external_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
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
            agent_submit_tool_result,
            agent_poll_events,
            agent_abort,
            agent_provider_status,
            harness_ensure_default_sandbox,
            agent_settings_get,
            agent_settings_save,
            agent_api_key_set,
            agent_api_key_delete,
            agent_provider_models,
            browser_sync_bounds,
            browser_navigate,
            browser_run_js,
            browser_embedding_kind,
            browser_check_iframable,
            browser_close_tab,
            path_nav_exec_cmd,
            list_directory,
            create_directory,
            default_cwd,
            pty_spawn,
            pty_write,
            pty_resize,
            pty_kill,
            pty_drain,
            pty_peek_output,
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
            memory::memory_root,
            memory::memory_list,
            memory::memory_read,
            memory::memory_write,
            memory::memory_create,
            memory::memory_delete,
            memory::memory_rename,
            memory::memory_graph,
            memory::memory_backlinks,
            memory::memory_search,
            memory::memory_export,
            memory::memory_import,
            memory::memory_install_pointers,
            memory::memory_uninstall_pointers,
            memory::memory_pointer_status,
            tasks::tasks_list,
            tasks::tasks_get,
            tasks::tasks_create,
            tasks::tasks_update,
            tasks::tasks_delete,
            tasks::tasks_reorder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
