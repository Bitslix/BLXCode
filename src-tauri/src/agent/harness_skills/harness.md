# Harness Actions

Client-side tools that mutate the BLXCode workbench window. After each call you receive a `role:"tool"` reply describing the result.

## Workspace & terminal management

### `harness.create_workspace { title?, cwd?, terminalCount?, agentSlugs? }`
Creates and selects a new workspace in the UI.
- `terminalCount` — 1–16 terminal slots
- `agentSlugs` — optional per-slot array like `["claude", "claude"]`
- `cwd` — omit to use the active workspace cwd or the configured harness root

### `harness.open_terminal { count?, agentSlug?, agentSlugs? }`
Opens one or more terminal slots in the **active** workspace.
- **Default:** call with no arguments `{}` for a single plain shell.
- `count` — open multiple at once (max 16). Do NOT call in a loop.
- `agentSlug` — apply the same CLI agent to every new slot
- `agentSlugs` — per-slot array (length must equal `count`)
- Only pass agent slugs when the user explicitly names one of: `claude`, `codex`, `gemini`, `opencode`, `cursor`

Example — open 3 Codex terminals: `{ "count": 3, "agentSlug": "codex" }`

## Inspecting & driving other CLI agents

### `harness.list_terminals`
Returns `[{ slotId, agentSlug, running }]` for the active workspace. **Always call this first** when you intend to interact with another agent.

### `harness.send_terminal_keys { slotId? | agentSlug?, text, submit? }`
Types `text` into a slot's PTY.
- `submit: true` — appends a newline so the command/prompt is executed
- Address by `slotId` when possible (unique); `agentSlug` picks the first matching slot
- Use to ask a running CLI agent for status, delegate work, or drive plain shells

### `harness.send_agent_context { slotId? | agentSlug?, instruction?, includeKinds?, submit? }`
Hands off BLXCode-attached context to a terminal CLI agent. Prefer this over raw `send_terminal_keys` when the other agent needs workspace context (memory/learnings, plans, tasks, images).
- Image bytes are exported to `<workspace>/.blxcode/agent-context/images/`; base64 is never written into the prompt
- `includeKinds` defaults to all four: `["memory", "plans", "tasks", "images"]`
- Call `harness.list_terminals` first when multiple slots could match

### `harness.read_terminal_output { slotId? | agentSlug?, maxBytes? }`
Non-destructively reads the last bytes from a slot's rolling tail buffer (cap 64 KiB). Use after `send_terminal_keys` to observe the response. Output contains ANSI escapes — focus on the readable text.

## Delegation pattern
1. `harness.list_terminals` — find the target slot
2. `harness.send_agent_context` or `harness.send_terminal_keys` — send the prompt
3. Wait briefly, then `harness.read_terminal_output` — capture the reply (repeat for long-running tasks)
