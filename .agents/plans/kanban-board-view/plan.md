# Workspace Multi-Kanban Board

## Summary

Build a workspace-scoped Multi-Kanban as a pinned center tab for every real workspace.

- Add a pinned `Kanban` center tab with ID `0`.
- Keep the existing `Terminals` tab ID `1` as the active/default loaded view.
- Plan states stay derived exactly like the current Plans tab from task summaries: blocked > in progress > pending > completed > cancelled > empty.
- Task states remain the existing canonical statuses: pending, in progress, blocked, completed, cancelled.
- Plan Markdown under `.agents/plans/<slug>/plan.md` remains the source of truth for plan/task content.
- Kanban persistence stores only layout and metadata.
- Export/import covers Kanban layout and metadata, not plan Markdown contents.

## Decisions

- Plan state is not an independently editable field for v1. It is derived from the plan's task summary to stay 1:1 compatible with the existing Plans tab.
- Kanban export/import is layout plus metadata only. It must not write or overwrite plan Markdown contents.
- The Kanban tab is pinned, non-closeable, and always present as tab `0` for real workspaces.
- The Terminal tab remains the active tab after workspace creation, workspace restore, and migration from old snapshots.
- Plan Markdown stays the durable source of truth; Kanban layout metadata never stores full plan bodies.
- Remote workspaces use the same API contract only where existing workspace file access supports the required `.agents` paths. If not supported in v1, the UI shows a localized limited/unsupported state.

## Implementation Notes

### Phase 1: Data Model, Persistence, And Migration

- Extend center-tab state:
  - Add `CenterTabKind::Kanban`.
  - Add `CENTER_KANBAN_TAB_ID = 0`.
  - Change `default_center_tabs()` to `[Kanban, Terminals]`.
  - Keep `default_center_active_tab_id()` as `CENTER_TERMINALS_TAB_ID`.
  - Add repair/backfill logic so old snapshots automatically insert missing Kanban tab at index `0` and preserve the active terminal tab.
  - Make Kanban pinned and non-closeable; Terminals keeps the existing close-workspace behavior.

- Add workspace Kanban storage:
  - New backend module, e.g. `src-tauri/src/kanban.rs`.
  - Store at `<workspace>/.agents/kanban/index.json`.
  - Bootstrap `.agents/kanban/README.md` and `index.json` through `agents_layout.rs`.
  - Schema fields: `version`, `workspaceRoot`, `planSectionOrder`, `collapsedPlanSections`, `expandedPlans`, `taskLaneOrder`, `collapsedTaskLanes`, `planOrder`, `taskOrder`, `filters`, `updatedAt`.
  - Validate workspace path, prevent traversal, write JSON atomically, and offload I/O with `proc::run_blocking`.

### Phase 2: Backend Kanban APIs

- Add typed Tauri commands and register them in `src-tauri/src/lib.rs`:
  - `kanban_board_load(workspace_cwd)` returns plan metadata, parsed plan tasks, task-store mirror info, and persisted layout.
  - `kanban_layout_save(workspace_cwd, layout_patch)` updates only Kanban metadata.
  - `kanban_task_create(workspace_cwd, plan_path, status, title)` appends to the plan's `## Tasks`.
  - `kanban_task_update(workspace_cwd, plan_path, task_id, patch)` updates title/status and rewrites the plan task section.
  - `kanban_task_delete(workspace_cwd, plan_path, task_id)` removes the task line.
  - `kanban_import_layout(workspace_cwd, json)` validates and writes layout metadata.
  - `kanban_export_layout(workspace_cwd)` returns the export JSON.

- Reuse existing plan parser/writer semantics:
  - Same task markers: `[ ]`, `[>]`, `[!]`, `[x]`, `[-]`.
  - Exclude protected `PLANS.md`.
  - Keep `PlanMeta`, `PlanTaskSummary`, and current derived plan grouping behavior.
  - Best-effort sync mirrored plan-linked runtime tasks after Kanban task status/title changes.

### Phase 3: Frontend Center-Tab UI

