# Hardcoded UI Strings — Audit Report

> Auto-generated scan of `src/` for English UI strings that are **not** routed through the `I18nKey` / `i18n.tr(...)` pipeline.
>
> Scope: every `.rs` file under `src/` except the i18n infrastructure (`i18n/locales/*.rs`, `i18n/keys.rs`, `i18n/eula.rs`, `service/service.i18n.rs`).
>
> Methodology: regex harvest of `title=…`, `aria-label=…`, `placeholder=…`, `alt=…`, `label=…`, plain JSX text nodes, `toast.success/error/warn/info/loading/resolve(…)` literals, and `format!(…)` static prefixes. Each hit is then checked against the 1107 canonical English strings declared in `src/i18n/locales/en_us.rs`. Anything in the canonical set is considered already i18n'd and is dropped from this report.
>
> Result: **532** hardcoded strings across **67** files.

**Legend** for the *Pos* column:

- `T:` — JSX `title=…` attribute (hover tooltip / native label)
- `A:` — `aria-label=…` (screen reader / accessibility)
- `P:` — `placeholder=…` (input field hint)
- `L:` — `alt=…` (image alt text)
- `X:` — `label=…` (form field label)
- `txt:` — JSX text content (between tags)
- `toast:` / `toast.resolve(...)` — toast / notification call
- `T(dyn)` / `A(dyn)` / `P(dyn)` — dynamic closure (`i18n.tr(...)()` etc.) — included for context, **not** flagged as hardcoded

---

## Summary by file

| File | Hardcoded strings |
|---|---:|
| `src/agent_wire.rs` | 4 |
| `src/app.rs` | 6 |
| `src/boot_loading.rs` | 6 |
| `src/skills_rules_wire.rs` | 11 |
| `src/tauri_bridge.rs` | 51 |
| `src/theme/appearance.rs` | 7 |
| `src/workbench/agent_context_handoff.rs` | 24 |
| `src/workbench/agent_onboarding_dialog.rs` | 3 |
| `src/workbench/agent_panel/composer/mod.rs` | 6 |
| `src/workbench/agent_panel/context_list.rs` | 12 |
| `src/workbench/agent_panel/diagram_result.rs` | 3 |
| `src/workbench/agent_panel/image_context.rs` | 29 |
| `src/workbench/agent_panel/mod.rs` | 27 |
| `src/workbench/agent_panel/reducer.rs` | 10 |
| `src/workbench/agent_panel/session_stats.rs` | 1 |
| `src/workbench/agent_panel/timeline.rs` | 15 |
| `src/workbench/agent_panel/voice_orb/drobo_glue.rs` | 2 |
| `src/workbench/agent_settings_pane/data.rs` | 30 |
| `src/workbench/agent_timeline.rs` | 57 |
| `src/workbench/app_titlebar/help_menu.rs` | 24 |
| `src/workbench/app_titlebar/mod.rs` | 1 |
| `src/workbench/app_titlebar/notifications_menu.rs` | 3 |
| `src/workbench/browser_tab.rs` | 2 |
| `src/workbench/chat_markdown.rs` | 1 |
| `src/workbench/context_drag.rs` | 5 |
| `src/workbench/context_drag_overlay.rs` | 3 |
| `src/workbench/core_status/mod.rs` | 3 |
| `src/workbench/create_workspace_wizard.rs` | 4 |
| `src/workbench/diagram_gallery/mod.rs` | 4 |
| `src/workbench/file_preview/code_view.rs` | 3 |
| `src/workbench/file_preview/codemirror_glue.rs` | 2 |
| `src/workbench/file_preview/editor/policy.rs` | 2 |
| `src/workbench/file_preview/mermaid_glue.rs` | 1 |
| `src/workbench/file_preview/util.rs` | 8 |
| `src/workbench/fuzzy.rs` | 1 |
| `src/workbench/git_graph/mod.rs` | 5 |
| `src/workbench/harness_chords.rs` | 2 |
| `src/workbench/harness_ui.rs` | 6 |
| `src/workbench/harness_voice_pane/model_manager/mod.rs` | 1 |
| `src/workbench/harness_voice_pane/ptt_section/mod.rs` | 1 |
| `src/workbench/heartbeat_settings_pane.rs` | 12 |
| `src/workbench/kanban_dnd.rs` | 1 |
| `src/workbench/memory_graph/graph_glue.rs` | 1 |
| `src/workbench/memory_graph/mod.rs` | 3 |
| `src/workbench/memory_panel.rs` | 24 |
| `src/workbench/memory_settings_pane.rs` | 6 |
| `src/workbench/mod.rs` | 8 |
| `src/workbench/path_nav.rs` | 1 |
| `src/workbench/plans_panel/mod.rs` | 2 |
| `src/workbench/pointer_agents.rs` | 4 |
| `src/workbench/session_role_picker.rs` | 1 |
| `src/workbench/shortcut_config.rs` | 10 |
| `src/workbench/shortcuts_settings_pane/mod.rs` | 4 |
| `src/workbench/sidebar.rs` | 3 |
| `src/workbench/skills_rules_panel/rule_card.rs` | 1 |
| `src/workbench/skills_rules_panel/rules_pointers.rs` | 2 |
| `src/workbench/skills_rules_panel/rules_tab.rs` | 4 |
| `src/workbench/skills_rules_panel/skill_card.rs` | 1 |
| `src/workbench/skills_rules_panel/skills_tab.rs` | 4 |
| `src/workbench/state.rs` | 9 |
| `src/workbench/terminal_agent_profiles.rs` | 2 |
| `src/workbench/terminal_cell.rs` | 7 |
| `src/workbench/terminal_usage.rs` | 16 |
| `src/workbench/update_service.rs` | 1 |
| `src/workbench/workspace_kanban/mod.rs` | 11 |
| `src/workbench/workspace_panel.rs` | 6 |
| `src/workbench/workspace_settings_pane/mod.rs` | 2 |
| **Total** | **532** |

