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
- **Local OpenAI-compatible**: Ollama and LM Studio. No API key is required; configure the base URL if your local server is not on the default port.
- **Cloud OpenAI-compatible**: OpenAI, Hugging Face router, Cloudflare Workers AI, Together AI, and Portkey.

Model lists are fetched live when possible. If a provider request fails or returns no models, BLXCode falls back to cached or curated model entries.

| Provider | Default endpoint | Required setup |
|----------|------------------|----------------|
| OpenRouter | `https://openrouter.ai/api/v1` | `BLX_OPENROUTER_API_KEY` or API Keys row |
| Anthropic | `https://api.anthropic.com/v1` | `BLX_ANTHROPIC_API_KEY` or API Keys row |
| OpenAI | `https://api.openai.com/v1` | `BLX_OPENAI_API_KEY` or API Keys row |
| Ollama | `http://localhost:11434/v1` | Running Ollama server |
| LM Studio | `http://localhost:1234/v1` | Running LM Studio OpenAI-compatible server |
| Hugging Face | `https://router.huggingface.co/v1` | `BLX_HUGGINGFACE_API_KEY` or API Keys row |
| Cloudflare | account-scoped Workers AI endpoint | `BLX_CLOUDFLARE_API_TOKEN` or API Keys row, plus Cloudflare Account ID in Agent settings |
| Together AI | `https://api.together.ai/v1` | `BLX_TOGETHER_API_KEY` or API Keys row |
| Portkey | `https://api.portkey.ai/v1` | `BLX_PORTKEY_API_KEY` or API Keys row; optional base URL override |

These providers are text-agent providers in this release. Image mode and Voice settings keep their existing provider lists; adding an LLM key row does not enable image generation, STT, or TTS for that provider.

## API Keys

All secrets are managed under **Settings → API Keys** (single Save/Discard pane). BLXCode stores them in the OS keyring (`BLXCode` service) with `BLX_*` env fallback when a slot is empty.

**Settings → BLXCode Agent** shows provider/model/thinking for text, image, and voice plus web-tool backend choice. Each column displays a short **configured / missing** hint — no password fields.

| Use | Keys in API Keys pane |
|-----|------------------------|
| Text agent | OpenRouter, Anthropic, OpenAI, Hugging Face, Cloudflare, Together AI, Portkey |
| Image mode | OpenAI, OpenRouter, fal.ai |
| Voice STT/TTS | OpenAI, OpenRouter, AWS (Polly) |
| Web search/fetch | Tavily, Brave |

See [Settings](settings.md) and [Voice](voice.md) / [Image Mode](image.md).

## Thinking Levels

Off, Low, Medium, High, Max — mapped per provider where supported.

## Chat header (context, compact, send/stop)

The chat above the compose box shows live conversation state in the header:

- **Context-window meter** — `used / max · NN%` with a thin progress bar that turns **warning** past 70% occupancy and **danger** past 85%. Occupancy tracks the **newest main-agent round's prompt size** (true window occupancy), not the cumulative token count used for cost; subagent rounds are excluded. The maximum is resolved from the provider's own model metadata where possible — for example OpenRouter's `context_length` is cached per model — and falls back to a static table (`agent/context_window.rs`) that covers the direct providers (Claude 200K, GPT-5 400K, Gemini 1M, GPT-4.1 1M, …). For unknown models, the header shows a plain token count without a percentage instead of inventing a denominator.
- **Compact** — a button next to the meter that summarizes the running conversation into a dense briefing and starts the session fresh from it, freeing context-window budget while preserving goals, decisions, file paths, task state, and open questions. A single non-tool provider call does the summarization (so it cannot enter a tool loop); the backend replaces history with a compact `user`→`assistant` pair, the visible timeline resets to a fresh chat, and the meter drops to the post-compaction estimate.
- **Auto-compact** — same path runs automatically once occupancy crosses a threshold (default **85%**, configurable 50–95% under **Settings → BLXCode Agent**). It fires only between turns and **at most once per crossing** — it re-arms only after occupancy falls back below the threshold — so it never interrupts a running turn or storms.

The compose bar's submit button is a **single Send / Stop toggle**: it shows **Send** (sparkles icon) while idle and flips to **Stop** (square icon, abort styling) while a turn is running. Clicking submits or aborts depending on state; pressing **Enter** in the input still submits.

## Agent session stats

The Agent hero (the right column of the chat header) is a compact, **unframed** live stats panel. It derives all values client-side from the existing timeline, usage aggregate, context-window signal, model label, and busy state — no card chrome, no stats tooltips:

- **Provider / model** label with a single state chip that follows open *Thinking* blocks before falling back to **Running** / **Standby**. The chip is a real button that opens **Settings → Agent Provider** directly.
- **Local session start time** (the first user turn's `createdAt` after workspace reload, not the load time). Chat clear resets the session.
- **Context-window occupancy** with a mini progress meter.
- **Turn counts** (combined `User: x / Model: y`).
- **Total tool calls** with merged open / read / edit / rm buckets.
- **Active subagents** — count and names.
- **Accumulated session cost.**

The Chat log titlebar no longer repeats cost / turn / context stats. The numbers come from a dedicated `session_stats` aggregator that walks nested tool and subagent timeline parts (with unit tests for model-round exclusion, merged tool counts, bucket classification, active-subagent detection, first-user-turn start recovery, and active-thinking detection). `ChatUsageStats` now persists a backwards-compatible `session_started_at` timestamp, and `UserPart` stores an optional `createdAt`.

## Per-message text-to-speech (Play button)

The chat timeline may show a small **Play** button on assistant messages to read the reply out loud. It only appears when **TTS is enabled** *and* the selected TTS provider actually has an API key set in **Settings → Voice** (OpenAI / OpenRouter via the agent key status, AWS via the `aws_polly` key). When TTS is disabled or the key is missing, the button is hidden — see [Voice](voice.md).

## Tool-loop limit

The hard ceiling on tool-call rounds inside a single turn — historically a fixed **36** that produced *"Tool-Loop-Limit erreicht (36 Runden)"* when a long investigation ran out of rounds — is now a user setting.

- **Range**: 1–500 (clamped on save and again at the call site).
- **Default**: 36.
- **Where**: **Settings → BLXCode Agent → Tool-loop limit** (next to *Thinking level*).
- Applies to both the OpenAI-compatible (OpenRouter / OpenAI) and Anthropic coordinator loops.

A higher value lets the agent run deeper investigations; a lower value limits the blast radius of a runaway plan.

## Timeline grouping

When a finished **Thinking** block is immediately followed by a tool-bearing **MODEL ROUND**, the two collapse into one row in the timeline: the round label sits on the left and the *Thinking ▾* toggle floats to the right edge of the same line, with the reasoning text dropping below when expanded. The pair occupies a single sequential line number, so a model round always sorts correctly into the rest of the timeline instead of standing on its own row. Rounds without groupable tools, and still-streaming thinking, keep their standalone rows.

Consecutive tool activity in a single round now renders as slim **grouped status rows** for the main agent and subagent cards — per-tool icons, argument summaries, status indicators, expandable details, metrics, and path aggregation all collapse into a single line with an `×N` count for repeats.

## Thinking stream preview

While the current turn is actively thinking, a compact inline preview appears under the Drobo orb and follows the newest open *Thinking* block from the timeline. It autoscrolls as reasoning text streams in, uses the active theme radius/color tokens, and is automatically hidden in compact chat mode so the maximized chat header stays clean.

## Changed files card

When a model turn mutates workspace files, the turn ends with a **Changed files** summary card built from the existing `git_status_changes` command. It shows totals for additions / deletions, a collapsible directory tree with per-file stats, and clicking a row opens the file's diff in the existing center-tab diff view. The card is rendered from the same git data the sidebar already uses — no new backend protocol fields are involved.

## Agent tool list output

JSON-array tool results such as `rules_list` and `skills_list` render as readable compact lists in the chat timeline instead of raw one-line JSON blobs. The agent itself still receives the original JSON; the renderer is a UI-only presentation layer that extracts `title` / `name`, `summary`, category/kind, and small metadata chips. A tolerant fallback still shows complete list items from truncated array prefixes so the same tool call stays readable when its payload is large.

## Modern composer

The compose area below the chat is a modern auto-growing textarea with a footer that holds:

- a **Model picker** (the same picker the Stats panel chip links to),
- a **Mode popover** (Plan / Build / Full access — mapped onto the existing `AgentChatMode` values),
- a **Thinking-level selector** (Off / Low / Medium / High / Max — provider-dependent),
- busy-safe controls, and
- a single **Send / Stop toggle** orb (the same one as in the chat header).

## Enhance prompt before send

A per-workspace **Enhance prompt before send** toggle in the composer rewrites the draft through an isolated one-shot provider call before submitting it as the actual user turn. The rewrite uses a small dedicated model call (the same path that backs AI commit messages) and never mutates chat history, tools, memory, plans, or timeline state — the enhanced text is what the model actually sees, but everything else is preserved verbatim.

## Terminal CLI-agent control

The BLXCode Agent can drive interactive terminal agents (Claude Code, Codex, Gemini, OpenCode, Cursor) end to end through the harness. The launch/resume profiles for each supported agent are centralized so the UI launch commands, docs, and agent guidance stay in sync. The agent has a new family of terminal-control tools that let it:

- list / target a specific terminal slot by `slotId`, terminal `name`, or `agentSlug`,
- send raw keys or an attached BLXCode context block,
- read recent terminal output,
- wait for new or settled output with a sequence id (so it can correlate partial reads),
- interrupt a stuck session with Ctrl+C.

A new embedded core skill, **`prompt-generating`**, teaches the model how to scope a prompt for BLXCode chat, terminal CLI agents, subagents, and user-facing replies, and the system prompt requires the model to consult that skill before any substantive CLI-agent handoff.

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

**Web tools** need Tavily or Brave keys in **Settings → API Keys**, then a backend choice under **BLXCode Agent → Web Tools**. **Shell/Git** need `environment_detect` once per workspace session.

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
  <img src="../images/settings-terminal-hooks.png" alt="BLXCode settings showing installed terminal hooks" />
</p>

## Missing Key Behavior

If the selected cloud provider has no configured API key, the agent panel reports the missing key instead of attempting a network request. Ollama and LM Studio skip this check and fail with the provider's connection error if the local server is not running.

## See also

- [Agent Harness](agent-harness.md) — core skills, web/shell/git tools
- [Subagents](subagents.md) — parallel subagent runs
- [Image Mode](image.md) — chat image generation toggle
- [Plans](plans.md) — plan tools and context
- [Workspaces](workspaces.md) — handoff and terminals
- [Memory And Tasks](memory-and-tasks.md) — memory tools and graph