- Add `src/workbench/workspace_kanban/` with component-local CSS.
- Render it from `DynamicCenterPanels` for `CenterTabKind::Kanban`.
- UI shape:
  - Modern tree-kanban, not classic horizontal columns.
  - Top-level collapsible plan-state sections: Blocked, In progress, Pending, Completed, Cancelled, Empty.
  - Plan rows/cards inside each section with icon, title, path, task counts, modified time, and quick actions.
  - Expanded plan row shows nested task-state lanes with task cards.
  - Icons via `icondata`/lucide: board, folder/tree, circle states, chevrons, plus, save, import/export, refresh, trash.
  - Token-only styling using `--accent`, `--warning`, `--success`, `--danger`, `--overlay-*`, `--radius-*`.

- Include all existing Plans-tab capabilities:
  - Create plan.
  - Rename plan.
  - Delete plan except `PLANS.md`.
  - Edit/preview plan body via existing Plans panel flow or a focused Kanban detail drawer.
  - Load plan into BLXCode Agent.
  - AI Plan / AI Tasks entry points may remain in Plans panel, but Kanban must expose navigation/actions so feature parity is reachable.
  - Refresh, search, filter, empty states, loading states, and errors.

### Phase 4: Kanban Interactions

- Implement core Kanban functions:
  - Expand/collapse plan-state sections.
  - Expand/collapse plans.
  - Reorder plans within their derived state section, persisted as metadata only.
  - Reorder task cards within a task lane, persisted as metadata.
  - Drag task cards between task-status lanes; write status back to Markdown.
  - Quick-add task into a selected plan/status.
  - Inline rename task title.
  - Delete task with confirmation.
  - Search by plan title/path/task title.
  - Filter by status, active-only, blocked-only, completed visibility.
  - Refresh from disk.
  - Import/export layout JSON.

- Do not allow direct manual plan-state mutation because plan-state is derived from task summaries.
- If moving a task changes the plan's derived state, the plan moves to the correct top-level section after save.

### Phase 5: Agent, Skills, Rules, And Notifications

- Extend Agent tool catalog:
  - Add Kanban read/write tools to `agent/tools.rs`.
  - Add tools to `ToolGroup::PlansRead` / `PlansWrite`.
  - Add permission classes for mutating Kanban tools.
  - Update `system_prompt.rs` tool index with Kanban tools.
  - Update `harness_skills/plans.md` or add a focused `kanban.md` core skill.

- Agent-facing tool set:
  - `kanban_board_load`
  - `kanban_layout_save`
  - `kanban_task_create`
  - `kanban_task_update`
  - `kanban_task_delete`
  - `kanban_export_layout`
  - `kanban_import_layout`

- Add or update `.agents/rules` guidance so agents:
  - Treat plan Markdown as source of truth.
  - Keep Kanban, plan tasks, and runtime tasks synchronized.
  - Use notifications for completed plans/tasks, blockers, and import/export failures.

- Notification integration:
  - Add target `{ "view": "kanban" }`.
  - Bell click opens the active workspace Kanban center tab.
  - Kanban mutations can create/update notifications for blocked tasks, completed tasks, and completed plans.
  - Add icon handling for Kanban-related notification targets.

### Phase 6: Titlebar, i18n, Docs, And Theme

- Add Navigate menu shortcut:
  - New menu item: "Workspace Kanban".
  - Enabled only with active real workspace.
  - Calls `open_center_kanban_tab(active_workspace_id)`.
  - Uses a board/layout icon.

- Add i18n keys to `I18nKey` and all locale tables:
  - `TabKanban`
  - `TbNavKanban`
  - Kanban toolbar, import/export, filters, empty states, errors, confirmations, drag labels, task actions, plan section labels.
  - Use existing plan/task status keys where possible.

- Update docs:
  - `docs/user/workspaces.md`: Kanban as pinned tab `0`, terminal still active by default.
  - `docs/user/plans.md`: Multi-Kanban behavior and layout export/import.
  - `docs/developer/architecture.md`: backend module, storage, data flow.
  - Fix the existing doc mismatch by stating plan-state is derived from task summaries, not independently edited.

