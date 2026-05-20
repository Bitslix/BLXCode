# Changelog

All notable changes to BLXCode are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

- Right panel: native hover tooltips (`title`) on icon rail tabs and header tab strip (inactive tabs show icon-only).
- Sidebar footer brand label is **BLXCode** (was lowercase `blxcode`).
- Plans panel: `plans-panel.css` is now loaded via Trunk (was edited but never linked); task-summary gap/colors use `!important` so they override stale rules in `styles.css`.
- Plans panel: plan list column is **drag-resizable** (default 288px, persisted `blxcode_plans_list_width_px_v1`); list rows are more compact (less padding, tighter chips).

### Removed

## [0.1.12] - 2026-05-20

### Added

- **Plan Manager**: durable Markdown plans under `<workspace>/.agents/plans/`, with `PLANS.md` as a protected index seeded on workspace bootstrap. Each plan can declare a canonical `## Tasks` (or `## Todos`) section using the syntax `- [ ] \`task-id\` - title` where the marker is one of `[ ] [>] [!] [x] [-]` (pending/in-progress/blocked/completed/cancelled).
- **Plans tab** in the right panel (between Browser and Memory): list with per-plan task summary, Markdown editor with debounced auto-save, preview toggle, create/rename/delete, and a "Load into BLXCode Agent" action that syncs plan tasks into the task manager and attaches the plan to shared context. On workspace activation, the panel auto-opens the last-active plan via the persisted `activePlanPath`.
- **Plan-linked tasks**: `TaskRecord` gains optional `planPath` / `planTaskId`; `TaskSnapshot` gains `activePlanPath`. `plan_load` replaces only tasks where `planPath == path` and leaves free tasks untouched. `task_update` on plan-linked tasks writes status changes back into the plan Markdown automatically. The Agent panel's task list groups plan-linked tasks by plan first, then renders a separate **Free Tasks** group.
- **Agent tools**: `plan_list`, `plan_read`, `plan_create`, `plan_write`, `plan_delete`, `plan_rename`, `plan_load`, `plan_sync_from_tasks` (server-side), and `plan_context_list`, `plan_context_attach`, `plan_context_detach` (client-side).
- **Tauri commands**: `plan_list`, `plan_read`, `plan_create`, `plan_write`, `plan_delete`, `plan_rename`, `plan_load`, `plan_sync_from_tasks` under `src-tauri/src/plans.rs`.
- **Shared context kinds**: `AgentContextKind::{PlanIndex, PlanFile, PlanTaskGroup}` (mirrored in `src/agent_wire.rs`). `render_context_prompt` renders attached plans separately from memory, citing per-plan task counts.
- **Terminal handoff**: `harness.send_agent_context.includeKinds` accepts `"memory" | "plans" | "tasks" | "images"` (default: all four). Renderer adds a new `## Attached plans / tasks` section with per-plan status counts, the active/in-progress task title, and a compact task list. The handoff renderer is primed by a per-workspace task-snapshot cache populated by the Plans/Agent panels on workspace activation so the synchronous renderer surfaces restored plan state immediately after a reload.
- **System prompt — mandatory Turn checklist**: explicit ordered steps at the top of the prompt that the agent must run every turn — (1) `rules_list` + `rules_read` on relevant active rules as a binding first step, (2) `skills_list` + `skills_read` on matching skills when the task warrants, (3) **Resume check** that recognises continuation directives in English (*continue, keep going, resume, proceed*) and German (*weiter, fortsetzen, weitermachen, mach weiter*) and resumes from `task_list` / `activePlanPath`, (4) memory/project context as needed, (5) execute. Trivial conversational turns may skip steps 1–2.
- **Workspace plans system-prompt section**: explains plan tooling, `## Tasks` line syntax, the `task_*` vs. `plan_*` separation, automatic status write-back, and that plan files and the task store survive workspace reload / harness restart / OS exit — so `plan_list` + `task_list` reconstruct in-flight work authoritatively after a restart.
- **i18n**: `TabPlans` plus Plans-panel keys (`PlansEmptyTitle`, `PlansEmptyLead`, `PlansNewPlan`, `PlansNewPlanPh`, `PlansRename`, `PlansDelete`, `PlansSelectPlan`, `PlansEdit`, `PlansPreview`, `PlansLoadIntoAgent`, `PlansLoaded`, `PlansRefresh`, `PlansTaskSummary`, `PlansTaskStatTotal`, `PlansTaskStatPending`, `PlansTaskStatInProgress`, `PlansTaskStatBlocked`, `PlansTaskStatCompleted`, `PlansTaskStatCancelled`, `PlansProtectedIndex`) added to all 14 locales; `TabPlans` and `de_de` task-stat labels are translated, other locales fall back to English for the stat keys.
- **Tests**: 11 new plan tests covering CRUD round-trip, `PLANS.md` protection, path-traversal sandboxing, parser status markers + `## Todos` alias, `plan_load` replace-only-plan-tasks semantics, `task_update` status write-back to plan Markdown, `plan_sync_from_tasks` round-trip preserving non-Task sections. New handoff-renderer tests for the plans/tasks section. New system-prompt tests verifying the mandatory Turn checklist, all continuation keywords, and the persistence guarantee.
- **Sidebar layout — bottom panel**: Project Files and Git graph share a single `.workbench-sidebar__panels` block (default **50%** of sidebar height, not the full area below Workspaces). A horizontal drag handle between the workspace list and this block resizes the combined panel; the existing handle inside the block still splits Explorer vs. graph (`blxcode_sidebar_panels_height_pct_v1` / `blxcode_sidebar_explorer_height_pct_v1` in `localStorage`).
- **Sidebar width**: drag handle on the right edge of the sidebar (like the right panel splitter) resizes width in pixels; default **260px** (was `clamp(216px, 22vw, 280px)`). Persisted as `blxcode_sidebar_width_px_v1` and on `WorkbenchSnapshot.sidebar_width_px`.
- **Project Explorer — hidden files**: toolbar eye toggle (`LuEye` / `LuEyeOff`) shows or hides dot-prefixed entries client-side; preference stored as `blxcode_sidebar_explorer_show_hidden_v1` (default off).
- **Project Explorer — tree navigation**: clicking a folder row (name/icon area) expands or collapses the tree; the chevron still works and does not double-toggle.
- i18n: `SbExplorerShowHidden`, `SbExplorerHideHidden`, `SbExplorerResizeAria`, `SbPanelsResizeAria`, `SbWidthSplitterAria` — added to all 14 locales (`de_de`, `fr_fr`, `es_es` translated for the new strings).

