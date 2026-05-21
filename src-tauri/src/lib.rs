mod agent;
mod agent_hooks;
mod agent_settings;
mod agents_layout;
mod browser_host;
mod commands;
mod fs_entries;
mod git_graph;
mod git_info;
mod gitignore;
mod image;
mod memory;
mod plans;
mod pty_host;
mod skills_rules;
mod tasks;
mod voice;
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
use image::{image_curated_models, image_settings_get, image_settings_save};
use voice::{
    voice_cancel_recording, voice_provider_voices, voice_settings_get, voice_settings_save,
    voice_start_recording, voice_stop_and_transcribe, voice_tts_preview, VoiceRecorderState,
};
use workbench_state::{
    agent_latest_session_id, agent_session_exists, workbench_clear_terminal_notifications,
    workbench_drop_sessions, workbench_extract_sessions_prefix, workbench_load_notifications,
    workbench_load_sessions, workbench_load_state, workbench_merge_sessions_workspace,
    workbench_notifications_path, workbench_prune_notifications, workbench_prune_sessions,
    workbench_save_state, workbench_sessions_path, WorkbenchSessionsFileLock,
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

/// WebKit2GTK on Linux often misbehaves with the default DMABuf path (blank window, GBM errors,
/// or flaky network/render processes). Respect an explicit env override from the user.
#[cfg(target_os = "linux")]
fn apply_linux_webkit_workarounds() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg(not(target_os = "linux"))]
fn apply_linux_webkit_workarounds() {}

fn init_keyring_store() {
    #[cfg(target_os = "windows")]
    keyring::use_windows_native_store(&std::collections::HashMap::new())
        .expect("failed to initialize Windows credential store");
    #[cfg(target_os = "macos")]
    keyring::use_apple_keychain_store(&std::collections::HashMap::new())
        .expect("failed to initialize macOS keychain store");
    #[cfg(target_os = "linux")]
    {
        if keyring::use_linux_keyutils_store(&std::collections::HashMap::new()).is_err() {
            keyring::use_dbus_secret_service_store(&std::collections::HashMap::new())
                .expect("failed to initialize Linux credential store");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    apply_linux_webkit_workarounds();
    init_keyring_store();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AgentEngineState::new())
        .manage(BrowserHost::default())
        .manage(PtyManager::default())
        .manage(VoiceRecorderState::new())
        .manage(WorkbenchSessionsFileLock::default())
        .invoke_handler(tauri::generate_handler![
            open_external_url,
            greet,
            exit_app,
            agent_submit_turn,
            agent_submit_tool_result,
            agent_poll_events,
            agent_abort,
            agent_clear_conversation,
            agent_provider_status,
            agent_read_image_file,
            agent_export_context_images,
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
            pty_drain_wait,
            pty_peek_output,
            git_branch,
            git_graph::git_is_repository,
            git_graph::git_commit_graph,
            fs_entries::list_path_entries,
            install_agent_hooks,
            agent_hooks_status,
            uninstall_agent_hooks,
            workbench_save_state,
            workbench_load_state,
            workbench_sessions_path,
            workbench_load_sessions,
            workbench_drop_sessions,
            workbench_extract_sessions_prefix,
            workbench_merge_sessions_workspace,
            workbench_notifications_path,
            workbench_load_notifications,
            workbench_clear_terminal_notifications,
            workbench_prune_notifications,
            workbench_prune_sessions,
            agent_session_exists,
            agent_latest_session_id,
            gitignore::gitignore_append_blxcode,
            memory::workspace_ensure_agents,
            memory::memory_root,
            memory::memory_list,
            memory::memory_read,
            memory::memory_write,
            memory::memory_create,
            memory::memory_list_categories,
            memory::memory_create_category,
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
            plans::plan_list,
            plans::plan_read,
            plans::plan_create,
            plans::plan_write,
            plans::plan_delete,
            plans::plan_rename,
            plans::plan_load,
            plans::plan_sync_from_tasks,
            skills_rules::commands::rules_list,
            skills_rules::commands::rules_read,
            skills_rules::commands::rules_write,
            skills_rules::commands::rules_set_enabled,
            skills_rules::commands::rules_remove,
            skills_rules::commands::skills_list,
            skills_rules::commands::skills_read,
            skills_rules::commands::skills_write,
            skills_rules::commands::skills_set_enabled,
            skills_rules::commands::skills_remove,
            skills_rules::commands::skills_install,
            skills_rules::commands::skills_rules_bootstrap,
            voice_start_recording,
            voice_stop_and_transcribe,
            voice_cancel_recording,
            voice_settings_get,
            voice_settings_save,
            voice_provider_voices,
            voice_tts_preview,
            image_settings_get,
            image_settings_save,
            image_curated_models,
            crate::image::commands::generated_image_preview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
