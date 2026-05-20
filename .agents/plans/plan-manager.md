# BLXCode Plan Manager

**Status:** planned

## Summary

Add a first-class Plan Manager for `<workspace>/.agents/plans/`, shown as a
new right-panel tab before **Memory**. Plans stay Markdown-first, with
standardized task sections that sync into the existing task manager.

The shared context path must include plans and plan tasks, not only memory and
images, so loaded plans are available to both the BLXCode Agent and terminal
agents.

## Key Changes

- Extend `.agents` bootstrap to create `.agents/plans/` and seed `PLANS.md` if
  missing.
- Add sandboxed plan CRUD and sync tools: `plan_list`, `plan_read`,
  `plan_create`, `plan_write`, `plan_delete`, `plan_rename`, `plan_load`, and
  `plan_sync_from_tasks`.
- Add a right-panel **Plans** tab before **Memory**.
- Build the Plans **Manage** view like Memory **Files**: plan list/sidebar,
  selected Markdown editor, preview toggle, rename/delete, create, refresh, and
  load action.
- Extend task records with optional `planPath` and `planTaskId`; extend task
  snapshots/store with optional `activePlanPath`.
- Agent task UI groups plan-linked tasks by plan first, then free tasks in a
  separate **Free Tasks** group.

## Shared Context

- Extend `AgentContextKind` on frontend/backend with plan-aware kinds:
  - `PlanIndex`
  - `PlanFile`
  - `PlanTaskGroup`
- When a plan is loaded via UI or `plan_load`, automatically upsert a shared
  context item:
  - id: `plan:<path>`
  - kind: `PlanFile`
  - label: plan title
  - source: plan path plus task count/status summary
  - paths: `[path]`
- Attach current plan-linked task state as context metadata for that plan, so
  status counts and active/in-progress task are visible without rereading the
  whole store.
- Update the Agent Context panel copy and rows so attached Plans appear
  alongside Memory, Terminal, and Image context.
- Add client tools:
  - `plan_context_list`
  - `plan_context_attach { path }`
  - `plan_context_detach { id }`
- Keep existing Memory context tools intact; do not overload
  `memory_context_attach` with plans.

## Terminal And Agent Handoff

- Update `render_context_prompt` for BLXCode Agent turns so attached plan
  context is explicitly rendered as plans/tasks, not generic memory text.
- Update terminal handoff renderer to include a new section:
  - `## Attached plans / tasks`
  - plan path/title
  - task status counts
  - active/in-progress task
  - compact task list for loaded plan tasks
- Extend `harness.send_agent_context.includeKinds` from `["memory", "images"]`
  to `["memory", "plans", "tasks", "images"]`.
- Default `includeKinds` should include `memory`, `plans`, `tasks`, and
  `images`.
- The Plans tab "Load into BLXCode Agent" button should load tasks, attach plan
  context, and make terminal handoff immediately include that plan/task state.

## Agent System Prompt

Update `src-tauri/src/agent/system_prompt.rs` with a **Workspace plans**
section explaining:

- Plans live in `.agents/plans/` and are durable Markdown plans.
- Use `plan_list`/`plan_read` before guessing about existing plans.
- Use `plan_create`/`plan_write` to create or update plans.
- Call `plan_load` whenever opening/loading/working from a plan, including
  plans the Agent just created.
- `plan_load` syncs plan tasks into the task manager and attaches the plan to
  shared context.
- `plan_*` tools manage durable plan files; `task_*` tools manage execution
  state.
- Plan-linked tasks and free tasks are separate in the UI.
- Attached plans/tasks are shared with terminal agents through
  `harness.send_agent_context`.
- Keep `PLANS.md` as the index and never delete it.

## Task Format

```markdown
## Tasks

- [ ] `task-id` - Pending task title
- [>] `task-id` - In-progress task title
- [!] `task-id` - Blocked task title
- [x] `task-id` - Completed task title
- [-] `task-id` - Cancelled task title
```

- `## Tasks` is canonical; `## Todos` is accepted.
- `plan_load` replaces only tasks where `planPath == path`; free tasks stay
  untouched.
- `task_update` on plan-linked tasks writes status changes back into the plan
  Markdown.

## Test Plan

- Rust tests for plan CRUD, parser/status round trips, path sandboxing,
  `PLANS.md` seed/protection.
- Task sync tests proving plan tasks are replaced per plan and free tasks
  survive.
- Shared context tests proving `plan_load` attaches `PlanFile` context and
  includes current task state.
- Handoff renderer tests for memory-only, plan-only, tasks-only, mixed context,
  and `includeKinds`.
- Agent prompt/tool catalog tests include all `plan_*` and `plan_context_*`
  tools.
- Frontend smoke: Plans tab before Memory, Manage view mirrors Memory Files,
  load groups tasks and updates shared context.
