# Agent Harness (developer)

This document describes the **Better Harness** stack: slim system prompt, embedded core skills, unified tool dispatch, tool groups, environment cache, shell/Git/web tools, and related UI.

**Subagents** are documented separately in [Subagents (developer)](subagents.md).

## Goals

1. **Token efficiency** — Tool documentation lives in core skills (`skills_read`), not in every request’s system prompt.
2. **Maintainability** — One `system_prompt.rs`, one `tool_dispatch.rs`, one `tool_groups.rs` for OpenRouter, OpenAI-compatible, Anthropic, and subagent loops.
3. **Safety** — Workspace sandbox, environment gate for shell/Git, read-only subagent defaults, explicit `allowedToolGroups`.

## Module map

```text
src-tauri/src/agent/
  system_prompt.rs       # Shared prompt (~250 lines): checklist + tool name index
  harness_skills/*.md    # 11 core skill bodies (include_str! in store)
  tool_dispatch.rs       # handle_tool_call for coordinator + subagents
  tool_groups.rs         # ToolGroup enum, registry_filtered, coordinator_groups
  environment.rs         # environment_detect + session cache
  shell_exec.rs          # shell_exec + child registry + cancel kill
  git_agent.rs           # git_* server tools
  workspace_agent.rs     # workspace_search, workspace_git_status, workspace_diff
  web_settings.rs        # agent.web envelope + keyring + runtime cache
  web_tools.rs           # web_search (Tavily), web_fetch
  web_commands.rs        # Tauri: agent_web_* , agent_environment_invalidate
  subagents.rs           # see developer/subagents.md
  tools.rs               # Full registry; execute_server_tool
  tools_extra.rs         # submit_result and harness-only pieces
  session_orchestrator.rs
  provider.rs              # Text-provider registry: endpoint/auth/model metadata
  openrouter.rs / anthropic.rs  # Compatible + native loops, both use tool_dispatch

src/skills_rules/store.rs   # CORE_SKILLS, core SkillSourceKind, availability
src-tauri/src/api_keys.rs     # Central key catalog, resolve, api_keys_status/apply
src/workbench/
  harness_ui.rs               # SettingsDock, App pane
  appearance_settings_pane/   # Theme picker (Settings → Appearance)
  agent_provider_pane/        # BLXCode Agent grid (text, web footer)
  harness_image_pane/         # AgentImageColumn
  harness_voice_pane/         # AgentVoiceColumn
  agent_model_picker/         # Shared model dropdown + pricing detail
  api_keys_pane/              # Settings → API Keys UI
  workspace_settings_pane/    # Paths, browser, category_colors
  agent_timeline.rs           # tool_label, subagent_*_label (i18n)
  agent_panel/timeline.rs     # chat timeline (subagent UI: see subagents.md)
src/tauri_bridge.rs           # api_keys_*, agent_web_*, agent_environment_invalidate
```

## Core skills (Better Harness)

### Embedding

`src-tauri/src/skills_rules/store.rs`:

```rust
pub const CORE_SKILLS: &[(&str, &str)] = &[
    ("file-access", include_str!("../agent/harness_skills/file-access.md")),
    // … memory, plans, tasks, rules-skills, harness,
    // environment, shell, git, web, subagents
];
```

### Source kind

`SkillSourceKind::Core` in `skills_rules/types.rs` and `src/skills_rules_wire.rs`. Core entries:

- Always listed in `skills_list` (merged before user skills)
- `read_skill` serves embedded Markdown
- `remove_skill` rejects core names
- `set_skill_enabled` persists in workspace `index.json` like user skills

### Runtime availability

`core_skill_availability("web")` returns `Some("disabled_no_key")` when `web_settings::web_tools_enabled()` is false. The skills UI can surface this without removing the skill from the catalog.

### System prompt contract

`system_prompt()` in `system_prompt.rs`:

- Retains scope, security, mandatory turn checklist, behaviour rules
- Replaces per-tool prose with a **compact name index** grouped by area
- Directs the model to `skills_read` with core skill names for full guidance
- **Requires `skills_read prompt-generating` before any substantive CLI-agent handoff** — the new `prompt-generating` core skill teaches the model how to scope prompts for BLXCode chat, terminal CLI agents (Claude Code, Codex, Gemini, OpenCode, Cursor), subagents, and user-facing replies

Adding a new server tool typically requires:

1. Register in `tools.rs`
2. Document in the appropriate `harness_skills/*.md`
3. Add a line to the tool index in `system_prompt.rs`
4. Add `I18nKey::AgTool*` + all locale files if the UI shows a label

## Tool dispatch unification

`tool_dispatch.rs` exposes `DispatchContext` and `handle_tool_call` used by:

- `openrouter.rs` / OpenAI-compatible streaming loop
- `anthropic.rs` streaming loop
- `subagent_runner.rs` — see [Subagents](subagents.md)

New tools should be wired once in dispatch + `tools::execute_server_tool`, not duplicated per provider.

## Text provider registry

`agent/provider.rs` is the central registry for BLXCode Agent text providers. Add new text providers there first, then reuse the registry metadata from settings/model refresh/runtime code.

Registry metadata covers:

- provider id and label
- local/cloud/gateway class
- default OpenAI-compatible base URL
- auth mode (`none`, required bearer, optional bearer)
- model discovery strategy
- compatibility flags such as OpenRouter request extras and OpenAI `reasoning_effort`

Anthropic is the only native Messages API loop. OpenRouter, OpenAI, Ollama, LM Studio, Hugging Face, Cloudflare Workers AI, Together AI, and Portkey route through the OpenAI-compatible chat-completions loop. Local providers do not require a key. Cloudflare also requires `cloudflare_account_id` in `AgentProviderSettings`.

`AgentProviderSettings` keeps legacy `model_cache_openrouter`, `model_cache_anthropic`, and `model_cache_openai` fields for compatibility, plus the provider-keyed `model_caches` map used by new providers. Base URL overrides live in `provider_base_urls`; secrets stay in keyring/env via the API Keys catalog.

The v1 provider expansion is text-only. Image and Voice settings intentionally keep their own provider enums and HTTP clients.

### Agent Chat modes and permission gate

`UserTurn.chat_mode` carries the per-session mode selected in the Agent panel:

- `ask_edits` — mutating edit tools, command execution, and app/window/settings state changes emit `ToolPermissionRequest` and wait for `agent_submit_tool_result`.
- `allow_all` — no prompt; tool calls execute directly.
- `plan` — non-mutating mode; write tools, write-capable commands, workspace switches, window/settings changes, submitted terminal commands, context handoff, and terminal interrupts are blocked before execution.

Server tools are gated in `tool_dispatch.rs` before `execute_server_tool`; client harness tools are gated before the `ToolCall` event is emitted, so the frontend cannot execute a client tool before approval.

## Tool groups

`ToolGroup` in `tool_groups.rs` maps group IDs (e.g. `git_read`, `shell_write`) to tool name sets.

| API | Purpose |
|-----|---------|
| `coordinator_groups(web_enabled)` | Full coordinator catalog |
| `registry_filtered(groups, web_enabled)` | Subagent or filtered coordinator set |
| `render_for_openai_filtered` / `render_for_anthropic_filtered` | Provider tool JSON |