### Changed

- Sidebar Explorer/graph: `SidebarResizer` supports top- vs. bottom-measured splits (`measure_from_bottom` for the workspace↔panels boundary); container basis for the inner split is `.workbench-sidebar__panels` instead of `.workbench-sidebar__views`.
- Workspaces nav in the sidebar uses remaining space above the bottom panel (`flex: 1 1 auto`) instead of a fixed `max-height: 32%`.

- `ensure_agents_layout` now also creates `.agents/plans/` and seeds `PLANS.md` if missing; `WorkspaceRoots` gains a `plans` field.
- `RightPanelTab` adds a `Plans` variant; the right-panel rail and tabstrip render it before Memory.
- `harness.send_agent_context` tool description updated: default `includeKinds` is `["memory", "plans", "tasks", "images"]`; the rendered Markdown block includes attached plans and plan-linked task state.

### Fixed

- Sidebar Project Explorer no longer occupies almost the full sidebar when the tree is empty: Explorer and graph are confined to the resizable bottom panel so the graph stays visible and the file tree does not “stick” at the top of an oversized slot.
- **Project Files** and **Graph** section expand/collapse state now restores correctly after app restart or workbench hydrate (per workspace via `sidebar_explorer_open` / `sidebar_graph_open` on `WorkspaceEntry`; default open on first load).
- Project Explorer: nested folder clicks no longer bubble to parent rows (fixing “second level closes the tree”); row hover uses a visible highlight (`rgba` instead of undefined `--bg-hover`).
- Sidebar: collapsed rail is **44px** wide (`min-width: 200px` no longer applies); Explorer/Git panels **snap to the bottom** above the footer and shrink to header height when both sections are minimized (no empty 50% gap).
- Plans panel: plan list rows show hover highlight; list/editor divider (`border-left` on the editor column) spans the full panel height; `.workbench-right-plans` dock fills the tab body so layout height propagates correctly.
- Plans panel: collapsible plan list sidebar (same control as Memory — `LuPanelLeftClose` / `LuPanelLeftOpen`, narrow `3.8rem` rail with plan initials badges).
- **Git commit graph (sidebar)**: lane layout assigns separate swim-lanes for forked branches (not everything on lane 0); `branch_from_lane` and `pass_through_lanes` wire fields; SVG uses cubic Bézier connectors for fork/merge and full-height pass-through lines; merge commits render as hollow nodes. Section title renamed to **Git Commits** (`SbGraphTitle`).
- Plans panel: plan list rows show **colorized task-summary chips** (Leptos icons + counts per status — pending, in-progress, blocked, completed, cancelled) instead of the ASCII summary string (`0 · 0p / 0>!0b / …`); zero counts are muted. New-plan toolbar uses `LuPlus` instead of a literal `+`.

