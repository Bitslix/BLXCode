# Task Tracking

Live task execution state stored at `<workspace>/.blxcode/tasks/index.json`. Survives workspace reload and OS exit.

Use tasks for multi-step work. Prefer plan-linked tasks (via `plan_load`) for structured implementation; use free tasks for ad-hoc tracking.

## Tools (server-side)

### `task_list { status?, includeCompleted? }`
Lists tracked tasks as a stable JSON snapshot sorted by position. `status` filters to a single status; `includeCompleted` includes done/cancelled tasks (default `false`).

The snapshot includes `activePlanPath` — if set, that plan is the current active work.

### `task_get { id }`
Reads one task by id.

### `task_create { title, description?, status?, parentId?, notes? }`
Creates a task. `status` defaults to `pending`. Use for complex, multi-step work that needs structure. Skip for trivial one-step answers.

### `task_update { id, title?, description?, status?, parentId?, notes? }`
Updates one task. Call this as you make progress — especially when a task becomes `in_progress`, `blocked`, or `completed`.

For plan-linked tasks, `task_update` with a new `status` automatically writes the change back into the plan Markdown.

### `task_delete { id }`
Removes a task that is obsolete. Prefer `status: "cancelled"` for audit trails.

### `task_reorder { orderedIds }`
Rewrites task ordering using the full list of task ids.

## Status values
`pending` · `in_progress` · `blocked` · `completed` · `cancelled`

## Notes format
Notes support Obsidian-style `[[wikilinks]]` and `#tags` — both are indexed by the harness graph view.

## Guidance
- Call `task_list` early on complex work and keep state current via `task_update`.
- Reuse and update existing tasks instead of duplicating when the user expands ongoing work.
- Do not create throwaway tasks for trivial single-step answers.
- Plan-linked tasks (non-null `planPath` + `planTaskId`) and free tasks are shown separately in the UI.
