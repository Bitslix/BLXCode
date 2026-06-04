# Multi-Agent Chat Sessions

## Summary

Implement true parallel BLXCode Agent chat sessions per workspace on a new Git branch based on `stage` (for example `feature/multi-agent-chat-sessions`). The Chatlog header gets a new `+` icon for creating a session, and the Chatlog renders a subtle tab bar directly below the Chatlog header/titlebar. Each session has isolated chat-near state, while app restart restores tabs, timelines, drafts, and status metadata without automatically resuming interrupted provider streams.

## Decisions

- The implementer must read and apply the Rust, Tauri, and frontend design skills before changing code: `rust-best-practices` or an equivalent Rust skill, `tauri-v2`, and `frontend-design`.
- True parallelism is required: multiple BLXCode Agent chat sessions in the same workspace can run at the same time.
- Restart persistence restores session metadata and history only. Running HTTP/provider streams become idle/restored after restart and are not replayed automatically.
- Chat-near state is scoped per session: timeline, draft, busy/idle/ask/error/restored status, chat mode, image mode, enhance toggle, usage stats, pending approvals/toolcalls, and pending context/image attachments.
- Shared workspace sources remain workspace-scoped and attachable into the active session.
- Keep the current visual style, use theme tokens for all new styling, and provide full i18n for all new user-visible strings.

## Implementation Notes

- Extend `WorkspaceEntry` and `WorkbenchSnapshot` with `agent_chat_sessions` and `active_agent_chat_session_id`. Migrate legacy single-session fields into one default session during hydration/backfill.
- Add an `AgentChatSession` model with stable string id, title, timeline, draft, modes, usage, status, timestamps, unread/background counters, and session-scoped pending attachments.
- Replace the single backend `Arc<AgentEngineState>` with an `AgentEngineRegistry` keyed by `session_id`. Each engine owns its own conversation, event queue, pending client-tool map, cancel flag, turn generation, and chat-mode override.
- Thread `sessionId` through Tauri commands and frontend bridge calls for `agent_submit_turn`, `agent_poll_events`, `agent_abort`, `agent_clear_conversation`, `agent_submit_tool_result`, and `agent_compact_conversation`. Keep a short compatibility path for omitted `sessionId` during migration.
- In `AgentPanelDock`, add a `LuPlus` icon button to the Chatlog actions. Render `.agent-chat-session-tabs` between `.agent-chat-head` and `.workbench-agent-chat-log`.
- Each tab shows localized title, status icon, optional unread dot/count, and a close button for non-last idle sessions. Switching tabs swaps timeline/draft/modes without interrupting inactive running sessions.
- Route event batches by `session_id`. Inactive sessions update their persisted timeline and status in the background.
- Integrate inactive lifecycle events with the existing notification system:
  - `done` or successful turn completion creates an appropriate success/response notification.
  - `error` creates an error notification.
  - `harness.ask_user` creates a question notification before the ask card waits for input.
  - Notification targets include `{ "view": "agent", "workspaceId": <id>, "sessionId": "<id>" }` so clicking a notification opens the Agent panel and selects the right workspace/session.
- Update BLXCode Agent system prompt/tool docs so notification tools, `harness.ask_user`, and client-tool results are explicitly multi-session aware.
- Add i18n keys and locale entries for: new session, close session, session tab list aria, running, idle, thinking, needs input, errored, restored, unread background updates, and cannot close running session.
- Replace hardcoded Chatlog tooltip strings touched by this feature with i18n-backed strings.
- Style the tabs with existing tokens only, such as `--bg-app`, `--overlay-*`, `--border`, `--accent`, `--success`, `--warning`, `--danger`, text tokens, and radius tokens. Do not introduce a new palette, gradient treatment, or card layout.

## Tests

- Rust unit tests for snapshot migration from legacy single chat fields to one default session.
- Rust unit tests for backend registry isolation: two sessions keep separate conversations, events, tool results, cancellation, and chat-mode overrides.
- Rust unit tests that abort, clear, and compact affect only the targeted session.
- Hydration test that sessions persisted as running become idle/restored and are not replayed after restart.
- Frontend/state tests or focused component tests for creating a session, switching tabs, preserving per-session drafts/modes/timelines, and close-button rules.
- Notification routing test or manual verification that clicking an inactive-session notification selects the correct workspace and session.
- Manual verification with two concurrent sessions, background ask/done/error notifications, restart persistence, dark/light themes, and at least English/German locale checks.

## Tasks

- [ ] `branch-from-stage` - Create a feature branch from `stage` before implementation.
- [ ] `read-required-skills` - Read and apply the Rust, Tauri, and frontend design skills before coding.
- [ ] `model-session-state` - Add persisted `AgentChatSession` state and migrate legacy single-session workspace fields.
- [ ] `backend-session-registry` - Introduce a session-keyed backend agent engine registry with isolated engine state.
- [ ] `thread-session-id` - Thread `sessionId` through Tauri commands, frontend bridge calls, polling, abort, clear, compaction, and tool-result delivery.
- [ ] `build-session-tabs-ui` - Add the Chatlog `+` action and token-styled session tab bar under the Chatlog header.
- [ ] `route-background-events` - Route background session events into inactive timelines, status icons, and unread counters.
- [ ] `integrate-notifications` - Connect background done/error/ask events to the existing Agent notification system with session targets.
- [ ] `update-agent-docs` - Update system prompt/tool docs so BLXCode Agent fully supports multi-session toolcalls, asks, and notifications.
- [ ] `add-i18n-and-token-css` - Add all required i18n strings and ensure new CSS uses only theme tokens.
- [ ] `verify-multi-session` - Run automated and manual checks for concurrency, restart persistence, notifications, themes, and locales.