Subagent filtering rules: [Subagents](subagents.md#tool-catalog-filtering).

## Environment cache

`environment.rs`:

- `tool_environment_detect` — builds snapshot JSON, sets cache entry for workspace path
- `require_environment` — gates `shell_exec` and git tools
- `invalidate_cache` — clears session cache (Tauri command `agent_environment_invalidate`)
- `note_workspace_change` — clears cache when workspace root changes (orchestrator on turn start)

Frontend: `WorkbenchService::select_workspace` calls `agent_environment_invalidate()` when switching workspaces.

Cache stores only the workspace path string (presence = detect completed for that root).

## Shell execution

`shell_exec.rs`:

- Spawns bash/powershell in workspace CWD
- Registers children for `kill_all_children()` on cancel
- Read-only allowlist unless `ToolExecOpts.shell_writes` is true (coordinator `shell_write` only)

## Web settings

Persistence:

- Settings envelope key `web` inside `agent_provider_settings.json` (provider enum only)
- Secrets: keyring `BLXCode` / `agent:web:tavily` | `agent:web:brave`

Commands (`web_commands.rs`):

- `agent_web_settings_get` / `agent_web_settings_save`
- `agent_web_api_key_set` / `agent_web_api_key_delete`
- `agent_environment_invalidate`

Frontend wrappers in `tauri_bridge.rs`; UI in `harness_ui.rs` `AgentProviderPane`.

`web_tools.rs` implements Tavily search; Brave may be stubbed or partial — check source before documenting provider-specific behaviour in release notes.

## Terminal CLI-agent control

The coordinator and subagent loops can drive **interactive terminal CLI agents** end to end through the same harness PTY pipeline. The supported slugs are `claude`, `codex`, `gemini`, `opencode`, and `cursor` (empty string for a plain shell). The launch / resume profiles for each are centralized in `agent/terminal_agents.rs` so UI launch commands, docs, and the model prompt stay in sync.

Tools (in `agent/tools.rs`, gated by `ToolGroup::harness`):

| Tool | Purpose |
|------|---------|
| `harness.list_terminals` | Enumerate terminal slots in the active workspace. Each entry carries `slotId`, `agentSlug`, `running`, and (post v0.5.0) `name` + `namingMode` (see [Named terminals](#named-terminals) in [Workspaces](../user/workspaces.md)). |
| `harness.send_terminal_keys` | Send keystrokes to a targeted slot. Address by `slotId` (preferred) or `agentSlug`. Set `submit: true` to append a newline so the command executes. |
| `harness.send_agent_context` | Render the current BLXCode context as a Markdown block, export any selected images to `<workspace>/.blxcode/agent-context/images/`, and write the block into the terminal's PTY. `includeKinds` defaults to `["memory", "plans", "tasks", "images"]`. |
| `harness.read_terminal_output` | Non-destructive read of the slot's rolling tail buffer (capped at **64 KiB**). Use after `send_terminal_keys` to see how a CLI agent responded. |
| `harness.wait_terminal_output` | Incremental wait with `afterSeq`, optional `contains` marker, and `idleMs`; returns `{ sessionId, seq, bytes, text, timedOut }`. `wait_terminal_output` runs as an **async polling command** that takes short PTY snapshots and sleeps with Tokio, so it never blocks the Tauri command thread. |
| `harness.terminal_interrupt` | Send Ctrl+C to a targeted slot. |

The system prompt requires the model to call `skills_read prompt-generating` before any substantive CLI-agent handoff. `prompt-generating` is a new core skill that teaches the model how to scope prompts for BLXCode chat, terminal CLI agents, subagents, and user-facing replies.

`harness.ask_user` is also part of the same harness tool family and is the way the model requests a structured decision before driving a long-lived CLI agent run.

## Agent timeline refactor

The chat timeline lives in `src/workbench/agent_panel/`. The v0.5.0 refactor split it into three focused component folders, each with its own token-only CSS:

- `agent_panel/tool_group/` — consecutive tool activity in a single round now renders as slim **grouped status rows** (per-tool icons, argument summaries, status indicators, expandable details, metrics, path aggregation, `×N` counts). Same component for the main agent and for subagent cards.
- `agent_panel/changed_files_card/` — when a model round mutates workspace files, the turn ends with a **Changed files** summary card built from the existing `git_status_changes` command (totals + collapsible directory tree with per-file stats). Clicking a row opens the file's diff in the existing center-tab diff view. **No new backend protocol fields** — the card is a pure renderer over the same `git_status_changes` payload the sidebar already uses.
- `agent_panel/composer/` — the modern auto-growing composer replaces the old mode toolbar + single-line input. Footer model picker, Plan / Build / access mode popover, thinking-level selector, busy-safe controls, and a single **Send / Stop toggle** orb. The compose bar's `chat_mode` (`AgentChatMode::ask_edits | allow_all | plan`) is unchanged — the popover is a UI presentation of the existing values.

The model-round line number is **decoupled from the stable expand-state key** (`stable_index` was being passed where the display index was expected, leaking `hash + 1` into the UI). The display line number is now threaded through explicitly, and rounds sort correctly into the sequential numbering.

A finished **Thinking** block that is immediately followed by a tool-bearing **MODEL ROUND** is collapsed onto the same line: the round label on the left, the *Thinking ▾* toggle on the right of the same line, with the reasoning text dropping below when expanded. The pair occupies a single line number. Rounds without groupable tools, and still-streaming thinking, keep their standalone rows.

## Agent tool list output (UI-only)

JSON-array tool results such as `rules_list` and `skills_list` are rendered as readable compact lists in the chat timeline instead of raw one-line JSON blobs. The agent itself still receives the original JSON; the renderer lives in `agent_panel/tool_group/list_view.rs` and extracts common fields (`title` / `name`, `summary`, category / kind, small metadata chips). A tolerant fallback can still show complete list items from truncated array prefixes so a large payload stays usable.

## Enhance prompt before send

A per-workspace **Enhance prompt before send** toggle in the composer rewrites the draft through an isolated one-shot provider call (the same `oneshot::complete_text` path that backs AI commit messages and AI plans) before submitting it as the actual user turn. The enhanced text is what the model sees, but chat history, tools, memory, plans, and timeline state are never mutated.

## Tool-loop limit and auto-compact

`AgentProviderSettings` gained a configurable `tool_loop_limit` and an `auto_compact` boolean.

- `tool_loop_limit` — caps the number of consecutive tool calls within a single assistant turn (default 24). When the cap is hit, the orchestrator stops the loop, returns the partial tool result set to the model, and asks the model to summarize. The chat panel surfaces a "Tool loop cap reached" notice for the affected turn. The cap is checked in `tool_dispatch::handle_tool_call` and applied across coordinator and subagent loops.
- `auto_compact` — when true, the orchestrator transparently compacts the conversation before submitting the next turn if estimated input tokens would exceed 80% of the active provider's context window. Compaction uses a one-shot, non-streaming completion that summarizes the older turns (preserving file paths, decisions, and current plan status) and prepends the summary as a system message; the original messages are still kept on disk for audit and can be expanded from the session timeline. The 80% threshold and the compaction prompt are tunable in `src-tauri/src/agent/session_orchestrator.rs`.
- `context_window` — the active provider's configured context window, returned by the new Tauri command. The chat panel's context meter reads this value to show real-time usage; auto-compact uses the same number as its ceiling. The setting is per-provider in `agent_provider_settings.json` under `models.<model_id>.context_window` (with a sensible default per provider family).

The settings UI exposes both as Advanced controls on the *BLXCode Agent* pane (see [Settings](../user/settings.md) and [Agent Providers](../user/agent-providers.md)). The chat panel's send button toggles to a stop button while the model is streaming; abort cleanly tears down the tool loop and re-enables the send button with the original prompt restored.

## Session stats

The chat panel shows a per-session stats strip at the top of the conversation:

- **Provider/Model chip** — the active provider and model from `AgentProviderSettings`.
- **Session start** — wall-clock time of the first user turn; persisted as `ChatUsageStats.session_started_at` (back-compat with older `ChatUsageStats` envelopes that omit the field).
- **Context meter** — current estimated input tokens vs the active `context_window`; coloured by usage band.
- **Turn counts** — user turns, assistant turns, and tool turns; tool turns include subagent-spawned turns.
- **Tool calls** — total number of `ToolCall` events since session start, broken down by tool name on hover.
- **Subagents** — number of subagent runs and their statuses; clicking filters the timeline to the selected subagent's children.
- **Cost** — running cost computed from the model's per-token price (USD) and the cumulative prompt / completion tokens; opens the model-picker detail when clicked.

The stats are produced by a dedicated `session_stats` aggregator in `session_orchestrator.rs` that subscribes to `AgentEvent`s and writes into the persisted `ChatUsageStats` envelope on every event boundary. The `UserPart` envelope gained an optional `createdAt` so a session re-opened mid-conversation can reconstruct its stats from the persisted transcript even if the in-memory aggregator was dropped.

## Frontend i18n

Tool and web labels use `I18nKey` variants (`AgWeb*`, `AgTool*`) in all `src/i18n/locales/*.rs`. Subagent-specific keys (`AgSubagent*`, `AgRole*`) are documented in [Subagents](subagents.md).

Skills panel: `SrSkillsTabCore`, `SrSkillsTabUser`, `SrSourceCore` — see [Internationalization](i18n.md).

## Session roles (harness session modes)

A **session role** lets the user launch a workspace in a specialized mode
(Coordinator, Architect, Security Reviewer, …). Roles are read-only built-ins,
embedded from `src-tauri/src/agent/harness_skills/specialized/*.md`.

- `agent/session_roles.rs` — embeds each role via `include_str!`
  (`SPECIALIZED_ROLES`), parses the YAML frontmatter (`name`, `description`,
  `tools`, `color`, `provider`, `models`), and exposes `list_roles()`,
  `role_meta(slug)`, and `role_prompt_body(slug)`. `role_prompt_body` strips the
  frontmatter and the duplicated `## Prompt Defense Baseline` section (Security
  already covers it).
- Each specialized `.md` must carry a `color:` frontmatter key; it drives the
  colored role sub-line in the agent name badge.
- `provider` (a terminal CLI-agent slug, e.g. `claude`) and `models` (a list)
  are **advisory metadata** shown in the role picker. They **never** change the
  BLXCode Agent's runtime model — the BLXCode Agent always uses the
  provider/model from **Settings** (`AgentProviderSettings`). The role is a
  behavioural overlay only.
- The chosen slug travels per turn on `UserTurn.session_role` (mirrored in
  `src/agent_wire.rs`) and is appended to the shared prompt by
  `system_prompt(workspace_root, agent_name, session_role)` as a trailing
  `# Active session role` block. The block **ranks below** Security, the Agent
  Chat mode, and the explicit user request — it shapes working style only.
- Persistence: the slug lives on `WorkspaceEntry.agent_session_role`
  (`#[serde(default)]`), so it is restored with the workbench snapshot. The
  composer reads it via `agent_session_role_for_workspace_untracked` when
  building the turn.

### CLI-agent model and effort selection (fleet)

Each terminal CLI agent (the fleet assigned in Create-Workspace step 2) can run
on chosen model and, where the CLI supports a safe launch-time override,
reasoning effort:

- `terminal_agent_profiles.rs` carries a per-slug model catalog
  (`models: &[&str]`), `model_flag`, `effort_passing`, and selectable
  `efforts`.
- The fleet step renders a model `<select>` per assigned agent row (options from
  `terminal_agent_models(slug)`). It renders an effort `<select>` only when
  `terminal_agent_efforts(slug)` is non-empty.
- Choices are stored per agent row in `CreateWorkspaceDraft.agent_models[5]` and
  `CreateWorkspaceDraft.agent_efforts[5]`, then expanded on commit into
  `WorkspaceEntry.slot_agent_models` and `WorkspaceEntry.slot_agent_efforts`
  (parallel to `slot_agent_labels`) via `fleet_slot_models_for_labels` and
  `fleet_slot_efforts_for_labels`.
- At launch, `terminal_cell.rs` resolves slot values through
  `WorkbenchService::agent_model_for_terminal_key` and
  `agent_effort_for_terminal_key`, then passes both to
  `terminal_agent_launch_command`. Empty values pass no override, so the
  external CLI keeps its own defaults/config.
- Model and effort vectors are kept parallel across slot add/remove/swap.

Current launch mapping:

| CLI | Model launch override | Effort launch override |
|-----|------------------------|------------------------|
| Claude Code | `claude --model <id-or-alias>`; aliases `opus`, `sonnet`, `haiku` track the latest family model | `CLAUDE_CODE_EFFORT_LEVEL=<level> claude ...` (`low`, `medium`, `high`, `xhigh`, `max`) |
| Codex | `codex --model <id>` | `codex ... -c 'model_reasoning_effort="<level>"'` (`minimal`, `low`, `medium`, `high`, `xhigh`) |
| Gemini CLI | `gemini --model <id>` | Config-file only (`~/.gemini/settings.json`); BLXCode does not write it at launch |
| OpenCode | `opencode --model <provider/model>` | Config-file only (`reasoningEffort` in OpenCode config); BLXCode does not write it at launch |
| Cursor Agent | `cursor-agent --model <id>` | No confirmed launch flag in the installed CLI help; BLXCode leaves effort to Cursor defaults/config |

### Workspace presets

`workspace_presets.rs` stores reusable fleet configurations (terminal count,
per-agent counts, per-agent models/efforts, per-slot names, session role) in
`{app_data_dir}/workspace_presets.json` (atomic tmp+rename). It is **global per
installation**, not committed with a workspace. CRUD commands:
`workspace_presets_list` / `_save` / `_delete`, mirrored in `tauri_bridge.rs`
as `WorkspacePresetView`. The Create-Workspace UI applies a preset onto the
draft via `WorkbenchService::apply_preset_to_draft` and launches it directly.

## IPC commands (harness-specific)

Registered in `lib.rs`:

```text
agent_web_settings_get
agent_web_settings_save
agent_web_api_key_set
agent_web_api_key_delete
agent_environment_invalidate
agent_session_roles_list
workspace_presets_list
workspace_presets_save
workspace_presets_delete
```

Existing agent runtime commands unchanged; see [Tauri IPC](tauri-ipc.md).

## Tests

- `environment.rs` — cache invalidate test
- `skills_rules/store.rs` — core skill count, merge with user skills, remove guard
- `tool_groups.rs` — filtered registry tests if present
- Run `cargo test -p blxcode` before PRs touching harness code

## Extending the harness

### New core skill

1. Add `src-tauri/src/agent/harness_skills/<name>.md`
2. Append to `CORE_SKILLS` in `store.rs`
3. Add name to system prompt core-skill list and tool index if new tools
4. Optional: `core_skill_availability` hook in `store.rs`
5. Regenerate or hand-add i18n if UI strings are needed

### New server tool

1. `tools.rs` — `ToolDef` + `execute_server_tool` arm
2. `tool_groups.rs` — assign to group(s)
3. `tool_dispatch.rs` — if special casing needed
4. Document in harness skill Markdown
5. `AgTool*` i18n + `agent_timeline::tool_label` mapping

## Plans (reference)

- `.agents/plans/better-harness/plan.md` — core skills + slim prompt
- `.agents/plans/coordinated-subagents/plan.md` — subagents (see [Subagents](subagents.md))

## See also

- [Subagents](subagents.md) — orchestration, protocol, extension guide
- [Architecture](architecture.md) — workbench and agent overview
- [Tauri IPC](tauri-ipc.md) — full command list
- [Internationalization](i18n.md) — locale workflow
- [User: Agent Harness](../user/agent-harness.md) — end-user guide