## Tests

- Backend:
  - `cargo test -p blxcode kanban`
  - `cargo test -p blxcode plans`
  - Verify invalid paths, invalid import JSON, protected `PLANS.md`, empty plans, and multiple plans with duplicate task IDs in different files.
  - Verify Kanban metadata survives reload and does not overwrite plan Markdown.

- Frontend:
  - `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
  - `cargo check -p blxcode`
  - Theme token lint: `scripts/lint_theme_tokens.sh`.

- Manual:
  - New workspace opens with Kanban tab `0` visible and Terminals tab active.
  - Existing workspace snapshot is migrated without changing active tab unexpectedly.
  - Navigate menu switches to Kanban.
  - Drag task status writes correct Markdown marker.
  - Import/export round-trips layout only.
  - Bell notification target opens Kanban.
  - Locale switch shows no English-only missing UI strings except intentionally untranslated generated content.

## Tasks

- [x] `kanban-tab-model` - Add `CenterTabKind::Kanban`, tab ID `0`, default tab insertion, snapshot repair, and pinned close behavior.
- [x] `kanban-workspace-bootstrap` - Add `.agents/kanban/` bootstrap and status checks.
- [x] `kanban-storage-schema` - Implement validated, atomic layout metadata load/save/export/import.
- [x] `kanban-bridge-types` - Add frontend wire types and typed `tauri_bridge.rs` wrappers.
- [x] `kanban-board-load` - Aggregate non-index plans, derived plan buckets, parsed tasks, and layout metadata.
- [x] `kanban-task-create` - Append canonical task lines to a plan.
- [x] `kanban-task-update` - Update task title/status and sync mirrored runtime task best-effort.
- [x] `kanban-task-delete` - Remove a task line without disturbing other Markdown sections.
- [x] `kanban-import-export` - Validate schema version and round-trip layout metadata.
- [x] `kanban-component-shell` - Add workspace Kanban component folder and render it as center tab.
- [x] `kanban-tree-sections` - Build collapsible plan-state sections with icons/counts.
- [x] `kanban-plan-rows` - Render plan rows with task summary, actions, expansion, and derived state accents.
- [x] `kanban-task-lanes` - Render nested task-state lanes and cards.
- [x] `kanban-toolbar` - Add search, filters, refresh, import, export, and quick-create actions.
- [x] `kanban-plans-panel-sync` - Keep Workspace Kanban and the right-side Plans panel synchronized after create, write, rename, delete, update, remove, and drag/move actions.
- [x] `kanban-expand-collapse` - Persist expanded plans and collapsed sections/lanes.
- [x] `kanban-task-dnd` - Drag tasks across lanes and within lanes.
- [x] `kanban-plan-order` - Persist manual plan ordering within derived state groups.
- [x] `kanban-inline-actions` - Quick-add, rename, delete, load into Agent, open/edit/preview.
- [x] `kanban-error-states` - Handle stale files, missing plans, write conflicts, invalid imports, and no-workspace state.
- [x] `kanban-agent-tools` - Add Kanban tools, schemas, dispatch, tool groups, and permission classes.
- [x] `kanban-agent-skill` - Update embedded skill/rule guidance for Kanban use.
- [x] `kanban-system-prompt` - Add Kanban tools to the prompt index and tests.
- [x] `kanban-notifications` - Add `{view:"kanban"}` target handling and Kanban notification semantics.
- [x] `kanban-titlebar-shortcut` - Add Navigate menu item for active workspace Kanban.
- [x] `kanban-i18n` - Add exhaustive locale keys and run frontend check.
- [x] `kanban-theme-css` - Use only semantic tokens and component-scoped CSS.
- [x] `kanban-docs` - Update user/developer docs and fix plan-state wording.
- [x] `kanban-tests` - Add backend parser/storage/API tests and frontend compile checks.
- [x] `kanban-manual-qa` - Verify app restart/reload, new workspace defaults, old snapshot migration, DnD, import/export, notifications, and titlebar navigation.
