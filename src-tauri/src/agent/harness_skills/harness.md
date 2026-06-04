---
name: harness
description: Control BLXCode workbench views, workspaces, terminals, settings, and terminal CLI agent interactions through harness tools.
categorie: workspace
---

# Harness Actions

Client-side tools that mutate the BLXCode workbench window. After each call you receive a `role:"tool"` reply describing the result.

## Workspace & terminal management

### `harness.create_workspace { title?, cwd?, terminalCount?, agentSlugs? }`
Creates and selects a new workspace in the UI.
- `terminalCount` — 1–16 terminal slots
- `agentSlugs` — optional per-slot array like `["claude", "claude"]`
- `cwd` — omit to use the active workspace cwd or the configured harness root

### `harness.worktree_list { baseCwd? }`
Lists Git worktrees for the active workspace or `baseCwd`. Use before creating a worktree.

### `harness.create_worktree_workspace { baseCwd?, branch, startPoint?, path?, confirmed }`
Previews or creates/opens a Git worktree as a BLXCode workspace.
- First call with `confirmed:false`; report the exact base, branch, start point, path, and local/remote target to the user.
- Only call with `confirmed:true` after the user explicitly confirms.
- If the preview reports an existing matching branch/path, open/switch that worktree instead of requesting a duplicate.

### `harness.open_terminal { count?, agentSlug?, agentSlugs? }`
Opens one or more terminal slots in the **active** workspace.
- **Default:** call with no arguments `{}` for a single plain shell.
- `count` — open multiple at once (max 16). Do NOT call in a loop.
- `agentSlug` — apply the same CLI agent to every new slot
- `agentSlugs` — per-slot array (length must equal `count`)
- Only pass agent slugs when the user explicitly names one of: `claude`, `codex`, `gemini`, `opencode`, `cursor`

Example — open 3 Codex terminals: `{ "count": 3, "agentSlug": "codex" }`

### `harness.workspace_list`
Lists open workspaces and marks the active one.

### `harness.workspace_switch { id? | title? | cwd? }`
Switches to an already-open workspace. Use `harness.workspace_list` first when ambiguous.

### `harness.workspace_prev` / `harness.workspace_next`
Switch to the previous or next open workspace in sidebar order; wraps around.

## Views, tabs, and window controls

### `harness.view_show { target }`
Shows a workbench area: `agent`, `browser`, `plans`, `memory`, `rules`, `skills`, `settings`, `terminals`, `project_files`, `git_diff`, or `git_graph`.

### `harness.open_settings { category }`
Opens Settings to one category: `app`, `appearance`, `shortcuts`, `api_keys`, `workspace`, `agent_provider`, `remote`, `memory`, `voice`, `image`.

### `harness.open_memory { path? }`
Opens Memory, optionally focusing one memory API path.

### `harness.open_plan { path? }`
Opens Plans. Use `plan_read` / `plan_load` for plan content and tasks.

### `harness.open_file { path }` / `harness.open_diff { path, staged? }`
Open a workspace-relative file preview or diff center tab.

### `harness.window_get_state`, `harness.window_set_size`, `harness.window_set_fullscreen`
Read or change the BLXCode main window.

Agent Chat modes apply: `Ask Edits` asks before state-changing harness actions and submitted terminal commands; `Allow all` runs directly; `Plan` blocks workspace switches, window changes, and submitted terminal commands.

## Inspecting & driving other CLI agents

Supported terminal-agent slugs are `claude`, `codex`, `gemini`, `opencode`, and `cursor`.
The harness uses interactive PTY sessions by default:

- `claude` launches `claude`; resume uses `claude --resume <id>`.
- `codex` launches `codex`; resume uses `codex resume <id>`.
- `gemini` launches `gemini`; resume uses `gemini --resume <id>`.
- `opencode` launches `opencode`; resume uses `opencode --session <id>`.
- `cursor` launches `cursor-agent`; resume uses `cursor-agent --resume <id>`.

Headless/non-interactive flags from those CLIs can be useful for user-run scripts, but when BLXCode Agent needs to converse with another agent, keep the PTY open and use the harness tools below.

### `harness.list_terminals`
Returns `[{ slotId, name, namingMode, agentSlug, running }]` for the active workspace. **Always call this first** when you intend to interact with another agent.
- `slotId` is the stable unique identifier — always prefer it for addressing.
- `name` is the user-facing terminal name (e.g. `Devon`). When the user refers to a terminal by name, map that name to its `slotId` from this list.
- `namingMode` is `slots` (titles show `#slotId`) or `names` (titles show `name`).

### `harness.send_terminal_keys { slotId? | name? | agentSlug?, text, submit? }`
Types `text` into a slot's PTY.
- `submit: true` — appends a newline so the command/prompt is executed
- Address by `slotId` when possible (unique). `name` matches the terminal's friendly name (case-insensitive); `agentSlug` picks the first matching slot
- Use to ask a running CLI agent for status, delegate work, or drive plain shells

### `harness.send_agent_context { slotId? | name? | agentSlug?, instruction?, includeKinds?, submit? }`
Hands off BLXCode-attached context to a terminal CLI agent. Prefer this over raw `send_terminal_keys` when the other agent needs workspace context (memory/learnings, plans, tasks, images).
- Image bytes are exported to `<workspace>/.blxcode/agent-context/images/`; base64 is never written into the prompt
- `includeKinds` defaults to all four: `["memory", "plans", "tasks", "images"]`
- Call `harness.list_terminals` first when multiple slots could match

### `harness.read_terminal_output { slotId? | name? | agentSlug?, maxBytes? }`
Non-destructively reads the last bytes from a slot's rolling tail buffer (cap 64 KiB). Use after `send_terminal_keys` to observe the response. Output contains ANSI escapes — focus on the readable text.

### `harness.wait_terminal_output { slotId? | name? | agentSlug?, afterSeq?, timeoutMs?, idleMs?, maxBytes?, contains? }`
Waits for new terminal output without consuming the user's view. Returns `{ sessionId, seq, bytes, text, timedOut }`.
- Pass `afterSeq` from the previous wait/read result to observe only newer output.
- Use `contains` when waiting for a known marker or phrase.
- Use `idleMs` (default 250 ms) to wait until output settles before reading the tail.
- Use `timeoutMs` to bound the wait; if it expires, inspect `timedOut` and the returned tail.

### `harness.terminal_interrupt { slotId? | name? | agentSlug? }`
Sends Ctrl+C to the targeted PTY session. Use when a shell command or CLI agent is stuck or the user asks to interrupt it. `Ask Edits` asks before interrupting; `Plan` blocks it.

## Delegation pattern
1. `harness.list_terminals` — find the target slot
2. For substantive work delegated to a CLI agent, apply the `prompt-generating` skill first so the prompt is clear, scoped, and safe.
3. `harness.send_agent_context` or `harness.send_terminal_keys` — send the prompt
4. `harness.wait_terminal_output` — wait for response text or idle output
5. `harness.read_terminal_output` — quick tail peek when you do not need to wait
6. `harness.terminal_interrupt` — stop long-running or stuck sessions when appropriate

When the composer toggle "Enhance prompt before send" is enabled, BLXCode rewrites the user's draft through an isolated one-shot provider call before sending it as the actual Agent Chat turn. That enhancement does not mutate chat history, tools, tasks, memory, or timelines by itself.