---

## Detailed listing

### `src/agent_wire.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 68 | txt: | `text` | `Option::is_none` |
| 117 | txt: | `text` | `Option::is_none` |
| 119 | txt: | `text` | `Option::is_none` |
| 128 | txt: | `text` | `Option::is_none` |

### `src/app.rs`  (6 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 364 | A: | `aria-label` | `Application status` |
| 445 | txt: | `text` | `HeartBeat` |
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

### `src/skills_rules_wire.rs`  (11 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 40 | txt: | `text` | `Option::is_none` |
| 59 | txt: | `text` | `Option::is_none` |
| 61 | txt: | `text` | `Option::is_none` |
| 63 | txt: | `text` | `Option::is_none` |
| 65 | txt: | `text` | `Option::is_none` |
| 67 | txt: | `text` | `Option::is_none` |
| 75 | txt: | `text` | `Option::is_none` |
| 77 | txt: | `text` | `Option::is_none` |
| 79 | txt: | `text` | `Option::is_none` |
| 81 | txt: | `text` | `Option::is_none` |
| 83 | txt: | `text` | `Option::is_none` |

### `src/tauri_bridge.rs`  (51 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 392 | txt: | `text` | `Option::is_none` |
| 394 | txt: | `text` | `Option::is_none` |
| 432 | txt: | `text` | `Option::is_none` |
| 435 | txt: | `text` | `Option::is_none` |
| 1255 | txt: | `text` | `Option::is_none` |
| 1280 | txt: | `text` | `Option::is_none` |
| 1305 | txt: | `text` | `Option::is_none` |
| 1331 | txt: | `text` | `Option::is_none` |
| 1369 | txt: | `text` | `Option::is_none` |
| 1411 | txt: | `text` | `Option::is_none` |
| 1413 | txt: | `text` | `Option::is_none` |
| 1501 | txt: | `text` | `Option::is_none` |
| 1525 | txt: | `text` | `Option::is_none` |
| 1549 | txt: | `text` | `Option::is_none` |
| 1589 | txt: | `text` | `Vec::is_empty` |
| 1738 | txt: | `text` | `Option::is_none` |
| 1740 | txt: | `text` | `Option::is_none` |
| 1777 | txt: | `text` | `Option::is_none` |
| 1779 | txt: | `text` | `Option::is_none` |
| 1839 | txt: | `text` | `Vec::is_empty` |
| 2021 | txt: | `text` | `Option::is_none` |
| 2023 | txt: | `text` | `Option::is_none` |
| 2025 | txt: | `text` | `Option::is_none` |
| 2027 | txt: | `text` | `Option::is_none` |
| 2029 | txt: | `text` | `Option::is_none` |
| 2518 | txt: | `text` | `Option::is_none` |
| 2520 | txt: | `text` | `Option::is_none` |
| 2522 | txt: | `text` | `Option::is_none` |
| 2737 | txt: | `text` | `Option::is_none` |
| 3219 | txt: | `text` | `Option::is_none` |
| 3413 | txt: | `text` | `Option::is_none` |
| 3420 | txt: | `text` | `Option::is_none` |
| 3422 | txt: | `text` | `Option::is_none` |
| 3440 | txt: | `text` | `Option::is_none` |
| 3730 | txt: | `text` | `Option::is_none` |
| 3741 | txt: | `text` | `Option::is_none` |
| 3842 | txt: | `text` | `Option::is_none` |
| 3866 | txt: | `text` | `Option::is_none` |
| 3907 | txt: | `text` | `Option::is_none` |
| 3925 | txt: | `text` | `Option::is_none` |
| 3950 | txt: | `text` | `Option::is_none` |
| 3974 | txt: | `text` | `Option::is_none` |
| 3993 | txt: | `text` | `Option::is_none` |
| 4004 | txt: | `text` | `Option::is_none` |
| 4020 | txt: | `text` | `Option::is_none` |
| 4044 | txt: | `text` | `Option::is_none` |
| 4058 | txt: | `text` | `Option::is_none` |
| 4101 | txt: | `text` | `Option::is_none` |
| 4112 | txt: | `text` | `Option::is_none` |
| 4123 | txt: | `text` | `Option::is_none` |
| 4139 | txt: | `text` | `Option::is_none` |

