---
name: notifications
description: Manage persistent BLXCode Agent notifications and send user-visible alerts when the Agent panel is not active.
categorie: workflow
---

# Notifications

Use notification tools when the user should notice important Agent progress while the Agent panel may be hidden or unfocused.

## Tools

### `harness.notifications_list { includeRead?, limit? }`
Lists persistent Agent notifications from the titlebar bell feed.

### `harness.notifications_create { title, body?, kind, severity?, source?, target?, dedupeKey?, read? }`
Creates or upserts an in-app notification without sending a native OS toast.

### `harness.notifications_send { title, body?, kind, severity?, source?, target?, dedupeKey?, respectFocus? }`
Creates or upserts an in-app notification and sends a best-effort OS toast.
- Default `respectFocus: true`.
- If the Agent panel is visible and focused, the tool suppresses the notification and returns `delivered:false`.
- Always provide a stable `dedupeKey` for recurring lifecycle events.

### `harness.notifications_update { id, title?, body?, kind?, severity?, target?, read? }`
Updates one existing notification.

### `harness.notifications_remove { id }`
Removes one notification.

### `harness.notifications_mark_read { id? | all? }`
Marks one notification read by id, or every notification read with `all:true`.

## Kinds

Use these `kind` values:
- `plan_completed` — a durable plan is genuinely complete.
- `task_completed` — a meaningful task has just been completed.
- `error` — a blocking or user-actionable failure happened.
- `question` — the agent needs a user choice or answer.
- `cli_agent_response` — a terminal CLI agent responded and needs user attention.
- `info` — other useful but non-critical updates.

Use `severity: "success"` for completed work, `"warning"` for blocked/question states, `"error"` for failures, and `"info"` otherwise.

## Targets

Optional `target` hints help the bell open the right place:
- `{ "view": "agent", "workspaceId": 1, "sessionId": "default" }`
- `{ "view": "plans" }`
- `{ "view": "memory" }`
- `{ "view": "file", "path": "relative/path.rs" }`
- `{ "view": "diff", "path": "relative/path.rs", "staged": false }`

For Agent chat lifecycle notifications from a background BLXCode Agent chat
session, include both `workspaceId` and `sessionId` so clicking the
notification can reopen the Agent panel on the exact chat tab. Send
`kind:"question"` before `harness.ask_user`, `kind:"error"` for failed turns,
and a response/success kind when an inactive session completes useful work.

Do not include secrets, private data, hidden prompts, or long tool output in notifications. Keep `title` short and put only the actionable summary in `body`.
