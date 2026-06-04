# Hardcoded UI Strings — Audit Report

> Auto-generated scan of `src/` for English UI strings that are **not** routed through the `I18nKey` / `i18n.tr(...)` pipeline.
>
> Scope: every `.rs` file under `src/` except the i18n infrastructure (`i18n/locales/*.rs`, `i18n/keys.rs`, `i18n/eula.rs`, `service/service.i18n.rs`).
>
> Methodology: regex harvest of `title=…`, `aria-label=…`, `placeholder=…`, `alt=…`, `label=…`, plain JSX text nodes, and `toast.success/error/warn/info/loading/resolve(…)` literals. Each hit is then checked against the 1107 canonical English strings declared in `src/i18n/locales/en_us.rs`. Anything in the canonical set is considered already i18n'd and is dropped from this report.
>
> **Filtering:** lines starting with `#[…]` attributes, lines inside `#[cfg(test)] mod tests { … }` blocks, lines declaring `const`/`id`/`stack`/`slug`/`name`/`label` fields, match-arm patterns like `"running" => …`, and Rust type/enum patterns like `Foo::Bar` are all skipped.
>
> Result: **326** hardcoded strings across **67** files.

**Legend** for the *Pos* column:

- `T:` — JSX `title=…` attribute (hover tooltip / native label)
- `A:` — `aria-label=…` (screen reader / accessibility)
- `P:` — `placeholder=…` (input field hint)
- `L:` — `alt=…` (image alt text)
- `X:` — `label=…` (form field label)
- `txt:` — JSX text content (between tags)
- `toast:` — `toast.success/error/warn/info/loading(…)` literal (static portion of `format!(…)` is preserved)

---

## Summary by file

| File | Hardcoded strings |
|---|---:|
| `src/app.rs` | 4 |
| `src/boot_loading.rs` | 6 |
| `src/tauri_bridge.rs` | 1 |
| `src/workbench/agent_context_handoff.rs` | 1 |
| `src/workbench/agent_onboarding_dialog.rs` | 4 |
| `src/workbench/agent_panel/composer/mod.rs` | 6 |
| `src/workbench/agent_panel/context_list.rs` | 12 |
| `src/workbench/agent_panel/diagram_result.rs` | 4 |
| `src/workbench/agent_panel/image_context.rs` | 30 |
| `src/workbench/agent_panel/mod.rs` | 8 |
| `src/workbench/agent_panel/reducer.rs` | 3 |
| `src/workbench/agent_panel/session_stats.rs` | 1 |
| `src/workbench/agent_panel/timeline.rs` | 7 |
| `src/workbench/agent_panel/voice_orb/drobo_glue.rs` | 2 |
| `src/workbench/agent_panel/voice_orb/mod.rs` | 3 |
| `src/workbench/agent_settings_pane/data.rs` | 24 |
| `src/workbench/agent_timeline.rs` | 26 |
| `src/workbench/app_titlebar/help_menu.rs` | 13 |
| `src/workbench/app_titlebar/navigate_menu.rs` | 1 |
| `src/workbench/app_titlebar/notifications_menu.rs` | 4 |
| `src/workbench/app_titlebar/view_mode_menu.rs` | 1 |
| `src/workbench/browser_tab.rs` | 3 |
| `src/workbench/close_terminals_tab_dialog/mod.rs` | 1 |
| `src/workbench/commit_dialog/mod.rs` | 1 |
| `src/workbench/confirm_dialog/mod.rs` | 1 |
| `src/workbench/context_drag_overlay.rs` | 3 |
| `src/workbench/core_status/mod.rs` | 1 |
| `src/workbench/create_workspace_wizard.rs` | 9 |
| `src/workbench/diagram_gallery/mod.rs` | 5 |
| `src/workbench/editor_shortcut_config.rs` | 2 |
| `src/workbench/file_preview/code_view.rs` | 1 |
| `src/workbench/file_preview/codemirror_glue.rs` | 1 |
| `src/workbench/file_preview/mermaid_glue.rs` | 1 |
| `src/workbench/git_graph/mod.rs` | 1 |
| `src/workbench/harness_chords.rs` | 4 |
| `src/workbench/harness_ui.rs` | 5 |
| `src/workbench/harness_voice_pane/model_manager/mod.rs` | 1 |
| `src/workbench/heartbeat_settings_pane.rs` | 8 |
| `src/workbench/hook_install_dialog/mod.rs` | 1 |
| `src/workbench/memory_graph/graph_glue.rs` | 1 |
| `src/workbench/memory_graph/mod.rs` | 1 |
| `src/workbench/memory_panel.rs` | 22 |
| `src/workbench/memory_settings_pane.rs` | 3 |
| `src/workbench/mod.rs` | 6 |
| `src/workbench/path_nav.rs` | 1 |
| `src/workbench/plans_panel/ai_generate_dialog/mod.rs` | 1 |
| `src/workbench/plans_panel/mod.rs` | 2 |
| `src/workbench/pointer_agents.rs` | 4 |
| `src/workbench/post_update_notes.rs` | 1 |
| `src/workbench/remote_settings_pane/connection_card.rs` | 1 |
| `src/workbench/shortcut_config.rs` | 13 |
| `src/workbench/shortcuts_settings_pane/mod.rs` | 5 |
| `src/workbench/sidebar.rs` | 3 |
| `src/workbench/skills_rules_panel/rules_pointers.rs` | 2 |
| `src/workbench/skills_rules_panel/rules_tab.rs` | 2 |
| `src/workbench/skills_rules_panel/skill_card.rs` | 1 |
| `src/workbench/skills_rules_panel/skills_tab.rs` | 2 |
| `src/workbench/state.rs` | 1 |
| `src/workbench/terminal_agent_profiles.rs` | 1 |
| `src/workbench/terminal_cell.rs` | 8 |
| `src/workbench/terminal_context_menu.rs` | 1 |
| `src/workbench/terminal_usage.rs` | 14 |
| `src/workbench/update_dialog.rs` | 1 |
| `src/workbench/update_service.rs` | 1 |
| `src/workbench/workspace_kanban/mod.rs` | 14 |
| `src/workbench/workspace_panel.rs` | 3 |
| `src/workbench/workspace_settings_pane/mod.rs` | 1 |
| **Total** | **326** |