### `src/theme/appearance.rs`  (7 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 98 | txt: | `text` | `JetBrains Mono` |
| 103 | txt: | `text` | `Cascadia Code` |
| 108 | txt: | `text` | `Fira Code` |
| 113 | txt: | `text` | `SF Mono` |
| 118 | txt: | `text` | `Menlo` |
| 123 | txt: | `text` | `Consolas` |
| 128 | txt: | `text` | `System Monospace` |

### `src/workbench/agent_context_handoff.rs`  (24 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 1453 | txt: | `text` | `BLXCode attached context` |
| 1461 | txt: | `text` | `Do the thing` |
| 1463 | txt: | `text` | `Do the thing` |
| 1496 | txt: | `text` | `Cover` |
| 1524 | txt: | `text` | `Shot` |
| 1543 | txt: | `text` | `Foo` |
| 1545 | txt: | `text` | `Bar` |
| 1551 | txt: | `text` | `Chat Pal` |
| 1553 | txt: | `text` | `Chat Pal` |
| 1563 | txt: | `text` | `Plan Manager` |
| 1578 | txt: | `text` | `Plan Manager` |
| 1590 | txt: | `text` | `Draft schema` |
| 1596 | txt: | `text` | `Land backend` |
| 1638 | txt: | `text` | `Workspace:` |
| 1639 | txt: | `text` | `Attached memory` |
| 1666 | txt: | `text` | `Demo` |
| 1711 | txt: | `text` | `Fix bug` |
| 1734 | txt: | `text` | `Fix bug` |
| 1734 | txt: | `text` | `Longer body` |
| 1738 | txt: | `text` | `Fix bug` |
| 1742 | txt: | `text` | `Longer body` |
| 1777 | txt: | `text` | `Snippet` |
| 1815 | txt: | `text` | `Other` |
| 1819 | txt: | `text` | `Target terminal:` |

### `src/workbench/agent_onboarding_dialog.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
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
| 558 | txt: | `text` | `Prompt` |
| 561 | txt: | `text` | `Rewrite before sending` |

### `src/workbench/agent_panel/context_list.rs`  (12 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 28 | txt: | `text` | `Collapse context` |
| 30 | txt: | `text` | `Expand context` |
| 43 | txt: | `text` | `Attached context` |
| 111 | T: | `title` | `Remove context` |
| 112 | A: | `aria-label` | `Remove context` |
| 150 | T: | `title` | `Use image again` |
| 151 | A: | `aria-label` | `Use image again` |
| 193 | T: | `title` | `Remove image` |
| 194 | A: | `aria-label` | `Remove image` |
| 251 | txt: | `text` | `Use again` |
| 264 | A: | `aria-label` | `Image preview` |
| 282 | L: | `alt` | `Attached image preview` |

