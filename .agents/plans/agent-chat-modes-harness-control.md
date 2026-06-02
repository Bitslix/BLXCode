# BLXCode Agent Chat Modes & Harness Control

## Summary

Extend BLXCode Agent so each Agent Chat session has a mode toolbar above the input and the harness can safely control the app where useful: files/folders, commands, settings, window state, workspace switching, visible panels, tabs, memory/plans/rules/skills views, terminals, and browser.

## Decisions

- New chat sessions default to `Ask Edits`.
- `Allow all` executes everything without prompts, including Bash/PowerShell/CMD.
- `Plan` is non-mutating: read/search/analyze/plan only; block edits, settings changes, window changes, workspace mutations, terminal key submits, and write-capable commands.
- Mode is per Agent Chat session/workspace, not a global app setting; clearing/resetting chat returns to `Ask Edits`.
- App-control tools should be client-side harness tools when they manipulate Leptos `WorkbenchService` state.

## Implementation Notes

- Add `AgentChatMode` to frontend/backend wire types and `UserTurn`; persist `agent_chat_mode` on `WorkspaceEntry`.
- Add segmented toolbar above `.agent-compose`: `Ask Edits`, `Allow all`, `Plan`; disabled while busy.
- Add central permission classification before server and client tool execution: read, mutating edit, command, settings/window, navigation/view.
- Add permission UI rows for `Ask Edits`; show exact command text for `shell_exec` and terminal-submit actions.
- Add workspace file/folder tools using existing `fs_entries` guards: `workspace_file_write`, `workspace_file_delete`, `workspace_dir_create`, `workspace_entry_rename`.
- Add app navigation/client tools: `harness.workspace_list`, `harness.workspace_switch`, `harness.workspace_prev`, `harness.workspace_next`.
- Add view-control/client tools: `harness.view_show` with targets `agent`, `browser`, `plans`, `memory`, `rules`, `skills`, `settings`, `terminals`, `project_files`, `git_diff`, `git_graph`.
- Add focused open tools: `harness.open_settings { category }`, `harness.open_memory { path? }`, `harness.open_plan { path? }`, `harness.open_file { path }`, `harness.open_diff { path, staged? }`.
- Add window tools: `harness.window_get_state`, `harness.window_set_size`, `harness.window_set_fullscreen`.
- Update `system_prompt.rs`, `harness_skills/file-access.md`, `shell.md`, `harness.md`, and user/developer docs with mode behavior and new tools.
- Add i18n labels and timeline summaries for all new tool calls.

## Tests

- Unit-test permission classification for every mutating/read/navigation tool and every mode.
- Test `Plan` blocks writes, commands with writes, terminal submit, window/settings changes, and workspace switches.
- Test `Allow all` bypasses prompts.
- Test `Ask Edits` asks before file/folder changes and command execution.
- Test workspace switching by id/title/cwd and prev/next wrap behavior.
- Test view tools open right-panel tabs, center tabs, sidebar sections, memory note focus, settings category, files, diffs, and terminals.
- Manual Tauri smoke test for toolbar persistence, reset behavior, permission cards, and app/window control.

## Tasks

- [x] `chat-mode-wire` - Add `AgentChatMode` to wire types, backend protocol, workspace state, and reset behavior
- [x] `mode-toolbar-ui` - Add the per-session mode toolbar above the Agent composer
- [x] `permission-gate` - Implement central tool permission classification and approval flow
- [x] `workspace-file-tools` - Add sandboxed workspace file/folder create/write/delete/rename tools
- [x] `workspace-switch-tools` - Add workspace list, switch, previous, and next harness tools
- [x] `view-control-tools` - Add harness tools for right panel, center tabs, sidebar sections, memory/plans/settings/files/diffs
- [x] `window-settings-tools` - Add controlled window and safe settings tools
- [x] `prompt-skills-docs` - Update system prompt, core skills, i18n labels, and docs
- [x] `tests-smoke` - Add automated tests and run manual Tauri smoke checks