---

## Detailed listing

### `src/app.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 448 | txt: | `text` | `Memory index stalled` |
| 451 | txt: | `text` | `Memory index error` |
| 452 | txt: | `text` | `Memory indexing` |
| 481 | txt: | `text` | `Plan migration failed` |

### `src/boot_loading.rs`  (6 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 13 | txt: | `text` | `Starting BLXCode` |
| 14 | txt: | `text` | `Restoring workspace` |
| 15 | txt: | `text` | `Opening workbench` |
| 21 | txt: | `text` | `Preparing the interface` |
| 22 | txt: | `text` | `Loading sidebar, sessions, and workspace state` |
| 23 | txt: | `text` | `Bringing the panels online` |

### `src/tauri_bridge.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 4509 | txt: | `text` | `Space` |

### `src/workbench/agent_context_handoff.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 850 | txt: | `text` | `Escape` |

### `src/workbench/agent_onboarding_dialog.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 106 | txt: | `text` | `Escape` |
| 117 | txt: | `text` | `Name your BLXCode Agent` |
| 120 | txt: | `text` | `Choose the name and default role used when you create new workspaces.` |
| 188 | txt: | `text` | `Use defaults` |

### `src/workbench/agent_panel/composer/mod.rs`  (6 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 189 | txt: | `text` | `Remove favorite` |
| 189 | txt: | `text` | `Add favorite` |
| 190 | txt: | `text` | `Remove favorite` |
| 190 | txt: | `text` | `Add favorite` |
| 258 | txt: | `text` | `Escape` |
| 366 | txt: | `text` | `Enter` |

### `src/workbench/agent_panel/context_list.rs`  (12 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 28 | txt: | `text` | `Collapse context` |
| 30 | txt: | `text` | `Expand context` |
| 111 | T: | `title` | `Remove context` |
| 112 | A: | `aria-label` | `Remove context` |
| 150 | T: | `title` | `Use image again` |
| 151 | A: | `aria-label` | `Use image again` |
| 193 | T: | `title` | `Remove image` |
| 194 | A: | `aria-label` | `Remove image` |
| 216 | txt: | `text` | `Escape` |
| 251 | txt: | `text` | `Use again` |
| 264 | A: | `aria-label` | `Image preview` |
| 282 | L: | `alt` | `Attached image preview` |

