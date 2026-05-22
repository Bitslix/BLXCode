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
  openrouter.rs / anthropic.rs  # Use tool_dispatch

src/skills_rules/store.rs   # CORE_SKILLS, core SkillSourceKind, availability
src-tauri/src/api_keys.rs     # Central key catalog, resolve, api_keys_status/apply
src/workbench/
  harness_ui.rs               # SettingsDock, App/Appearance panes
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

## Frontend i18n

Tool and web labels use `I18nKey` variants (`AgWeb*`, `AgTool*`) in all `src/i18n/locales/*.rs`. Subagent-specific keys (`AgSubagent*`, `AgRole*`) are documented in [Subagents](subagents.md).

Skills panel: `SrSkillsTabCore`, `SrSkillsTabUser`, `SrSourceCore` — see [Internationalization](i18n.md).

## IPC commands (harness-specific)

Registered in `lib.rs`:

```text
agent_web_settings_get
agent_web_settings_save
agent_web_api_key_set
agent_web_api_key_delete
agent_environment_invalidate
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

- `.agents/plans/better-harness.md` — core skills + slim prompt
- `.agents/plans/coordinated-subagents.md` — subagents (see [Subagents](subagents.md))

## See also

- [Subagents](subagents.md) — orchestration, protocol, extension guide
- [Architecture](architecture.md) — workbench and agent overview
- [Tauri IPC](tauri-ipc.md) — full command list
- [Internationalization](i18n.md) — locale workflow
- [User: Agent Harness](../user/agent-harness.md) — end-user guide
