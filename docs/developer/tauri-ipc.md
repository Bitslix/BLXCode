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

Authoritative list from `src-tauri/src/lib.rs` (grouped for navigation):

### App shell

- `open_external_url`, `greet`, `exit_app`

### Agent runtime

- `agent_submit_turn`, `agent_submit_tool_result`, `agent_poll_events`, `agent_abort`, `agent_clear_conversation`, `agent_provider_status`
- `agent_read_image_file`, `agent_export_context_images`

### Agent settings and keys

- `agent_settings_get`, `agent_settings_save`, `agent_provider_models`
- `api_keys_status`, `api_keys_apply` — centralized key catalog (LLM, Tavily, Brave, fal.ai, AWS Polly); see `src-tauri/src/api_keys.rs`
- `agent_web_settings_get`, `agent_web_settings_save` (backend choice only; keys via `api_keys_apply`)
- `agent_environment_invalidate` — clears `environment_detect` session cache (also triggered from UI on workspace switch)
- `harness_ensure_default_sandbox`, `harness_user_home_dir`

### Image generation

- `image_settings_get`, `image_settings_save`, `image_curated_models`, `generated_image_preview`

### Browser host

- `browser_sync_bounds`, `browser_navigate`, `browser_run_js`, `browser_embedding_kind`, `browser_check_iframable`, `browser_close_tab`

### Workspace paths

- `default_cwd`, `path_nav_exec_cmd`, `list_directory`, `create_directory`
- `gitignore_append_blxcode`

### PTY

- `pty_spawn`, `pty_write`, `pty_resize`, `pty_kill`, `pty_drain`, `pty_drain_wait`, `pty_peek_output`

### Git and explorer

- `git_branch`
- `git_is_repository`, `git_commit_graph` (`git_graph` module)
- `list_path_entries` (`fs_entries` module)

### Hooks

- `install_agent_hooks`, `agent_hooks_status`, `uninstall_agent_hooks`

### Workbench persistence

- `workbench_save_state`, `workbench_load_state`
- `workbench_sessions_path`, `workbench_load_sessions`, `workbench_drop_sessions`, `workbench_extract_sessions_prefix`, `workbench_merge_sessions_workspace`, `workbench_prune_sessions`
- `workbench_notifications_path`, `workbench_load_notifications`, `workbench_clear_terminal_notifications`, `workbench_prune_notifications`
- `agent_session_exists`, `agent_latest_session_id`

### Workspace bootstrap and memory

- `workspace_ensure_agents` — `.agents/memory`, `.agents/learnings`, `.agents/plans`, migration, wikilink upgrade
- `memory_root`, `memory_list`, `memory_read`, `memory_write`, `memory_create`
- `memory_list_categories`, `memory_create_category`
- `memory_delete`, `memory_rename`, `memory_graph`, `memory_backlinks`, `memory_search`
- `memory_export`, `memory_import`
- `memory_install_pointers`, `memory_uninstall_pointers`, `memory_pointer_status`

### Tasks and plans

- `tasks_list`, `tasks_get`, `tasks_create`, `tasks_update`, `tasks_delete`, `tasks_reorder`
- `plan_list`, `plan_read`, `plan_create`, `plan_write`, `plan_delete`, `plan_rename`, `plan_load`, `plan_sync_from_tasks`

### Skills and rules

- `rules_list`, `rules_read`, `rules_write`, `rules_set_enabled`, `rules_remove`
- `skills_list`, `skills_read`, `skills_write`, `skills_set_enabled`, `skills_remove`, `skills_install`
- `skills_rules_bootstrap`

### Voice

- `voice_start_recording`, `voice_stop_and_transcribe`, `voice_cancel_recording`
- `voice_settings_get`, `voice_settings_save`, `voice_tts_preview`

Server-side agent tools (`environment_detect`, `shell_exec`, `git_*`, `web_*`, `subagents.run`, …) run inside provider/subagent HTTP loops, not as separate Tauri commands. See [Agent Harness](agent-harness.md) and [Subagents](subagents.md).

Client-side agent tools (for example `memory_context_attach`, `plan_context_attach`, `harness.send_agent_context`) are registered in `src-tauri/src/agent/tools.rs` with site `client` and do not appear as Tauri commands.

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

## See also

- [Architecture](architecture.md) — subsystem diagrams and module layout
- [Contributing](contributing.md) — register new commands in `lib.rs` and `tauri_bridge.rs`
