# Plan Folder Migration

## Summary

Migrate BLXCode plans from flat files under `.agents/plans/*.md` to per-plan folders at `.agents/plans/<slug>/plan.md`. The root `PLANS.md` and `README.md` remain root files. Existing flat plans must be migrated automatically in the app, asynchronously off the main thread, with a compact progress indicator in the left statusbar slot similar to the update progress indicator.

The implementation must apply the Tauri v2, Rust best practices, Rust async patterns, and Rust testing skills. After every implementation phase, update this plan with current status and findings, run the phase checks, then commit the phase.

## Decisions

- Canonical plan file path: `<slug>/plan.md`.
- Accepted API/tool inputs: slug (`foo`), legacy file (`foo.md`), and canonical path (`foo/plan.md`).
- Public API/tool outputs use canonical paths only.
- Migration is automatic and workspace-scoped.
- Migration work must run in a backend background task and must not block the Tauri main thread.
- Statusbar left slot shows migration progress while active and an error state if migration fails.
- Root `.agents/plans/PLANS.md` and `.agents/plans/README.md` are not migrated into folders.
- Sidecar files inside `.agents/plans/<slug>/` are allowed for future use and must not be treated as separate plans.

## Implementation Notes

- Backend plan model:
  - Add one central path normalizer that converts `foo` and `foo.md` to `foo/plan.md`, accepts existing `foo/plan.md`, and rejects traversal or non-plan Markdown targets.
  - Extend `PlanMeta` with `slug` and `folder_path`; keep `path` as canonical `foo/plan.md` and `name` as slug.
  - Update list/index/create/read/write/delete/load/rename/sync behavior to use canonical plan files only.
  - Rename moves the plan folder and rewrites plan references.
- Migration:
  - Add a workspace-scoped migration state and Tauri commands to start/poll migration progress.
  - Run file moves and JSON rewrites through `tauri::async_runtime::spawn_blocking` or equivalent non-main-thread work.
  - Move each root plan file except `PLANS.md` and `README.md` into `<slug>/plan.md`.
  - Resolve collisions deterministically with suffixes such as `foo-2/plan.md`.
  - Rewrite task state, active plan paths, Kanban layout path keys, and workbench plan context references to canonical paths.
  - Rebuild/sync `PLANS.md` after migration while preserving existing status and description where possible.
- UI:
  - Add a frontend `PlanMigrationService` modeled after `UpdateService`.
  - Show compact progress in the left app statusbar slot, e.g. `Plans 7/31`, and refresh plan/Kanban/core status views when migration completes.
  - Update Plans Panel, AI plan generation, Kanban, task lists, handoff/context rendering, and drag/drop to display and use canonical plan paths.
- Agent, tools, skills, prompts, docs:
  - Update `plan_*`, Kanban task tools, system prompt, core plan skill, core tasks skill, specialized architect/coordinator prompts, and subagent-related plan wording.
  - Update local `.agents/skills/manage-plans` skill and its agent prompt.
  - Update i18n placeholders and current user/developer docs.
  - Implement or remove any advertised but missing plan context tool handlers; preferred outcome is implementation with canonical path normalization.

## Tests

- Unit tests for path normalization, canonical metadata, legacy aliases, traversal rejection, and sidecar exclusion.
- Migration tests for flat plans, already-migrated plans, collisions, index preservation, and idempotent reruns.
- Task and Kanban tests for canonical path rewrites and markdown task writeback.
- Frontend tests or focused checks for Plans Panel create/rename/load/delete, AI save, Kanban plan selection, task grouping, context handoff, and statusbar progress.
- Agent tool tests for old and new input paths with canonical output paths.
- Phase checks must include the relevant Rust/Tauri checks before each commit; document any unavailable or failing checks in this plan before committing.

## Progress

- 2026-06-03 Phase 1 complete:
  - Added canonical backend path normalization for `foo`, `foo.md`, and `foo/plan.md`.
  - Added `slug` and `folder_path` to backend `PlanMeta`.
  - Updated backend plan CRUD/list/load/sync/rename behavior to return canonical paths while preserving legacy input compatibility.
  - Updated index sync to list canonical plan files only and preserve curated rows from legacy `foo.md` links.
  - Added sidecar exclusion coverage so nested Markdown files beside `plan.md` are not listed as plans.
  - Check: `cargo test -p blxcode --locked` passed with 311 tests.
- 2026-06-03 Phase 2 complete:
  - Added workspace-scoped backend migration state and `plan_migration_ensure_started` / `plan_migration_poll` Tauri commands.
  - Added async background migration for root legacy plan files, including deterministic target selection and progress fields.
  - Rewrites legacy task paths, active plan path, Kanban expanded/order state, and `PLANS.md` links/descriptions during migration.
  - Added frontend `PlanMigrationService` and a compact left statusbar indicator for busy/error migration state.
  - Check: `cargo check -p blxcode-ui --locked` passed.
  - Check: `cargo check -p blxcode --locked` passed with existing `ProviderEnv::from_environment` dead-code warning.
  - Check: `cargo test -p blxcode --locked` passed with 312 tests.

## Tasks

- [x] `phase-1-backend-path-model` - Implement canonical plan path model and backend CRUD/list/index behavior, update this plan, run checks, and commit phase 1.
- [x] `phase-2-async-migration-statusbar` - Implement async migration, reference rewrites, and statusbar progress, update this plan, run checks, and commit phase 2.
- [ ] `phase-3-ui-agent-docs-skills` - Update UI surfaces, agent tools/prompts, skills, i18n, and docs, update this plan, run checks, and commit phase 3.
- [ ] `phase-4-tests-repo-migration` - Complete tests, migrate repository plan files to folder layout, update this plan, run final checks, and commit phase 4.