### `src/workbench/agent_panel/diagram_result.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 95 | toast: | `toast` | `Export failed:` |
| 107 | txt: | `text` | `Diagram not rendered yet` |
| 115 | toast: | `toast` | `Export failed:` |

### `src/workbench/agent_panel/image_context.rs`  (29 strings)

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

### `src/workbench/agent_panel/mod.rs`  (27 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 60 | txt: | `text` | `Dangling` |
| 61 | txt: | `text` | `Baking` |
| 62 | txt: | `text` | `Tracing` |
| 63 | txt: | `text` | `Stitching` |
| 64 | txt: | `text` | `Weighing` |
| 65 | txt: | `text` | `Sketching` |
| 66 | txt: | `text` | `Linking` |
| 67 | txt: | `text` | `Sorting` |
| 68 | txt: | `text` | `Composing` |
| 69 | txt: | `text` | `Refining` |
| 678 | txt: | `text` | `Attach images or generate visual output` |
| 686 | A: | `aria-label` | `Jump to bottom` |
| 698 | txt: | `text` | `Timeline` |
| 700 | txt: | `text` | `Jump to bottom` |
| 701 | txt: | `text` | `Slide to the latest output` |
| 730 | txt: | `text` | `Layout` |
| 741 | txt: | `text` | `Resize the Agent workspace` |
| 760 | txt: | `text` | `Select a workspace tab first.` |
| 786 | txt: | `text` | `History` |
| 927 | txt: | `text` | `Idle` |
| 950 | A: | `aria-label` | `Model speed by turn` |
| 1019 | txt: | `text` | `No speed data` |
| 1135 | txt: | `text` | `Select a workspace tab first.` |
| 1172 | txt: | `text` | `Prompt enhancement returned an empty prompt.` |
| 1307 | txt: | `text` | `Agent error` |
| 1324 | txt: | `text` | `The agent needs your input.` |
| 1329 | txt: | `text` | `Agent needs input` |

### `src/workbench/agent_panel/reducer.rs`  (10 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 416 | txt: | `text` | `Approve once` |
| 417 | txt: | `text` | `Run this tool call and keep supervised mode.` |
| 421 | txt: | `text` | `Auto-accept` |
| 422 | txt: | `text` | `Run this and switch this workspace to Full Access.` |
| 428 | txt: | `text` | `Approve` |
| 429 | txt: | `text` | `Run this tool call now.` |
| 843 | txt: | `text` | `Approve once` |
| 844 | txt: | `text` | `Auto-accept` |
| 874 | txt: | `text` | `Approve` |
| 1021 | txt: | `text` | `Cargo.toml` |

### `src/workbench/agent_panel/session_stats.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 291 | txt: | `text` | `Turn` |

### `src/workbench/agent_panel/timeline.rs`  (15 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 724 | txt: | `text` | `The model completed the turn without emitting visible text.` |
| 995 | T: | `title` | `Play` |
| 996 | A: | `aria-label` | `Play message audio` |
| 1070 | T: | `title` | `Copy answer to clipboard` |
| 1071 | A: | `aria-label` | `Copy answer` |
| 1099 | T: | `title` | `Redo this turn (resubmit the same prompt)` |
| 1100 | A: | `aria-label` | `Redo` |
| 2338 | txt: | `text` | `Show active rules` |
| 2349 | txt: | `text` | `Show active rules` |
| 2356 | txt: | `text` | `Readable` |
| 2366 | txt: | `text` | `Readable` |
| 2377 | txt: | `text` | `Approve file edit?` |
| 2379 | txt: | `text` | `Approve once` |
| 2381 | txt: | `text` | `Auto-accept` |
| 2382 | txt: | `text` | `Switch to Full Access.` |

### `src/workbench/agent_panel/voice_orb/drobo_glue.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 50 | txt: | `text` | `Drobo orb bundle did not become ready` |
| 59 | txt: | `text` | `Drobo orb id missing` |