### `src/workbench/agent_panel/diagram_result.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 95 | toast: | `toast` | `Export failed:` |
| 107 | toast: | `toast` | `Diagram not rendered yet` |
| 107 | txt: | `text` | `Diagram not rendered yet` |
| 115 | toast: | `toast` | `Export failed:` |

### `src/workbench/agent_panel/image_context.rs`  (30 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 60 | txt: | `text` | `Drop images to attach` |
| 61 | txt: | `text` | `Drop terminal to attach session context` |
| 62 | txt: | `text` | `Drop file to attach as context` |
| 63 | txt: | `text` | `Drop folder to attach as context` |
| 64 | txt: | `text` | `Drop diff to attach as context` |
| 65 | txt: | `text` | `Drop commit to attach as context` |
| 66 | txt: | `text` | `Drop plan to load into the agent` |
| 67 | txt: | `text` | `Drop task to attach as context` |
| 68 | txt: | `text` | `Only image files or terminal sessions can be attached` |
| 249 | txt: | `text` | `Escape` |
| 340 | txt: | `text` | `Pasted image` |
| 348 | txt: | `text` | `Only PNG, JPEG, WebP, and GIF images can be attached.` |
| 361 | txt: | `text` | `Could not create image reader.` |
| 369 | txt: | `text` | `Could not read image data.` |
| 373 | txt: | `text` | `Could not parse image data.` |
| 401 | txt: | `text` | `Only PNG, JPEG, WebP, and GIF images can be attached.` |
| 413 | txt: | `text` | `Select a workspace tab first.` |
| 455 | txt: | `text` | `Select a workspace tab first.` |
| 461 | txt: | `text` | `Terminal context can only be attached to its own workspace.` |
| 524 | txt: | `text` | `Select a workspace tab first.` |
| 530 | txt: | `text` | `Kanban items can only be attached to their own workspace.` |
| 542 | txt: | `text` | `Select a workspace tab first.` |
| 589 | txt: | `text` | `Select a workspace tab first.` |
| 595 | txt: | `text` | `Context can only be attached to its own workspace.` |
| 604 | txt: | `text` | `Dragged file has no path.` |
| 614 | txt: | `text` | `Dragged folder has no path.` |
| 624 | txt: | `text` | `Dragged diff has no path.` |
| 630 | txt: | `text` | `Workspace has no path.` |
| 649 | txt: | `text` | `Dragged commit has no id.` |
| 659 | txt: | `text` | `Workspace has no path.` |

### `src/workbench/agent_panel/mod.rs`  (8 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 686 | A: | `aria-label` | `Jump to bottom` |
| 760 | txt: | `text` | `Select a workspace tab first.` |
| 927 | txt: | `text` | `Idle` |
| 1135 | txt: | `text` | `Select a workspace tab first.` |
| 1172 | txt: | `text` | `Prompt enhancement returned an empty prompt.` |
| 1307 | txt: | `text` | `Agent error` |
| 1324 | txt: | `text` | `The agent needs your input.` |
| 1329 | txt: | `text` | `Agent needs input` |

### `src/workbench/agent_panel/reducer.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 417 | txt: | `text` | `Run this tool call and keep supervised mode.` |
| 422 | txt: | `text` | `Run this and switch this workspace to Full Access.` |
| 429 | txt: | `text` | `Run this tool call now.` |

### `src/workbench/agent_panel/session_stats.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 291 | txt: | `text` | `Turn` |

### `src/workbench/agent_panel/timeline.rs`  (7 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 724 | txt: | `text` | `The model completed the turn without emitting visible text.` |
| 995 | T: | `title` | `Play` |
| 996 | A: | `aria-label` | `Play message audio` |
| 1070 | T: | `title` | `Copy answer to clipboard` |
| 1071 | A: | `aria-label` | `Copy answer` |
| 1099 | T: | `title` | `Redo this turn (resubmit the same prompt)` |
| 1100 | A: | `aria-label` | `Redo` |

