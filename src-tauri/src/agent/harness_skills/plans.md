# Workspace Plans

Durable Markdown files that capture a multi-step implementation strategy. Lives at `<workspace>/.agents/plans/`. Plans are the long-lived counterpart to the task list and are checked into git.

`PLANS.md` is the protected index — never delete or rename it.

## Tools (server-side)

### `plan_list`
Overview of every plan with task summary counts. Call before guessing about existing plans.

### `plan_read { path }`
Reads one plan's Markdown body. `path` is relative to `<workspace>/.agents/plans/` (e.g. `my-plan.md`).

### `plan_create { path, content? }`
Creates a new plan. Path must end in `.md` and must not exist. Optionally seed with initial content.

### `plan_write { path, content }`
Overwrites an existing plan. Use `plan_sync_from_tasks` instead when you only need to update the `## Tasks` section.

### `plan_delete { path }`
Deletes a plan (cannot delete `PLANS.md`).

### `plan_rename { oldPath, newPath }`
Renames within `.agents/plans/`. Task records pointing at the old path are rewritten automatically.

### `plan_load { path }`
Syncs the plan's `## Tasks` (or `## Todos`) section into the task manager. Also attaches the plan to BLXCode Agent shared context (`PlanFile`) and sets `activePlanPath` on the snapshot.

**Call `plan_load` whenever you open or start working from a plan** — including plans you just created. Without it, plan tasks are not in the task manager.

### `plan_sync_from_tasks { path }`
Writes the current plan-linked task state back into the plan Markdown. Use after reordering or batch-status-changing plan tasks via `task_*` tools.

## Agent context (client-side)

### `plan_context_list`
Lists plans attached to BLXCode Agent shared context.

### `plan_context_attach { path }`
Attaches a plan (makes it visible to terminal CLI agents via `harness.send_agent_context`).

### `plan_context_detach { id }`
Detaches a plan from context by id.

## Task line syntax in `## Tasks`

```
- [ ] `task-id` - Pending task title
- [>] `task-id` - In-progress task title
- [!] `task-id` - Blocked task title
- [x] `task-id` - Completed task title
- [-] `task-id` - Cancelled task title
```

## Key semantics
- `plan_*` manage durable plan files; `task_*` manage live execution state. Both coexist for one plan.
- `task_update` on plan-linked tasks writes status back into the plan Markdown automatically.
- Plan files and the task store survive workspace reload, harness restart, and OS exit.
- Reference plans from `PLANS.md` with Markdown links.