### `src/workbench/agent_settings_pane/data.rs`  (30 strings)

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
| 498 | txt: | `text` | `Joanna` |
| 498 | txt: | `text` | `Matthew` |
| 498 | txt: | `text` | `Amy` |
| 498 | txt: | `text` | `Brian` |
| 498 | txt: | `text` | `Emma` |
| 498 | txt: | `text` | `Arthur` |

### `src/workbench/agent_timeline.rs`  (57 strings)

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
| 126 | txt: | `text` | `Write workspace file` |
| 127 | txt: | `text` | `Delete workspace entry` |
| 128 | txt: | `text` | `Create workspace folder` |
| 129 | txt: | `text` | `Rename workspace entry` |
| 130 | txt: | `text` | `List files` |
| 131 | txt: | `text` | `Read file` |
| 132 | txt: | `text` | `List memory notes` |
| 133 | txt: | `text` | `Read memory note` |
| 134 | txt: | `text` | `Search memory` |
| 135 | txt: | `text` | `Create memory note` |
| 136 | txt: | `text` | `Update memory note` |
| 137 | txt: | `text` | `Delete memory note` |
| 138 | txt: | `text` | `Rename memory note` |
| 139 | txt: | `text` | `Memory graph` |
| 140 | txt: | `text` | `Memory backlinks` |
| 141 | txt: | `text` | `List memory categories` |
| 142 | txt: | `text` | `Update memory category` |
| 143 | txt: | `text` | `List agent context` |
| 144 | txt: | `text` | `Attach memory context` |
| 145 | txt: | `text` | `Detach memory context` |
| 146 | txt: | `text` | `List plan context` |
| 147 | txt: | `text` | `Attach plan context` |
| 148 | txt: | `text` | `Detach plan context` |
| 149 | txt: | `text` | `List tools` |
| 150 | txt: | `text` | `List tasks` |
| 151 | txt: | `text` | `Read task` |
| 152 | txt: | `text` | `Create task` |
| 153 | txt: | `text` | `Update task` |
| 154 | txt: | `text` | `Delete task` |
| 155 | txt: | `text` | `Reorder tasks` |
| 156 | txt: | `text` | `Open terminal` |
| 157 | txt: | `text` | `List terminals` |
| 158 | txt: | `text` | `Send keys to terminal` |
| 159 | txt: | `text` | `Send agent context to terminal` |
| 160 | txt: | `text` | `Read terminal output` |
| 161 | txt: | `text` | `Wait for terminal output` |
| 162 | txt: | `text` | `Interrupt terminal` |
| 245 | txt: | `text` | `Option::is_none` |

### `src/workbench/app_titlebar/help_menu.rs`  (24 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 18 | txt: | `text` | `CARGO_PKG_VERSION` |
| 86 | A: | `aria-label` | `Help` |
| 87 | T: | `title` | `Help` |
| 97 | txt: | `text` | `Help` |
| 105 | txt: | `text` | `About` |
| 114 | txt: | `text` | `Docs` |
| 124 | txt: | `text` | `Discuss` |
| 133 | txt: | `text` | `Report Bug` |
| 143 | txt: | `text` | `Website` |
| 152 | txt: | `text` | `Update` |
| 187 | txt: | `text` | `Open-source AI workbench` |
| 194 | A: | `aria-label` | `Project status` |
| 197 | txt: | `text` | `Yes, Free!` |
| 201 | txt: | `text` | `Open Source` |
| 204 | txt: | `text` | `MIT` |
| 219 | A: | `aria-label` | `Stack` |
| 220 | txt: | `text` | `Rust 2021` |
| 221 | txt: | `text` | `Tauri 2` |
| 222 | txt: | `text` | `Leptos 0.8` |
| 228 | A: | `aria-label` | `Project links` |
| 243 | txt: | `text` | `Repository` |
| 251 | txt: | `text` | `Website` |
| 259 | txt: | `text` | `Issues` |
| 267 | txt: | `text` | `Discussions` |

### `src/workbench/app_titlebar/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 123 | A: | `aria-label` | `Breadcrumb` |

### `src/workbench/app_titlebar/notifications_menu.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 97 | T: | `title` | `Mark all read` |
| 98 | A: | `aria-label` | `Mark all read` |
| 174 | A: | `aria-label` | `Remove notification` |