### `src/workbench/agent_panel/voice_orb/drobo_glue.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 50 | txt: | `text` | `Drobo orb bundle did not become ready` |
| 59 | txt: | `text` | `Drobo orb id missing` |

### `src/workbench/agent_panel/voice_orb/mod.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 329 | txt: | `text` | `Escape` |
| 334 | txt: | `text` | `Enter` |
| 340 | txt: | `text` | `Enter` |

### `src/workbench/agent_settings_pane/data.rs`  (24 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 418 | txt: | `text` | `Amazon Transcribe` |
| 419 | txt: | `text` | `AWS speech-to-text.` |
| 422 | txt: | `text` | `Polly Neural` |
| 422 | txt: | `text` | `Neural TTS engine.` |
| 423 | txt: | `text` | `Polly Standard` |
| 423 | txt: | `text` | `Standard TTS engine.` |
| 461 | txt: | `text` | `Alloy` |
| 462 | txt: | `text` | `Nova` |
| 463 | txt: | `text` | `Echo` |
| 464 | txt: | `text` | `Shimmer` |
| 465 | txt: | `text` | `Onyx` |
| 466 | txt: | `text` | `Coral` |
| 469 | txt: | `text` | `Joanna` |
| 469 | txt: | `text` | `Joanna` |
| 470 | txt: | `text` | `Matthew` |
| 470 | txt: | `text` | `Matthew` |
| 471 | txt: | `text` | `Amy` |
| 471 | txt: | `text` | `Amy` |
| 472 | txt: | `text` | `Brian` |
| 472 | txt: | `text` | `Brian` |
| 473 | txt: | `text` | `Emma` |
| 473 | txt: | `text` | `Emma` |
| 474 | txt: | `text` | `Arthur` |
| 474 | txt: | `text` | `Arthur` |

### `src/workbench/agent_timeline.rs`  (26 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 107 | txt: | `text` | `List workspaces` |
| 108 | txt: | `text` | `Switch workspace` |
| 109 | txt: | `text` | `Previous workspace` |
| 110 | txt: | `text` | `Next workspace` |
| 111 | txt: | `text` | `Show view` |
| 112 | txt: | `text` | `Open settings` |
| 113 | txt: | `text` | `Open memory` |
| 114 | txt: | `text` | `Open plans` |
| 115 | txt: | `text` | `Open file` |
| 116 | txt: | `text` | `Open diff` |
| 117 | txt: | `text` | `Window state` |
| 118 | txt: | `text` | `Set window size` |
| 119 | txt: | `text` | `Set fullscreen` |
| 120 | txt: | `text` | `List notifications` |
| 121 | txt: | `text` | `Create notification` |
| 122 | txt: | `text` | `Send notification` |
| 123 | txt: | `text` | `Update notification` |
| 124 | txt: | `text` | `Remove notification` |
| 125 | txt: | `text` | `Mark notification read` |
| 156 | txt: | `text` | `Open terminal` |
| 157 | txt: | `text` | `List terminals` |
| 158 | txt: | `text` | `Send keys to terminal` |
| 159 | txt: | `text` | `Send agent context to terminal` |
| 160 | txt: | `text` | `Read terminal output` |
| 161 | txt: | `text` | `Wait for terminal output` |
| 162 | txt: | `text` | `Interrupt terminal` |

### `src/workbench/app_titlebar/help_menu.rs`  (13 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 43 | txt: | `text` | `Escape` |
| 86 | A: | `aria-label` | `Help` |
| 87 | T: | `title` | `Help` |
| 197 | txt: | `text` | `Yes, Free!` |
| 201 | txt: | `text` | `Open Source` |
| 204 | txt: | `text` | `MIT` |
| 220 | txt: | `text` | `Rust 2021` |
| 221 | txt: | `text` | `Tauri 2` |
| 222 | txt: | `text` | `Leptos 0.8` |
| 243 | txt: | `text` | `Repository` |
| 251 | txt: | `text` | `Website` |
| 259 | txt: | `text` | `Issues` |
| 267 | txt: | `text` | `Discussions` |

