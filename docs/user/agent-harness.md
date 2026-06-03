# Agent Harness

The **Agent Harness** is BLXCode’s built-in agent runtime: a slim system prompt, eleven **core skills** with full tool documentation, and server tools for environment detection, shell, Git, workspace search, and web research.

For **subagents** (parallel scout/review/security runs), see the dedicated guide [Subagents](subagents.md).

This guide covers what you see in the UI and how the agent is expected to behave. For implementation detail, see [Agent Harness (developer)](../developer/agent-harness.md).

## Settings

Open **Settings** from the command palette (center tab). Overview: [Settings](settings.md).

| Category | Harness-related content |
|----------|-------------------------|
| **API Keys** | All LLM, search, fal.ai, and AWS Polly secrets |
| **BLXCode Agent** | Text provider/model/thinking, image provider, voice STT/TTS, web tools |
| **App** | STT language + push-to-talk (not full voice models) |

### BLXCode Agent → Text

| Field | What it configures |
|-------|-------------------|
| **Provider** | OpenRouter, Anthropic, or OpenAI-compatible API |
| **Model** | `AgentModelPicker` — catalog, pricing line, custom id |
| **Thinking** | Off / Low / Medium / High / Max |

API keys are **not** entered here. A muted hint links to **Settings → API Keys**; masked status shows configured / missing.

### Web Tools (bottom of BLXCode Agent)

| Backend | Keyring account | Env fallback |
|---------|-----------------|--------------|
| **Tavily** | `agent:web:tavily` | `BLX_TAVILY_API_KEY` |
| **Brave** | `agent:web:brave` | `BLX_BRAVE_API_KEY` |

1. Set Tavily and/or Brave keys under **Settings → API Keys**.
2. Pick **Tavily**, **Brave**, or **Disabled** in BLXCode Agent.
3. Click **Save** on the BLXCode Agent footer (with text provider settings).

Until a key is available, `web_search` and `web_fetch` are omitted from the agent tool catalog. The **web** core skill still appears in the Skills panel with availability **disabled_no_key**.

## Core skills vs user skills

Previously, long tool documentation lived inside every API request. **Better Harness** moves that into **core skills** — built-in Markdown guides shipped inside the app.

| Kind | Where it lives | Editable? | Remove? |
|------|----------------|-----------|---------|
| **Core** | Embedded in the binary (`harness_skills/*.md`) | No (toggle enable only) | No |
| **User** | `<workspace>/.agents/skills/<name>/SKILL.md` | Yes | Yes |

In the **Skills** panel, use the **Core** / **User** tabs:

- **Core** — `file-access`, `memory`, `plans`, `tasks`, `rules-skills`, `harness`, `environment`, `shell`, `git`, `web`, `subagents`
- **User** — skills you install (git, npm, or local path)

The agent system prompt lists tool **names** only. For parameters and patterns, the agent calls `skills_read` on the relevant core skill (same as reading a user skill).

See [Rules And Skills](rules-and-skills.md) for install flow and the turn checklist.

## Turn checklist (every non-trivial turn)

The harness enforces this order in the system prompt:

1. **Rules** — `rules_list`, then `rules_read` on active rules that apply
2. **Skills** — `skills_list`, then `skills_read` when a skill matches the task (including core skills above)
3. **Resume** — on continuation phrases (*continue*, *resume*, *weiter*, *fortsetzen*, …): `task_list`, `activePlanPath`, `plan_read` if needed
4. **Memory / plans** — as required by the task
5. **Execute** — tools and a visible final reply

Trivial one-line chat may skip steps 1–2. Any file change or tool use should run the full checklist.

## Environment, shell, and Git

### Environment detect (required first)

Before **shell** or **Git** tools run in a workspace session, the agent must call **`environment_detect`** once. That caches OS, default shell, path separator, and whether Git is available.

- Switching workspaces clears the cache; the next turn should detect again.
- If shell/Git is called too early, the tool returns an error asking for `environment_detect`.

### Shell vs harness terminals

| Mechanism | Use for |
|-----------|---------|
| **`harness.*` (client)** | Interactive CLIs in the workbench terminal grid, context handoff, reading pane output |
| **`shell_exec` (server)** | One-shot commands in the workspace directory (read-only by default; write requires coordinator `shell_write` group) |

Core skill **harness** documents terminal tools; **shell** documents `shell_exec` and allowlists.

When terminals are in **named** mode (see [Workspaces → Named terminals](workspaces.md#named-terminals)), the harness terminal tools accept a `name` argument alongside the existing `slotId` and `agentSlug` targets. Matching is case-insensitive, so the agent can resolve a request like *"ask Devon to run the tests"* to the right slot. `harness.list_terminals` also returns the resolved display `name` and the current `namingMode` for every slot.

### Git and diff

Read-only Git inspection is available via dedicated tools (`git_status`, `git_diff`, `git_log`, `git_conflicts`, …) and workspace helpers (`workspace_git_status`, `workspace_diff`, `workspace_search`). Mutating Git (`git_add`, `git_commit`, `git_apply_patch`) is coordinator-only and requires explicit `git_write` permission in subagent runs (subagents do not get write by default).

Core skill **git** has full parameter notes.

## Conversation flow

Poll-based: `agent_submit_turn` → `agent_poll_events` until `Done`. Client tools (harness terminals, context attach) round-trip via `agent_submit_tool_result`.

## Agent Chat modes

The toolbar above the Agent input selects a mode for the current Agent Chat session:

| Mode | Behavior |
|------|----------|
| **Ask Edits** | Default. BLXCode asks before file/folder writes/deletes/renames, app/window/settings changes, and shell/terminal command execution. |
| **Allow all** | Runs tool calls directly, including Bash/PowerShell/CMD commands. |
| **Plan** | Read/search/analyze only. Mutating edits, window/settings changes, workspace switches, terminal submits, and write-capable commands are blocked. |

The mode is stored per workspace chat session and resets to **Ask Edits** when the chat is cleared.

When Ask Edits prompts for file or folder writes/deletes/renames, the permission card offers **Approve once** or **Auto-accept**. Auto-accept approves the current tool call and switches that workspace to **Allow all** immediately, so later file edits in the same turn do not repeatedly prompt.

The composer also has an **Enhance prompt** toggle. When enabled, BLXCode sends the draft through a separate one-shot provider request, replaces it with the improved prompt, and then submits that improved text as the actual user turn. If enhancement fails, the original draft is restored and nothing is sent.

## Workbench control tools

The Agent can list and switch open workspaces, move to previous/next workspace, open right-panel views (Agent, Browser, Plans, Memory, Rules, Skills), open Settings categories, show sidebar sections, open file/diff tabs, and control the BLXCode main-window size/fullscreen state. These actions follow the active Agent Chat mode.

For terminal CLI agents (`claude`, `codex`, `gemini`, `opencode`, `cursor`), the Agent uses PTY tools end to end: open/list terminals, send prompts or attached context, wait for new output with `harness.wait_terminal_output`, inspect tails with `harness.read_terminal_output`, and interrupt stuck sessions with `harness.terminal_interrupt`.

## See also

- [Subagents](subagents.md) — roles, timeline, tool groups, limits
- [Agent Providers](agent-providers.md) — providers, context, hooks, image mode
- [Rules And Skills](rules-and-skills.md) — rules, user skills, bootstrap
- [Workspaces](workspaces.md) — terminals and handoff
- [Troubleshooting](troubleshooting.md) — keys, web tools, environment errors