### `src/workbench/browser_tab.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 612 | txt: | `text` | `This page blocks iframe embedding in the app.` |
| 625 | txt: | `text` | `Open In Browser` |

### `src/workbench/chat_markdown.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 617 | txt: | `text` | `Hello, world!` |

### `src/workbench/context_drag.rs`  (5 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 43 | txt: | `text` | `Option::is_none` |
| 46 | txt: | `text` | `Option::is_none` |
| 49 | txt: | `text` | `Option::is_none` |
| 52 | txt: | `text` | `Option::is_none` |
| 55 | txt: | `text` | `Option::is_none` |

### `src/workbench/context_drag_overlay.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 32 | txt: | `text` | `File` |
| 33 | txt: | `text` | `Folder` |
| 34 | txt: | `text` | `Diff` |

### `src/workbench/core_status/mod.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 322 | T: | `title` | `Workspace memory` |
| 341 | T: | `title` | `Global memory` |
| 388 | txt: | `text` | `VIM` |

### `src/workbench/create_workspace_wizard.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 271 | txt: | `text` | `Name darf nicht leer sein` |
| 276 | txt: | `text` | `Kein Verzeichnis ausgewählt` |
| 280 | txt: | `text` | `Nicht in Tauri-Shell` |
| 590 | txt: | `text` | `Last opened workspaces` |

### `src/workbench/diagram_gallery/mod.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 246 | toast: | `toast` | `Export failed:` |
| 256 | txt: | `text` | `Diagram not rendered yet` |
| 263 | toast: | `toast` | `Export failed:` |
| 295 | toast: | `toast` | `Delete failed:` |

### `src/workbench/file_preview/code_view.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 501 | txt: | `text` | `Cargo.toml` |
| 505 | txt: | `text` | `Dockerfile` |
| 512 | txt: | `text` | `Makefile` |

### `src/workbench/file_preview/codemirror_glue.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 17 | txt: | `text` | `BlxCM` |
| 101 | txt: | `text` | `BlxCM not available` |

### `src/workbench/file_preview/editor/policy.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 112 | txt: | `text` | `README.md` |
| 121 | txt: | `text` | `LICENSE` |

### `src/workbench/file_preview/mermaid_glue.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 58 | txt: | `text` | `Mermaid bundle did not become ready` |

### `src/workbench/file_preview/util.rs`  (8 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 12 | txt: | `text` | `KiB` |
| 12 | txt: | `text` | `MiB` |
| 12 | txt: | `text` | `GiB` |
| 12 | txt: | `text` | `TiB` |
| 446 | txt: | `text` | `Grüße` |
| 519 | txt: | `text` | `Demo` |
| 534 | txt: | `text` | `Grüße aus München` |
| 536 | txt: | `text` | `Grüße aus München` |

### `src/workbench/fuzzy.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 98 | txt: | `text` | `MAIN` |

### `src/workbench/git_graph/mod.rs`  (5 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 537 | txt: | `text` | `No files changed` |
| 549 | txt: | `text` | `Loading files...` |
| 552 | txt: | `text` | `Could not load files` |
| 649 | txt: | `text` | `Loading...` |
| 665 | txt: | `text` | `Open on GitHub` |

### `src/workbench/harness_chords.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 198 | txt: | `text` | `INPUT` |
| 199 | txt: | `text` | `TEXTAREA` |

### `src/workbench/harness_ui.rs`  (6 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 929 | X: | `label` | `HeartBeat` |
| 1289 | txt: | `text` | `Stable` |
| 1291 | txt: | `text` | `Final GitHub Releases` |
| 1305 | txt: | `text` | `Beta` |
| 1307 | txt: | `text` | `GitHub Prereleases plus newer finals` |
| 1316 | txt: | `text` | `Channel` |

### `src/workbench/harness_voice_pane/model_manager/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 151 | txt: | `text` | `Default order` |

### `src/workbench/harness_voice_pane/ptt_section/mod.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 98 | txt: | `text` | `Options` |