### `src/workbench/app_titlebar/navigate_menu.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 38 | txt: | `text` | `Escape` |

### `src/workbench/app_titlebar/notifications_menu.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 51 | txt: | `text` | `Escape` |
| 97 | T: | `title` | `Mark all read` |
| 98 | A: | `aria-label` | `Mark all read` |
| 174 | A: | `aria-label` | `Remove notification` |

### `src/workbench/app_titlebar/view_mode_menu.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 33 | txt: | `text` | `Escape` |

### `src/workbench/browser_tab.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 497 | txt: | `text` | `Enter` |
| 612 | txt: | `text` | `This page blocks iframe embedding in the app.` |
| 625 | txt: | `text` | `Open In Browser` |

### `src/workbench/close_terminals_tab_dialog/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 59 | txt: | `text` | `Escape` |

### `src/workbench/commit_dialog/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 116 | txt: | `text` | `Escape` |

### `src/workbench/confirm_dialog/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 60 | txt: | `text` | `Escape` |

### `src/workbench/context_drag_overlay.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 32 | txt: | `text` | `File` |
| 33 | txt: | `text` | `Folder` |
| 34 | txt: | `text` | `Diff` |

### `src/workbench/core_status/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 388 | txt: | `text` | `VIM` |

### `src/workbench/create_workspace_wizard.rs`  (9 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 271 | txt: | `text` | `Name darf nicht leer sein` |
| 276 | txt: | `text` | `Kein Verzeichnis ausgewählt` |
| 280 | txt: | `text` | `Nicht in Tauri-Shell` |
| 434 | txt: | `text` | `Enter` |
| 437 | txt: | `text` | `Escape` |
| 507 | txt: | `text` | `Enter` |
| 510 | txt: | `text` | `Escape` |
| 809 | txt: | `text` | `Enter` |
| 812 | txt: | `text` | `Escape` |

### `src/workbench/diagram_gallery/mod.rs`  (5 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 246 | toast: | `toast` | `Export failed:` |
| 256 | toast: | `toast` | `Diagram not rendered yet` |
| 256 | txt: | `text` | `Diagram not rendered yet` |
| 263 | toast: | `toast` | `Export failed:` |
| 295 | toast: | `toast` | `Delete failed:` |

### `src/workbench/editor_shortcut_config.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 96 | txt: | `text` | `ArrowUp` |
| 97 | txt: | `text` | `ArrowDown` |

### `src/workbench/file_preview/code_view.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 174 | txt: | `text` | `Escape` |

### `src/workbench/file_preview/codemirror_glue.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 17 | txt: | `text` | `BlxCM` |

### `src/workbench/file_preview/mermaid_glue.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 58 | txt: | `text` | `Mermaid bundle did not become ready` |

### `src/workbench/git_graph/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 665 | txt: | `text` | `Open on GitHub` |

### `src/workbench/harness_chords.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 109 | txt: | `text` | `Escape` |
| 150 | txt: | `text` | `Escape` |
| 180 | txt: | `text` | `Escape` |
| 199 | txt: | `text` | `TEXTAREA` |

### `src/workbench/harness_ui.rs`  (5 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 366 | txt: | `text` | `Enter` |
| 929 | X: | `label` | `HeartBeat` |
| 1289 | txt: | `text` | `Stable` |
| 1305 | txt: | `text` | `Beta` |
| 1316 | txt: | `text` | `Channel` |

### `src/workbench/harness_voice_pane/model_manager/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 151 | txt: | `text` | `Default order` |

### `src/workbench/heartbeat_settings_pane.rs`  (8 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 101 | txt: | `text` | `Run internal BLXCode background services on a controlled interval.` |
| 126 | txt: | `text` | `Enable HeartBeat` |
| 154 | txt: | `text` | `Saving...` |
| 186 | txt: | `text` | `No response yet.` |
| 224 | txt: | `text` | `Run` |
| 247 | txt: | `text` | `Idle` |
| 249 | txt: | `text` | `Stalled` |
| 250 | txt: | `text` | `Error` |

### `src/workbench/hook_install_dialog/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 123 | txt: | `text` | `Escape` |

### `src/workbench/memory_graph/graph_glue.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 51 | txt: | `text` | `Graph 3D bundle did not become ready` |

