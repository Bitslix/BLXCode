# BLXCode Plugin Packages And Run Menu

## Summary

Add a real BLXCode plugin package system, a new **Settings -> Plugins** tab, and a titlebar **Run** menu after Worktree. Run plugins are installable packages that contribute command detectors for common project ecosystems. Selecting a command always starts it in a **new visible terminal slot** in the active workspace.

## Decisions

- Use the active `tauri-v2`, `rust-best-practices`, `rust-async-patterns`, `rust-testing`, `leptos-guide`, and `frontend-design` skills during implementation.
- Commit after each completed task, with focused commits that match the task boundary.
- v1 plugin packages are declarative for safety; no WASM/native plugin execution.
- v1 install source is a GitHub URL to a plugin package directory.
- Plugin management is app-global, not per-workspace, while run discovery is scoped to the active workspace.
- Run commands execute only in a new terminal slot.
- Initial run/dev/debug/test/build plugins use category `runtime`.
- No hardcoded user-facing UI text; all visible text goes through `I18nKey`.
- CSS must use theme tokens only so all existing themes remain supported.

## Implementation Notes

- Add `src-tauri/src/plugins/` with manifest/category/registry types, app-data store, GitHub install flow, install progress, declarative run detector parsing, and Tauri command handlers.
- Add Tauri commands `plugins_list`, `plugins_install_from_github`, `plugins_install_progress`, `plugins_set_enabled`, `plugins_remove`, and `run_commands_discover`; register them in `src-tauri/src/lib.rs` and expose typed wrappers in `src/tauri_bridge.rs`.
- Add built-in non-removable runtime plugins for Node/npm/pnpm/bun/yarn, Rust/Cargo, Go, C/C++/CMake/Make, Shell/PowerShell/Bat/Cmd, direct JS/TS/Node/Bun/Electron, and conservative common-language detectors.
- Add `PluginsSettingsPane` under `src/workbench/plugins_settings_pane/`, `HarnessSettingsCategory::Plugins`, SettingsDock category button, install dialog, category filters, search, enable/disable/remove actions, and theme-token CSS.
- Add `RunMenu` under `src/workbench/app_titlebar/` after `WorktreeMenu`; command click appends a plain terminal slot, focuses the Terminals tab, waits for PTY registration, and sends the command plus Enter via `pty_write`.
- Add exhaustive i18n keys for Plugin Settings, category labels, install dialog, Run menu labels, errors, phases, empty states, and command kinds. Replace the existing static HeartBeat category label with an i18n key while touching Settings categories.

## Tests

- Backend unit tests cover manifest validation, category validation, registry persistence, install rollback, built-in removal protection, enable/disable/remove behavior, run detector parsing, deduplication, and stable ordering.
- Frontend/behavior checks cover Settings -> Plugins rendering, category/search filtering, install dialog validation/progress, built-in removal lock, disabled plugin exclusion from Run menu, and Run command terminal-slot execution.
- Manual checks cover local BLXCode repo discovery, monorepo package grouping, remote SSH workspace discovery/launch, all-theme readability, narrow layout behavior, and language switch coverage.

## Tasks

- [x] `plugin-types-store` - Add plugin manifest/category/registry types and app-data store. Commit after tests pass.
- [ ] `builtin-runtime-plugins` - Add built-in declarative runtime plugin definitions and detector parser. Commit after detector tests pass.
- [ ] `plugin-install-commands` - Add GitHub install, progress polling, enable/disable/remove commands. Commit after install/store tests pass.
- [ ] `run-discovery-command` - Add `run_commands_discover` using enabled plugins and local/remote workspace scanning. Commit after discovery tests pass.
- [ ] `tauri-bridge-wrappers` - Add frontend typed wrappers/listeners or polling models. Commit after typecheck/check passes.
- [ ] `plugins-settings-pane` - Add Settings -> Plugins UI, install dialog, filters, and theme-token CSS. Commit after UI build/typecheck passes.
- [ ] `titlebar-run-menu` - Add Run menu after Worktree and terminal-slot execution flow. Commit after behavior tests/manual check pass.
- [ ] `i18n-docs-polish` - Complete locale keys, update developer/user docs where needed, and ensure no hardcoded new strings. Commit after final checks pass.