### `src/workbench/heartbeat_settings_pane.rs`  (12 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 98 | txt: | `text` | `HeartBeat` |
| 101 | txt: | `text` | `Run internal BLXCode background services on a controlled interval.` |
| 111 | txt: | `text` | `Schedule` |
| 126 | txt: | `text` | `Enable HeartBeat` |
| 130 | txt: | `text` | `Interval minutes` |
| 154 | txt: | `text` | `Saving...` |
| 165 | txt: | `text` | `Registered services` |
| 186 | txt: | `text` | `No response yet.` |
| 224 | txt: | `text` | `Run` |
| 247 | txt: | `text` | `Idle` |
| 249 | txt: | `text` | `Stalled` |
| 250 | txt: | `text` | `Error` |

### `src/workbench/kanban_dnd.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 31 | txt: | `text` | `Option::is_none` |

### `src/workbench/memory_graph/graph_glue.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 51 | txt: | `text` | `Graph 3D bundle did not become ready` |

### `src/workbench/memory_graph/mod.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 912 | txt: | `text` | `Loading...` |
| 934 | txt: | `text` | `ARCHITECTURE.md` |
| 1132 | txt: | `text` | `ARCHITECTURE.md` |

### `src/workbench/memory_panel.rs`  (24 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 34 | txt: | `text` | `README.md` |
| 40 | txt: | `text` | `ARCHITECTURE.md` |
| 382 | toast: | `toast` | `Architecture rebuild failed:` |
| 490 | txt: | `text` | `Hide terminal split view` |
| 492 | txt: | `text` | `Show terminal split view` |
| 497 | txt: | `text` | `Hide terminal split view` |
| 499 | txt: | `text` | `Show terminal split view` |
| 514 | T: | `title` | `Rebuild architecture map` |
| 515 | A: | `aria-label` | `Rebuild architecture map` |
| 647 | T: | `title` | `Create workspace memory` |
| 659 | T: | `title` | `Create global memory` |
| 697 | txt: | `text` | `Create folders` |
| 784 | txt: | `text` | `Agent memory pointers` |
| 919 | txt: | `text` | `File missing` |
| 925 | txt: | `text` | `Not installed` |
| 1214 | T: | `title` | `Open memory in centered tab` |
| 1215 | A: | `aria-label` | `Open memory in centered tab` |
| 1264 | txt: | `text` | `Projekt` |
| 2584 | txt: | `text` | `Display name` |
| 2596 | txt: | `text` | `Color` |
| 2620 | A: | `aria-label` | `Memory color presets` |
| 2650 | txt: | `text` | `Show in sidebar` |
| 2914 | txt: | `text` | `README.md` |
| 3074 | txt: | `text` | `README.md` |

### `src/workbench/memory_settings_pane.rs`  (6 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 54 | txt: | `text` | `Together` |
| 81 | txt: | `text` | `No index run yet.` |
| 207 | txt: | `text` | `Memory Indexer` |
| 236 | txt: | `text` | `Indexing model` |
| 255 | txt: | `text` | `Save indexing model` |
| 266 | txt: | `text` | `Loading...` |

### `src/workbench/mod.rs`  (8 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 223 | toast: | `toast` | `Creating BLXCode workspace files…` |
| 236 | txt: | `text` | `BLXCode workspace files are ready.` |
| 413 | toast: | `toast` | `Checking BLXCode workspace files…` |
| 439 | txt: | `text` | `BLXCode workspace files are missing; bootstrap skipped by saved choice.` |
| 448 | txt: | `text` | `Create BLXCode workspace files?` |
| 450 | txt: | `text` | `Create automatically` |
| 451 | txt: | `text` | `Not now` |
| 463 | txt: | `text` | `BLXCode workspace bootstrap skipped. The choice was saved.` |

### `src/workbench/path_nav.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 37 | txt: | `text` | `HOME is not available in the browser build; use an absolute path.` |

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

### `src/workbench/session_role_picker.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 9 | txt: | `text` | `Default BLXCode Agent without a specialized harness session role.` |

### `src/workbench/shortcut_config.rs`  (10 strings)

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

### `src/workbench/shortcuts_settings_pane/mod.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 86 | txt: | `text` | `Control` |
| 86 | txt: | `text` | `Shift` |
| 86 | txt: | `text` | `Alt` |
| 86 | txt: | `text` | `Meta` |

### `src/workbench/sidebar.rs`  (3 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 33 | txt: | `text` | `CARGO_PKG_VERSION` |
| 1024 | txt: | `text` | `Push to start voice transcription` |
| 1025 | txt: | `text` | `Open push-to-talk settings and local models` |