### `src/workbench/memory_graph/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 1132 | txt: | `text` | `ARCHITECTURE.md` |

### `src/workbench/memory_panel.rs`  (22 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 382 | toast: | `toast` | `Architecture rebuild failed:` |
| 490 | txt: | `text` | `Hide terminal split view` |
| 492 | txt: | `text` | `Show terminal split view` |
| 497 | txt: | `text` | `Hide terminal split view` |
| 499 | txt: | `text` | `Show terminal split view` |
| 514 | T: | `title` | `Rebuild architecture map` |
| 515 | A: | `aria-label` | `Rebuild architecture map` |
| 647 | T: | `title` | `Create workspace memory` |
| 649 | txt: | `text` | `Create the workspace memory and learnings folders with matching README.md overview files.` |
| 659 | T: | `title` | `Create global memory` |
| 661 | txt: | `text` | `Create the global memory and learnings folders with matching README.md overview files.` |
| 697 | txt: | `text` | `Create folders` |
| 784 | txt: | `text` | `Agent memory pointers` |
| 919 | txt: | `text` | `File missing` |
| 925 | txt: | `text` | `Not installed` |
| 1214 | T: | `title` | `Open memory in centered tab` |
| 1215 | A: | `aria-label` | `Open memory in centered tab` |
| 1264 | txt: | `text` | `Projekt` |
| 1665 | txt: | `text` | `Enter` |
| 1786 | txt: | `text` | `Enter` |
| 2650 | txt: | `text` | `Show in sidebar` |
| 2914 | txt: | `text` | `README.md` |

### `src/workbench/memory_settings_pane.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 54 | txt: | `text` | `Together` |
| 255 | txt: | `text` | `Save indexing model` |
| 266 | txt: | `text` | `Loading...` |

### `src/workbench/mod.rs`  (6 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 236 | txt: | `text` | `BLXCode workspace files are ready.` |
| 439 | txt: | `text` | `BLXCode workspace files are missing; bootstrap skipped by saved choice.` |
| 448 | txt: | `text` | `Create BLXCode workspace files?` |
| 450 | txt: | `text` | `Create automatically` |
| 451 | txt: | `text` | `Not now` |
| 463 | txt: | `text` | `BLXCode workspace bootstrap skipped. The choice was saved.` |

### `src/workbench/path_nav.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 37 | txt: | `text` | `HOME is not available in the browser build; use an absolute path.` |

### `src/workbench/plans_panel/ai_generate_dialog/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 156 | txt: | `text` | `Escape` |

### `src/workbench/plans_panel/mod.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 527 | P: | `placeholder` | `Search plans...` |
| 528 | A: | `aria-label` | `Search plans` |

### `src/workbench/pointer_agents.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 20 | txt: | `text` | `CLAUDE.md` |
| 26 | txt: | `text` | `AGENTS.md` |
| 32 | txt: | `text` | `GEMINI.md` |
| 44 | txt: | `text` | `AGENTS.md` |

### `src/workbench/post_update_notes.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 114 | txt: | `text` | `Escape` |

### `src/workbench/remote_settings_pane/connection_card.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 116 | txt: | `text` | `Enter` |

### `src/workbench/shortcut_config.rs`  (13 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 52 | txt: | `text` | `Control` |
| 52 | txt: | `text` | `Shift` |
| 52 | txt: | `text` | `Alt` |
| 52 | txt: | `text` | `Meta` |
| 86 | txt: | `text` | `Mod` |
| 89 | txt: | `text` | `Shift` |
| 92 | txt: | `text` | `Alt` |
| 107 | txt: | `text` | `Ctrl` |
| 110 | txt: | `text` | `Shift` |
| 113 | txt: | `text` | `Alt` |
| 239 | txt: | `text` | `Space` |
| 258 | txt: | `text` | `Space` |
| 394 | txt: | `text` | `Space` |

### `src/workbench/shortcuts_settings_pane/mod.rs`  (5 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 86 | txt: | `text` | `Control` |
| 86 | txt: | `text` | `Shift` |
| 86 | txt: | `text` | `Alt` |
| 86 | txt: | `text` | `Meta` |
| 89 | txt: | `text` | `Escape` |