### Removed

- Project Explorer toolbar action **Collapse all folders** (`SbExplorerCollapseAll`); replaced by the hidden-files eye toggle.


## [0.1.11] - 2026-05-20

### Added

- **Sidebar Explorer & Git graph** (VS Code–style view sections): collapsible panels at the bottom of the left sidebar (above the version footer), with the workspace list scrolling independently above.
- **Project Explorer** section: lazy file tree for the active workspace `cwd` (directories and files via Tauri `list_path_entries`, sandboxed under the workspace root); refresh and collapse-all toolbar actions; expanded paths and open/collapsed state persisted per workspace.
- **Git graph** section: commit history with swim-lane SVG layout, ref badges, and author/time metadata from `git_commit_graph` (up to 100 commits, `git log --topo-order`); shown only when `.git` is detected (`git_is_repository`); if Git is not on `PATH`, the section stays visible with an i18n hint (`SbGraphGitMissing`) instead of an empty graph.
- Reusable Leptos component `SidebarViewSection` (`src/workbench/sidebar_view_section/`) with optional toolbar row (hover-reveal) and persisted `sidebar_explorer_open` / `sidebar_graph_open` on `WorkspaceEntry`.
- Tauri: `list_path_entries`, `git_is_repository`, `git_commit_graph`; backend modules `fs_entries` and `git_graph` (lane layout unit-tested).
- i18n: `SbExplorerTitle`, `SbGraphTitle`, `SbSectionExpand`, `SbSectionCollapse`, `SbExplorerRefresh`, `SbExplorerCollapseAll`, `SbGraphRefresh`, `SbExplorerNoCwd`, `SbExplorerTauriOnly`, `SbGraphLoadError`, `SbGraphGitMissing` — added to all 14 locales (German fully translated for sidebar explorer/graph strings).
- **Tmux-style keyboard shortcuts** (default): prefix `Ctrl+b`, then a second key — `o` quick open, `p` toggle right panel, `a` / `b` / `m` agent / browser / memory tabs, `n` new terminal slot (active workspace only), `:` command palette. Prefix times out after 1.5 s; `Esc` cancels an armed prefix.
- **Classic (legacy) shortcut mode**: restores direct chords (`Ctrl+O`, `Ctrl+P`, `Ctrl+Shift+A/B/M/P`, `` Ctrl+` `` new terminal, `Ctrl+Shift+P` palette). Selectable in **BLXCode Settings → App → Keyboard shortcuts**; persisted as `blxcode_shortcut_mode_v1` (`tmux` | `legacy`, default `tmux`).
- Frontend module `harness_chords` (`handle_harness_keydown`, `dispatch_shortcut_action`, `open_new_terminal`, `ShortcutKeys` display helpers) wired from `HarnessHost`; welcome-screen shortcut list reflects the active mode (`Ctrl+b` **then** key vs. combined keys).
- i18n: `WsKwThen`, `AppShortcutHeading`, `AppShortcutModeTmux`, `AppShortcutModeLegacy`, `AppShortcutModeHint` — added to all 14 locales (German fully translated for shortcut settings).
- Success **toasts** for terminal/memory handoff actions: a lightweight toast stack (bottom-right) confirms when context is sent to a terminal or attached to the BLXCode Agent; errors always show an error toast regardless of the success-toast toggle.
- Optional **success sound** on handoff (short Web Audio beep, same timbre as terminal-hook notifications) — independent of the toast toggle.
- **BLXCode Settings → App → Notifications**: checkboxes to enable/disable success toasts and success sounds (`blxcode_success_toast_v1` / `blxcode_success_sound_v1` in `localStorage`, default on).
- Frontend modules `toast` (`ToastService`, `ToastHost`) and `app_prefs` (`AppPrefsService`) wired in `WorkbenchShell`.
- i18n: `HandoffOkAttached`, `HandoffNoActiveWorkspace`, `AppNotifHeading`, `AppNotifToasts`, `AppNotifToastsHint`, `AppNotifSound`, `AppNotifSoundHint` — added to all 14 locales; `HandoffToAgentContext` shortened to **Send to BLXCode Agent** (German: *An BLXCode-Agent senden*).

### Changed

- `` Ctrl+` `` (legacy) and **Ctrl+b** `n` (tmux) now open a **new terminal slot** via `append_terminal_slot` and focus it — no longer only reveal the Agent tab.
- In tmux mode, `Ctrl+b` is not intercepted while a workspace terminal has focus (PTY/shell keeps the prefix).
- Handoff feedback is **centralized in `HandoffMenu`**: Graph preview no longer shows an inline green/red status strip under the titlebar; the terminal titlebar handoff button no longer flips to check/alert icons — both rely on global toasts (+ optional sound) instead.
- `note_context_item` and workspace Memory-category attach now set `added_at` from `Date::now()` (consistent with the Memory panel context menu).

## [0.1.10] - 2026-05-20

### Added

- Terminal agent context handoff: new BLXCode Agent client tool `harness.send_agent_context { slotId? | agentSlug?, instruction?, includeKinds?, submit? }` that hands off the active workspace's attached Memory/Learnings/Images to a live `claude` / `codex` / `gemini` / `opencode` / `cursor` CLI session in the workbench terminal grid. Renders a terminal-safe Markdown block (workspace root, attached items, image paths, optional instruction), writes it through the existing PTY path, and submits by default. Image bytes are exported to disk and cited by path — base64 is never written into the prompt.
- Image export pipeline for context handoff: new Tauri command `agent_export_context_images` writes selected images to `<workspace>/.blxcode/agent-context/images/` with sanitized filenames and a JSON manifest sidecar; collision-safe (`-2`, `-3` suffix) and idempotent across retries.
- Terminal PTY env vars `BLX_AGENT_CONTEXT_DIR` and `BLX_AGENT_CONTEXT_MANIFEST` so hooks and the spawned CLI agent can discover the workspace handoff directory and manifest path. Hooks remain advisory — the explicit toolcall is the only transport.
- Terminal titlebar **handoff dropdown** on every workspace terminal cell: opens a menu listing every live terminal in the workspace (so context can be forwarded to any peer slot), a separator, and a "Send to BLXCode agent context" entry that attaches the workspace's Memory category to the BLXCode Agent context (idempotent upsert). Button icon briefly flips to a check or alert and the border tints green / red as visual feedback (~2.8 s).
- Memory **Graph preview** handoff button: the note preview popover gains a terminal-share icon (next to "Open in Files" / "Close") that opens the same dropdown. Picking a terminal sends ONLY this note as a one-shot `memory_note` / `learning_note` context item; the "Send to BLXCode agent context" entry attaches the previewed note to the BLXCode Agent context.
- Shared frontend module `agent_context_handoff` containing the Markdown renderer (`render_agent_context_block`), the `WorkspaceTerminalTarget` listing, the async `perform_handoff` helper, and the reusable Leptos components `HandoffMenu` + `TerminalSlotHandoffButton`. The renderer is the single source of truth for the prompt shape and is fully unit-tested (empty / memory-only / image-only / mixed-with-instruction / long-path collapsing).
- `pty_sessions_signal()` accessor on `WorkbenchService` so UI components can react to live PTY session registration (button enables the instant the cell registers; no stale-disabled states).
- i18n keys for the new UI: `MemGraphSendToTerminal`, `HandoffSendContext`, `HandoffPickTerminal`, `HandoffNoTerminals`, `HandoffOkSent`, `HandoffFailed`, `HandoffToAgentContext` — German strings translated, all 12 other locales stubbed with the English fallback.
- Agent Panel image context: attach PNG, JPEG, WebP, or GIF images to the BLXCode Agent via OS/browser drag-and-drop or paste.
- Agent Panel drop-zone feedback: dragging images over the Agent chat highlights the pane with a dashed border and helper text; unsupported drops show a rejection hint.
- Agent Context image rows: attached images show Pending/Read status, remove controls, manual “use again” reactivation, and a preview dialog with close/remove actions.
- BLXCode Agent multimodal provider integration: pending images are sent once through OpenAI/OpenRouter and Anthropic vision payloads, then marked read via an `ImageContextConsumed` event.
- BLXCode Agent image context client tools: `image_context_list` and `image_context_detach`.
- Native image file validation command for dropped files, including MIME detection and per-image size limits.
- Right panel **Rules** tab: cards for every `.agents/rules/rule-*.md` with title, summary, enabled/disabled pill, toggle, read, and remove controls; refresh button auto-loads on tab activation and workspace switch.
- Right panel **Skills** tab: cards for every `.agents/skills/<name>/SKILL.md` with source badge (`git` / `npm` / `local` / `agent`), `SKILL.md missing` warn marker, toggle/remove controls, and an **Install skill** dialog.
- Skill install dialog: segmented `Git` / `npm` / `Local` source picker with name + per-source fields; submits through the new `skills_install` Tauri command, shows progress and per-attempt error inline.
- Tauri command surface for skills & rules: `rules_list`, `rules_read`, `rules_write`, `rules_set_enabled`, `rules_remove`, `skills_list`, `skills_read`, `skills_write`, `skills_set_enabled`, `skills_remove`, `skills_install`, and `skills_rules_bootstrap`.
- On-disk manifests `.agents/rules/index.json` and `.agents/skills/index.json` tracking enabled state and (for skills) source provenance; atomic writes via tmp + rename, self-heal removes orphan entries at read time.
- First-touch bootstrap: when a workspace is opened, the harness auto-creates `.agents/{rules,skills}/` and seeds each `index.json` from the on-disk content (every existing rule and skill folder enters as `enabled: true`, skills with `source.kind = "local"`); manually disabled entries survive subsequent bootstraps.
- Skill install pipeline: `git clone --depth=1 --single-branch` for `git`, `npm pack` + `tar -xzf --strip-components=1` for `npm`, recursive copy for `local`; every install stages into `.install.<name>.tmp/`, validates that `SKILL.md` is present at the top level, and rolls the staging dir back on any failure.
- BLXCode Agent server tools mirroring the Tauri commands: `rules_list`, `rules_read`, `rules_write`, `rules_set_enabled`, `rules_remove`, `skills_list`, `skills_read`, `skills_write`, `skills_set_enabled`, `skills_remove`, and `skills_install { name, source: { kind, url?, ref?, package?, version?, path? } }`.
- System-prompt section for workspace skills & rules: active rules are declared **binding and non-negotiable** and outrank skill guidance; disabled entries must be treated as if they did not exist; install/remove require explicit user requests; `index.json` is harness-managed, not hand-edited.
- i18n: new keys `TabRules`, `TabSkills`, `SrRulesEmpty`, `SrSkillsEmpty`, `SrEnable`, `SrDisable`, `SrInstallSkill`, install-dialog labels and placeholders, status pills (`enabled`/`disabled`), `SrMissingSkillMd`, and `SrNoWorkspace` — added to all 14 shipped locales.

### Changed

- BLXCode Agent system prompt now documents `harness.send_agent_context` next to `harness.send_terminal_keys`, instructs the agent to prefer the new tool for context-aware delegation, and warns against broadcasting context to multiple terminals without explicit user intent.
- `WorkspaceTerminalCell` accepts `slot_id` and `pane_id` props so the new handoff dropdown can be embedded directly in the titlebar (caller in `workspace_panel.rs` updated).
- Agent conversation history now sanitizes image content after a turn so base64 image bytes are not persisted or resent on later text/voice turns.
- Agent Chat reset moved from the compose action row into the Chat log header as an icon-only control with tooltip.
- Right panel rail and open-tabstrip now carry five tabs (Agent / Browser / Memory / Rules / Skills) with `LuShield` and `LuPuzzle` icons for the two new entries.
- `RightPanelTab` enum extended with `Rules` and `Skills` variants.

## [0.1.8] - 2026-05-20

### Added

- One-time dialog after the EULA asking whether to append `.blxcode/` to the workspace `.gitignore` (answer stored in `blxcode_gitignore_prompt_v1`; skipped in non-Tauri builds).
- Tauri command `workspace_ensure_agents` to create `.agents/memory/`, `.agents/learnings/`, and `_templates/` on workspace open.
- Tauri command `gitignore_append_blxcode` to add a `.blxcode/` entry when the user accepts the post-EULA prompt.
- Release CI workflow: `authorize` job — only the repository owner may run release builds (`workflow_dispatch` or `v*` tag push); org repos may set Actions variable `RELEASE_OWNER`.
- Release CI **manual run**: choose build targets — **Alle**, **Linux (deb, rpm, AppImage)**, **Mac Universal**, or **Windows** (`plan` job sets the matrix). Tag pushes still build all platforms.
- Memory **Files** sidebar: collapsible **Memory** / **Learnings** groups (chevron toggle, collapsed by default). Group headers open the category index note (`README.md` / `MEMORY.md` or `learnings/LEARNINGS.md`) and list other notes when expanded.
- Memory **Search** view: category filter badges (pill style) between the query field and results — **All**, **Memory**, and **Learnings** with hit counts; filters appear whenever there are hits (not only when both roots match); filters reset on a new query.
- Memory **Search** and note editor: **Show in graph** control jumps to the 3D graph, selects the note node, and flies the camera to it.
- Memory **Graph** view: lazy-loaded offline 3D graph powered by a local `3d-force-graph` / Three.js bundle, with the existing SVG graph retained as the 2D fallback.
- Memory **Graph** toolbar: icon-only controls for reset, zoom in/out, and 2D/3D mode switching; the selected graph mode is persisted locally.
- Memory **Graph** preview flow: clicking a node stays in the Graph tab, flies/focuses to the node, and opens a markdown preview popover with close, “Open in Files”, and in-preview wikilink navigation.
- Memory category context menu: right-click category headers to edit display name, category color, sidebar visibility, graph visibility, or send the category to the BLXCode Agent.
- Memory note context menu: right-click individual Memory/Learnings entries to open them or send that single note to the BLXCode Agent.
- BLXCode Settings → Memory: app-wide color preset management for Memory category colors, with add/edit/delete/reset controls.
- Agent Panel **Context** section: list of attached Memory/Learnings categories and notes, with per-item remove controls.
- BLXCode Agent **`list_tools`** server tool: returns JSON catalog of every registered tool (name, `server`/`client` site, description, parameters schema).
- BLXCode Agent **memory management** server tools: `memory_delete`, `memory_rename` (move/rename within one root, optional wikilink rewrite), `memory_graph`, and `memory_backlinks`.
- BLXCode Agent **memory UI** client tools: `memory_category_list`, `memory_category_update` (label, `#rrggbb` color, sidebar/graph visibility for `memory` / `learnings`), `memory_context_list`, `memory_context_attach`, and `memory_context_detach`.

### Changed

- Memory file list no longer repeats the `learnings/` folder label on every row; notes are grouped under their category instead.
- Memory graph labels are cleaned up for display (`tanstack-start-api-routes` → `Tanstack Start API Routes`) in 3D, 2D fallback, and preview titles.
- Memory graph node colors now respect workspace category settings and graph visibility toggles.
- Memory graph spacing and interaction feel more physical: 3D nodes spread farther apart, links curve subtly, and connected lines wobble when nodes or links are dragged/released.
- BLXCode Agent turns now include attached Memory context as compact path metadata before the user prompt, leaving file contents to existing workspace read tools.
- Workspace memory and learnings now live under `.agents/memory/` and `.agents/learnings/` (unified Memory API with `learnings/…` paths). Legacy `.blxcode/memory/` is migrated automatically when the new memory folder is empty. Existing learnings Markdown index links are upgraded to `[[wikilinks]]` for the memory graph.
- `.agents/` layout is bootstrapped when a workspace path is set (wizard commit, workspace switch, or workbench restore), not only when opening the Memory tab.
- Agent system prompt and memory tool descriptions reference `.agents/memory/` and `.agents/learnings/`, document full memory CRUD/graph/category/context tools, and recommend `list_tools` when schemas are unclear.
- Agent Panel **Tasks** and **Context** sections stay collapsed when empty and expand automatically when at least one task or context item is present (manual toggle still works until the count changes).
- Agent memory pointer blocks list both memory and learnings roots.
- Memory export/import uses `memory/` and `learnings/` subdirectories.
- Developer docs (`architecture`, `tauri-ipc`, `getting-started`, `memory-and-tasks`) updated for the `.agents/` memory layout, `workspace_ensure_agents`, and memory-flow diagrams.
- `scripts/render_i18n_locales_from_en.py` default mode translates only **missing** keys (not a full locale rewrite); use `--full` for the previous behavior, `--patch-english-matches` or `--keys` for English placeholder rows.

### Fixed

- Release CI `authorize` job reads optional `RELEASE_OWNER` via the GitHub API (`actions/github-script`) instead of `${{ vars.RELEASE_OWNER }}` in workflow expressions (avoids invalid context warnings when the variable is unset).
- i18n render script no longer rewrites locale files when nothing changed; prints guidance when zero rows are translated.

## [0.1.5] - 2026-05-19

### Added

- Session resume for hooked agent terminals (`sessions.json`) with captured titles on the terminal grid.
- Workspace and per-terminal completion notification badges with optional sound when background agents finish.
- Voice input and replies: microphone STT, OpenAI/OpenRouter transcription, and OpenAI TTS playback with a dedicated settings pane.
- Embedded browser navigation history and `browser_run_js` for running scripts in browser tabs.
- Workspace terminal improvements: multi-slot agent assignment, active/visibility controls, `pty_drain_wait`, terminal-wait-ready events, and agent launch retry.
- Workspace session UUIDs and notification pruning for agent tasks.
- User documentation: building guide, UI language guide, extra screenshots, and expanded README highlights.

### Changed

- First-run terms updated for MIT open-source distribution (MIT license acknowledgment, third-party API disclaimer); acceptance storage key bumped to `blxcode_eula_v2` so existing installs see the new text.
- Terminal layout and workspace grid observation for more responsive resizing.
- Workspace ID allocation and reset logic for stable session handling.

### Fixed

- Localization keys for unread workspace notifications.
- Potential deadlock when resetting workspace ID in terminal update closures.

## [0.1.0] - 2026-05-18

### Added

- Initial public release under the [MIT License](LICENSE).
- Native desktop shell (Tauri 2 + Leptos CSR) with multi-terminal workspaces, split panes, and persisted layout.
- Agent panel with OpenRouter, Anthropic, and OpenAI-compatible providers; sandboxed workspace file tools.
- **14-language UI** with compile-time translations and localized first-run terms.
- Workspace memory (`.blxcode/memory`), tasks (`.blxcode/tasks`), and memory graph view.
- Embedded browser with native child webviews on supported platforms and iframe fallback on Linux.
- Agent hooks for Claude, Codex, Gemini, OpenCode, and Cursor session/title capture.
- Command palette, Quick Open, drag-and-drop workspace reordering, and harness settings.
- EULA acceptance gate and platform app-config persistence.