### `src/workbench/skills_rules_panel/rule_card.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 220 | A: | `aria-label` | `Rule category` |

### `src/workbench/skills_rules_panel/rules_pointers.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 213 | txt: | `text` | `File missing` |
| 219 | txt: | `text` | `Not installed` |

### `src/workbench/skills_rules_panel/rules_tab.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 162 | A: | `aria-label` | `Filter rules by category` |
| 234 | P: | `placeholder` | `Search rules...` |
| 235 | A: | `aria-label` | `Search rules` |
| 317 | txt: | `text` | `No rules match this search.` |

### `src/workbench/skills_rules_panel/skill_card.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 86 | A: | `aria-label` | `Skill category` |

### `src/workbench/skills_rules_panel/skills_tab.rs`  (4 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 117 | A: | `aria-label` | `Filter skills by category` |
| 180 | P: | `placeholder` | `Search skills...` |
| 181 | A: | `aria-label` | `Search skills` |
| 213 | txt: | `text` | `No skills match this search.` |

### `src/workbench/state.rs`  (9 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 206 | txt: | `text` | `Kanban` |
| 597 | txt: | `text` | `Memory Blue` |
| 602 | txt: | `text` | `Learnings Teal` |
| 607 | txt: | `text` | `Research Violet` |
| 612 | txt: | `text` | `Tasks Amber` |
| 617 | txt: | `text` | `Archive Slate` |
| 4857 | txt: | `text` | `Settings` |
| 5006 | txt: | `text` | `Workspace 7` |
| 5286 | txt: | `text` | `Settings` |

### `src/workbench/terminal_agent_profiles.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 42 | txt: | `text` | `CLAUDE_CODE_EFFORT_LEVEL` |
| 267 | txt: | `text` | `Fake` |

### `src/workbench/terminal_cell.rs`  (7 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 1000 | txt: | `text` | `BLX_TERMINAL_KEY` |
| 1002 | txt: | `text` | `BLX_AGENT_SLUG` |
| 1005 | txt: | `text` | `BLX_SESSIONS_PATH` |
| 1008 | txt: | `text` | `BLX_NOTIFICATIONS_PATH` |
| 1011 | txt: | `text` | `BLX_USAGE_PATH` |
| 1016 | txt: | `text` | `BLX_AGENT_CONTEXT_DIR` |
| 1020 | txt: | `text` | `BLX_AGENT_CONTEXT_MANIFEST` |

### `src/workbench/terminal_usage.rs`  (16 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
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
| 417 | txt: | `text` | `Usage` |
| 441 | txt: | `text` | `Usage unavailable` |
| 607 | txt: | `text` | `Resets in 3d` |
| 613 | txt: | `text` | `No API calls have been made` |

### `src/workbench/update_service.rs`  (1 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 246 | txt: | `text` | `Updater is only available in the desktop app.` |

### `src/workbench/workspace_kanban/mod.rs`  (11 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 206 | toast: | `toast` | `Export failed:` |
| 208 | toast: | `toast` | `Export failed:` |
| 224 | toast: | `toast` | `Import failed:` |
| 226 | toast: | `toast` | `Import failed:` |
| 280 | toast: | `toast` | `Plan move failed:` |
| 303 | toast: | `toast` | `Task move failed:` |
| 828 | T: | `title` | `Drag plan` |
| 1022 | toast: | `toast` | `Delete failed:` |
| 1056 | toast: | `toast` | `Rename failed:` |
| 1265 | txt: | `text` | `Drop plan` |
| 1372 | txt: | `text` | `Drop task` |

### `src/workbench/workspace_panel.rs`  (6 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 732 | A: | `aria-label` | `Resize terminal and memory split` |
| 759 | A: | `aria-label` | `Workspace views` |
| 1023 | txt: | `text` | `Last opened workspaces` |
| 1117 | A: | `aria-label` | `Main destinations` |
| 1121 | X: | `label` | `Kanban` |
| 1123 | A: | `aria-label` | `Utility shortcuts` |

### `src/workbench/workspace_settings_pane/mod.rs`  (2 strings)

| Line | Pos | Kind | String |
|---:|---:|:---|:---|
| 369 | txt: | `text` | `Architecture map` |
| 382 | txt: | `text` | `LLM prose ingest` |
