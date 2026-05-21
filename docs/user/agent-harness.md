# Agent Harness

The **Agent Harness** is BLXCode’s built-in agent runtime: a slim system prompt, eleven **core skills** with full tool documentation, and server tools for environment detection, shell, Git, workspace search, and web research.

For **subagents** (parallel scout/review/security runs), see the dedicated guide [Subagents](subagents.md).

This guide covers what you see in the UI and how the agent is expected to behave. For implementation detail, see [Agent Harness (developer)](../developer/agent-harness.md).

## Settings → Agent

Open **Harness settings** (command palette or sidebar) and choose the **Agent** pane.

| Section | What it configures |
|---------|-------------------|
| **Provider** | OpenRouter, Anthropic, or OpenAI-compatible API |
| **Model** | Model ID (datalist from live/cached catalog) |
| **Thinking** | Off / Low / Medium / High / Max |
| **API key** | Per-provider key in the OS keyring (`BLXCode` service) |
| **Web Tools** | Search/fetch backend (Tavily or Brave) and API keys |

Provider and web keys are stored separately. Masked values appear in the UI after save; secrets are never shown in full.

### Web Tools

Web search and fetch require a configured backend:

| Backend | Keyring account | Env fallback |
|---------|-----------------|--------------|
| **Tavily** | `agent:web:tavily` | `BLX_TAVILY_API_KEY` |
| **Brave** | `agent:web:brave` | `BLX_BRAVE_API_KEY` |

1. Pick **Tavily** or **Brave** as the backend (or **Disabled**).
2. Paste the API key for the provider you use and click **Save key**.
3. Click **Save** on the web section to persist the provider choice.

Until a key is available, `web_search` and `web_fetch` are omitted from the agent tool catalog. The **web** core skill still appears in the Skills panel with availability **disabled_no_key** so you know what to configure.

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

### Git and diff

Read-only Git inspection is available via dedicated tools (`git_status`, `git_diff`, `git_log`, …) and workspace helpers (`workspace_git_status`, `workspace_diff`, `workspace_search`). Mutating Git (`git_add`, `git_commit`, `git_apply_patch`) is coordinator-only and requires explicit `git_write` permission in subagent runs (subagents do not get write by default).

Core skill **git** has full parameter notes.

## Conversation flow

Poll-based: `agent_submit_turn` → `agent_poll_events` until `Done`. Client tools (harness terminals, context attach) round-trip via `agent_submit_tool_result`.

## See also

- [Subagents](subagents.md) — roles, timeline, tool groups, limits
- [Agent Providers](agent-providers.md) — providers, context, hooks, image mode
- [Rules And Skills](rules-and-skills.md) — rules, user skills, bootstrap
- [Workspaces](workspaces.md) — terminals and handoff
- [Troubleshooting](troubleshooting.md) — keys, web tools, environment errors
