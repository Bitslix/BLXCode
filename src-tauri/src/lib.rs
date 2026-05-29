mod agent;
mod agent_hooks;
mod agent_settings;
mod agents_layout;
mod api_keys;
mod app_paths;
mod browser_host;
mod clipboard;
mod commands;
mod fs_entries;
mod git_commit_ai;
mod git_graph;
mod git_info;
mod git_status;
mod git_sync;
mod image;
mod media_keys;
mod memory;
mod plans;
mod plans_index;
mod pointers;
mod pty_host;
mod skills_rules;
mod tasks;
mod updater;
mod voice;
mod workbench_state;

use agent::{
    agent_environment_invalidate, agent_web_settings_get, agent_web_settings_save, AgentEngineState,
};
use agent_hooks::{agent_hooks_status, install_agent_hooks, uninstall_agent_hooks};
use agent_settings::{agent_provider_models, agent_settings_get, agent_settings_save};
use api_keys::{api_keys_apply, api_keys_status};
use browser_host::BrowserHost;
use clipboard::{clipboard_read_text, clipboard_write_text};
use commands::*;
use image::{image_curated_models, image_settings_get, image_settings_save};
use pty_host::PtyManager;
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use updater::{
    app_relaunch, app_version, post_update_release_notes, updater_check, updater_install_start,
    updater_poll_progress, BlxUpdaterState,
};
use voice::{
    voice_cancel_recording, voice_settings_get, voice_settings_save, voice_start_recording,
    voice_stop_and_transcribe, voice_tts_preview, VoiceRecorderState,
};
use workbench_state::{
    agent_latest_session_id, agent_session_exists, workbench_clear_terminal_notifications,
    workbench_drop_sessions, workbench_extract_sessions_prefix, workbench_load_notifications,
    workbench_load_sessions, workbench_load_state, workbench_merge_sessions_workspace,
    workbench_notifications_path, workbench_prune_notifications, workbench_prune_sessions,
    workbench_rewrite_terminal_keys, workbench_save_state, workbench_sessions_path,
    WorkbenchSessionsFileLock,
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
fn frontend_console_log(level: String, message: String) {
    eprintln!("[frontend:{level}] {message}");
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
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app data dir unavailable: {e}"))?;
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("create app data dir {}: {e}", dir.display()))?;
            app_paths::init(dir);
            Ok(())
        })
        .manage(AgentEngineState::new())
        .manage(BlxUpdaterState::default())
        .manage(BrowserHost::default())
        .manage(git_status::GitWatcherState::default())
        .manage(PtyManager::default())
        .manage(VoiceRecorderState::new())
        .manage(WorkbenchSessionsFileLock::default())
        .invoke_handler(tauri::generate_handler![
            open_external_url,
            greet,
            frontend_console_log,
            exit_app,
            app_version,
            updater_check,
            updater_install_start,
            updater_poll_progress,
            app_relaunch,
            post_update_release_notes,
            agent_submit_turn,
            agent_submit_tool_result,
            agent_poll_events,
            agent_abort,
            agent_clear_conversation,
            agent_provider_status,
            agent_read_image_file,
            agent_export_context_images,
            harness_ensure_default_sandbox,
            harness_user_home_dir,
            agent_settings_get,
            agent_settings_save,
            agent_provider_models,
            api_keys_status,
            api_keys_apply,
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
            git_status::git_status_changes,
            git_status::git_file_diff,
            git_status::git_stage_file,
            git_status::git_unstage_file,
            git_status::git_stage_all,
            git_status::git_unstage_all,
            git_status::git_commit,
            git_commit_ai::git_generate_commit_message,
            git_status::git_status_watch_start,
            git_status::git_status_watch_stop,
            git_sync::git_sync_status,
            git_sync::git_fetch,
            git_sync::git_pull,
            git_sync::git_push,
            fs_entries::list_path_entries,
            fs_entries::create_workspace_file,
            fs_entries::create_workspace_dir,
            fs_entries::read_workspace_text_file,
            fs_entries::stat_workspace_file,
            fs_entries::read_workspace_image_file,
            fs_entries::read_workspace_video_file,
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
            workbench_rewrite_terminal_keys,
            agent_session_exists,
            agent_latest_session_id,
            memory::workspace_ensure_agents,
            memory::memory_root,
            memory::memory_status,
            memory::memory_bootstrap,
            memory::memory_rebuild_architecture,
            memory::memory_lint_architecture,
            memory::memory_list,
            memory::memory_read,
            memory::memory_write,
            memory::memory_create,
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
            skills_rules::commands::rules_pointer_status,
            skills_rules::commands::rules_install_pointers,
            skills_rules::commands::rules_uninstall_pointers,
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
            voice_tts_preview,
            image_settings_get,
            image_settings_save,
            agent_web_settings_get,
            agent_web_settings_save,
            agent_environment_invalidate,
            image_curated_models,
            crate::image::commands::generated_image_preview,
            clipboard_read_text,
            clipboard_write_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
