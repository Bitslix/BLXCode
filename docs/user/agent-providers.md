# Agent Providers

BLXCode includes an agent panel that can stream turns from remote model providers and execute registered tools. Provider settings are stored locally and can be changed from the app UI.

## Right panel tabs

| Tab | Purpose |
|-----|---------|
| **Agent** | Chat, context, tasks, voice orb |
| **Browser** | Embedded webview / iframe |
| **Plans** | Markdown plans — [Plans guide](plans.md) |
| **Memory** | Notes and graph — [Memory And Tasks](memory-and-tasks.md) |
| **Rules** | Workspace rules — [Rules And Skills](rules-and-skills.md) |
| **Skills** | Installable skills — [Rules And Skills](rules-and-skills.md) |

<p align="center">
  <img src="../images/agent-panel.png" alt="BLXCode Agent panel with provider chat, context, and tasks" />
</p>

## Supported Provider Types

- **OpenRouter**: default provider kind. The default model ID is `openai/gpt-5`.
- **Anthropic**: native Anthropic Messages API path.
- **OpenAI-compatible**: OpenAI API-compatible chat/model path.

Model lists are fetched live when possible. If a provider request fails or returns no models, BLXCode falls back to cached or curated model entries.

## API Keys

API keys are saved per provider. BLXCode tries to store them in the OS keyring using service name `BLXCode`.

If keyring access fails on Linux or another platform, BLXCode writes a fallback secret file under the Tauri app config directory. On Unix, the fallback secrets directory is created with private permissions.

The UI only displays masked API key values after save.

<p align="center">
  <img src="../images/screenshot-2026-05-18_17-58-05.png" alt="Agent provider settings showing provider, model, thinking level, and masked API key" />
</p>

## Voice And Image Keys

- **Voice** reuses provider keys — see [Voice](voice.md).
- **Image mode** (generate images in chat) reuses OpenAI/OpenRouter keys — see [Image Mode](image.md). This is separate from **context images** below.

## Thinking Levels

Off, Low, Medium, High, Max — mapped per provider where supported.

## Agent context

The **Context** section lists attached items (memory categories, notes, plans, images). Each row shows status, remove, and re-attach controls.

**Context images** (vision / handoff, not image generation):

- Attach via drag-and-drop or paste (PNG, JPEG, WebP, GIF).
- Pending images are sent once on the next turn through vision payloads, then marked read.
- Handoff exports copies to `<workspace>/.blxcode/agent-context/images/` — see [Workspaces — Handoff](workspaces.md#terminal-agent-context-handoff).
- Client tools: `image_context_list`, `image_context_detach`.

Conversation history strips image bytes after a turn so large payloads are not persisted.

## Mandatory turn checklist

For non-trivial work, the system prompt requires this order:

1. `rules_list` + `rules_read` on active rules
2. `skills_list` + `skills_read` when relevant (including **core** harness skills — see [Agent Harness](agent-harness.md))
3. Resume from `task_list` / `activePlanPath` on continuation phrases (*continue*, *resume*, *weiter*, *fortsetzen*, …)
4. Memory, plans, and project context
5. Execute

See [Rules And Skills](rules-and-skills.md) for rule/skill behavior.

## Agent tools (overview)

The system prompt sends a **compact tool name index** only. Full parameter docs live in core skills (`skills_read file-access`, `skills_read git`, etc.).

Call `list_tools` for the full JSON catalog (name, server/client site, schema).

| Group | Examples |
|-------|-----------|
| Workspace files | `list_workspace_files`, `read_workspace_file`, `workspace_search` |
| Memory | `memory_list`, `memory_read`, `memory_create`, `memory_graph`, `memory_context_*`, … |
| Tasks | `task_list`, `task_create`, `task_update`, … |
| Plans | `plan_list`, `plan_read`, `plan_load`, `plan_context_*`, … |
| Rules / skills | `rules_*`, `skills_*` |
| Harness (client) | `harness.send_terminal_keys`, `harness.send_agent_context`, … |
| Environment / shell / git (server) | `environment_detect`, `shell_exec`, `git_*`, `workspace_diff`, … |
| Web (server, if configured) | `web_search`, `web_fetch` |
| Subagents (server) | `subagents.run` — only on explicit user request — [Subagents guide](subagents.md) |

`harness.send_agent_context` prefers explicit single-terminal targets; default `includeKinds` is `["memory","plans","tasks","images"]`.

**Web tools** need Tavily or Brave keys in Agent settings → Web Tools. **Shell/Git** need `environment_detect` once per workspace session.

See [Agent Harness](agent-harness.md) for core skills and web keys; [Subagents](subagents.md) for roles, timeline, and tool groups.

## Conversation flow

```mermaid
sequenceDiagram
  participant UI as AgentPanel
  participant IPC as Tauri
  participant Orch as session_orchestrator
  participant API as Provider

  UI->>IPC: agent_submit_turn
  IPC->>Orch: dispatch_user_turn
  alt image_generate
    Orch->>API: images API
    Orch-->>UI: ImageGenerated + Done
  else text turn
    Orch->>API: chat + tools
    loop poll
      UI->>IPC: agent_poll_events
      IPC-->>UI: deltas / tool calls / Done
    end
  end
```

The frontend polls `agent_poll_events` (not SSE). Voice turns set `voice_input`; chat turns may emit `voice_ready` for TTS. Image turns may play a short confirmation phrase when voice + TTS are enabled — [Image Mode](image.md).

## Hooks For External Agents

BLXCode bundles helper scripts under `content/hooks/` for session and title capture: Claude, Codex, Gemini, OpenCode, Cursor.

<p align="center">
  <img src="../images/screenshot-2026-05-18_17-57-52.png" alt="BLXCode settings showing installed terminal hooks" />
</p>

## Missing Key Behavior

If the selected provider has no configured API key, the agent panel reports the missing key instead of attempting a network request.

## See also

- [Agent Harness](agent-harness.md) — core skills, web/shell/git tools
- [Subagents](subagents.md) — parallel subagent runs
- [Image Mode](image.md) — chat image generation toggle
- [Plans](plans.md) — plan tools and context
- [Workspaces](workspaces.md) — handoff and terminals
- [Memory And Tasks](memory-and-tasks.md) — memory tools and graph