### `src/workbench/sidebar.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 144 | txt: | `text` | `Escape` |
| 890 | txt: | `text` | `Enter` |
| 929 | txt: | `text` | `Enter` |

### `src/workbench/skills_rules_panel/rules_pointers.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 213 | txt: | `text` | `File missing` |
| 219 | txt: | `text` | `Not installed` |

### `src/workbench/skills_rules_panel/rules_tab.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 234 | P: | `placeholder` | `Search rules...` |
| 235 | A: | `aria-label` | `Search rules` |

### `src/workbench/skills_rules_panel/skill_card.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 86 | A: | `aria-label` | `Skill category` |

### `src/workbench/skills_rules_panel/skills_tab.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 180 | P: | `placeholder` | `Search skills...` |
| 181 | A: | `aria-label` | `Search skills` |

### `src/workbench/state.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 206 | txt: | `text` | `Kanban` |

### `src/workbench/terminal_agent_profiles.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 42 | txt: | `text` | `CLAUDE_CODE_EFFORT_LEVEL` |

### `src/workbench/terminal_cell.rs`  (8 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 437 | txt: | `text` | `Escape` |
| 1000 | txt: | `text` | `BLX_TERMINAL_KEY` |
| 1002 | txt: | `text` | `BLX_AGENT_SLUG` |
| 1005 | txt: | `text` | `BLX_SESSIONS_PATH` |
| 1008 | txt: | `text` | `BLX_NOTIFICATIONS_PATH` |
| 1011 | txt: | `text` | `BLX_USAGE_PATH` |
| 1016 | txt: | `text` | `BLX_AGENT_CONTEXT_DIR` |
| 1020 | txt: | `text` | `BLX_AGENT_CONTEXT_MANIFEST` |

### `src/workbench/terminal_context_menu.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 271 | txt: | `text` | `Escape` |

### `src/workbench/terminal_usage.rs`  (14 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 119 | txt: | `text` | `Escape` |
| 151 | txt: | `text` | `Usage unavailable` |
| 167 | T: | `title` | `Agent usage` |
| 168 | A: | `aria-label` | `Agent usage` |
| 201 | T: | `title` | `Refresh usage` |
| 202 | A: | `aria-label` | `Refresh usage` |
| 290 | txt: | `text` | `Usage unavailable until Claude updates its status line` |
| 296 | txt: | `text` | `No running terminal session` |
| 304 | txt: | `text` | `No running terminal session` |
| 310 | txt: | `text` | `Usage unavailable for this agent` |
| 332 | txt: | `text` | `Usage command timed out` |
| 345 | txt: | `text` | `Claude usage payload missing` |
| 348 | txt: | `text` | `Claude rate limits missing` |
| 441 | txt: | `text` | `Usage unavailable` |

### `src/workbench/update_dialog.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 57 | txt: | `text` | `Escape` |

### `src/workbench/update_service.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 246 | txt: | `text` | `Updater is only available in the desktop app.` |

### `src/workbench/workspace_kanban/mod.rs`  (14 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 206 | toast: | `toast` | `Export failed:` |
| 208 | toast: | `toast` | `Export failed:` |
| 224 | toast: | `toast` | `Import failed:` |
| 226 | toast: | `toast` | `Import failed:` |
| 280 | toast: | `toast` | `Plan move failed:` |
| 303 | toast: | `toast` | `Task move failed:` |
| 390 | txt: | `text` | `Enter` |
| 567 | txt: | `text` | `Escape` |
| 641 | txt: | `text` | `Escape` |
| 828 | T: | `title` | `Drag plan` |
| 1022 | toast: | `toast` | `Delete failed:` |
| 1056 | toast: | `toast` | `Rename failed:` |
| 1124 | txt: | `text` | `Enter` |
| 1126 | txt: | `text` | `Escape` |

### `src/workbench/workspace_panel.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 732 | A: | `aria-label` | `Resize terminal and memory split` |
| 842 | txt: | `text` | `Enter` |
| 1121 | X: | `label` | `Kanban` |

### `src/workbench/workspace_settings_pane/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 382 | txt: | `text` | `LLM prose ingest` |
