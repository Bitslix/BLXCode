# Tauri IPC

BLXCode uses Tauri commands as the boundary between the Leptos frontend and native Rust backend.

## Registration Rule

Every command must be registered in `tauri::generate_handler![]` inside `src-tauri/src/lib.rs`.

If a frontend invoke fails with "command not found", first check that:

- The backend function has `#[tauri::command]`.
- The command is listed in `generate_handler![]`.
- The frontend wrapper uses the same command name.

## Frontend Wrappers

Frontend calls should go through `src/tauri_bridge.rs` rather than scattering raw `invoke()` calls across components. This keeps serialization details, command names, and result typing in one place.

## Command Groups

Current command groups include:

- App shell: `open_external_url`, `exit_app`.
- Agent: `agent_submit_turn`, `agent_submit_tool_result`, `agent_poll_events`, `agent_abort`, `agent_clear_conversation`, `agent_provider_status`.
- Agent settings: `agent_settings_get`, `agent_settings_save`, `agent_api_key_set`, `agent_api_key_delete`, `agent_provider_models`.
- Browser host: `browser_sync_bounds`, `browser_navigate`, `browser_run_js`, `browser_embedding_kind`, `browser_check_iframable`, `browser_close_tab`.
- Workspace navigation: `default_cwd`, `path_nav_exec_cmd`, `list_directory`, `create_directory`.
- PTY: `pty_spawn`, `pty_write`, `pty_resize`, `pty_kill`, `pty_drain`, `pty_peek_output`.
- Git: `git_branch`.
- Hooks: `install_agent_hooks`, `agent_hooks_status`, `uninstall_agent_hooks`.
- Workbench persistence: `workbench_save_state`, `workbench_load_state`, `workbench_sessions_path`, `workbench_load_sessions`, `workbench_drop_sessions`, `workbench_extract_sessions_prefix`, `workbench_merge_sessions_workspace`, `agent_session_exists`.
- Memory: `memory_root`, `memory_list`, `memory_read`, `memory_write`, `memory_create`, `memory_delete`, `memory_rename`, `memory_graph`, `memory_backlinks`, `memory_search`, `memory_export`, `memory_import`, `memory_install_pointers`, `memory_uninstall_pointers`, `memory_pointer_status`.
- Tasks: `tasks_list`, `tasks_get`, `tasks_create`, `tasks_update`, `tasks_delete`, `tasks_reorder`.

## Command Design Guidelines

- Prefer owned parameter types for async commands.
- Return `Result<T, String>` for fallible operations so the UI can show useful errors.
- Keep command functions thin when possible and delegate implementation to focused modules.
- Validate paths on the backend, even if the frontend already validates them.
- Avoid blocking the main thread for slow IO or network work.

## Capabilities

Tauri v2 denies capabilities by default. The current main-window capability is in `src-tauri/capabilities/default.json` and includes:

- `core:default`
- `opener:default`

If a new plugin or API needs explicit permission, update capabilities and document the change.
