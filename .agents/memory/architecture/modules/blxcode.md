---
title: blxcode modules
enabled: true
tags: ["architecture"]
managed: static
stale: false
git_rev: d366fffff77386626c1d371e043344a023099d41
source_paths: ["src-tauri/src/agent/anthropic.rs", "src-tauri/src/agent/environment.rs", "src-tauri/src/agent/git_agent.rs", "src-tauri/src/agent/mod.rs", "src-tauri/src/agent/openrouter.rs", "src-tauri/src/agent/pricing.rs", "src-tauri/src/agent/project_docs.rs", "src-tauri/src/agent/protocol.rs", "src-tauri/src/agent/provider.rs", "src-tauri/src/agent/session_orchestrator.rs", "src-tauri/src/agent/shell_exec.rs", "src-tauri/src/agent/state.rs", "src-tauri/src/agent/subagent_prompts.rs", "src-tauri/src/agent/subagent_runner.rs", "src-tauri/src/agent/subagents.rs", "src-tauri/src/agent/system_prompt.rs", "src-tauri/src/agent/tool_dispatch.rs", "src-tauri/src/agent/tool_groups.rs", "src-tauri/src/agent/tools.rs", "src-tauri/src/agent/tools_extra.rs", "src-tauri/src/agent/web_commands.rs", "src-tauri/src/agent/web_settings.rs", "src-tauri/src/agent/web_tools.rs", "src-tauri/src/agent/workspace_agent.rs", "src-tauri/src/agent_hooks.rs", "src-tauri/src/agent_settings.rs", "src-tauri/src/agents_layout.rs", "src-tauri/src/api_keys.rs", "src-tauri/src/app_paths.rs", "src-tauri/src/browser_host.rs", "src-tauri/src/clipboard.rs", "src-tauri/src/commands.rs", "src-tauri/src/fs_entries.rs", "src-tauri/src/git_graph.rs", "src-tauri/src/git_info.rs", "src-tauri/src/git_status.rs", "src-tauri/src/git_sync.rs", "src-tauri/src/image/commands.rs", "src-tauri/src/image/generate.rs", "src-tauri/src/image/mod.rs", "src-tauri/src/image/settings.rs", "src-tauri/src/lib.rs", "src-tauri/src/main.rs", "src-tauri/src/media_keys.rs", "src-tauri/src/memory/architecture/mod.rs", "src-tauri/src/memory/architecture/state.rs", "src-tauri/src/memory/architecture/static_index.rs", "src-tauri/src/memory/frontmatter.rs", "src-tauri/src/memory/graph.rs", "src-tauri/src/memory/mod.rs", "src-tauri/src/memory/paths.rs", "src-tauri/src/memory/store.rs", "src-tauri/src/memory/types.rs", "src-tauri/src/memory/wikilinks.rs", "src-tauri/src/plans.rs", "src-tauri/src/pointers/mod.rs", "src-tauri/src/pty_host.rs", "src-tauri/src/skills_rules/commands.rs", "src-tauri/src/skills_rules/install.rs", "src-tauri/src/skills_rules/mod.rs", "src-tauri/src/skills_rules/pointers.rs", "src-tauri/src/skills_rules/store.rs", "src-tauri/src/skills_rules/types.rs", "src-tauri/src/tasks.rs", "src-tauri/src/updater.rs", "src-tauri/src/voice/commands.rs", "src-tauri/src/voice/mod.rs", "src-tauri/src/voice/recorder.rs", "src-tauri/src/voice/settings.rs", "src-tauri/src/voice/stt.rs", "src-tauri/src/voice/tts.rs", "src-tauri/src/workbench_state.rs"]
---
# blxcode Modules

Manual notes about this crate can live above or below the generated block.

<!-- architecture:static:begin -->
## `blxcode`

- Manifest: `src-tauri/Cargo.toml`
- Source root: `src-tauri/src`
- Rust sources: 72
- Root declarations: `agent`, `agent_hooks`, `agent_settings`, `agents_layout`, `api_keys`, `app_paths`, `browser_host`, `clipboard`, `commands`, `fs_entries`, `git_graph`, `git_info`, `git_status`, `git_sync`, `image`, `media_keys`, `memory`, `plans`, `pointers`, `pty_host`, `skills_rules`, `tasks`, `updater`, `voice`, `workbench_state`

### Top-Level Modules

- `agent` — submodules: `anthropic`, `environment`, `git_agent`, `openrouter`, `pricing`, `project_docs`, `protocol`, `provider`, `session_orchestrator`, `shell_exec`, `state`, `subagent_prompts`, `subagent_runner`, `subagents`, `system_prompt`, `tool_dispatch`, `tool_groups`, `tools`, `tools_extra`, `web_commands`, `web_settings`, `web_tools`, `workspace_agent`; declarations: `anthropic`, `environment`, `git_agent`, `openrouter`, `project_docs`, `provider`, `session_orchestrator`, `shell_exec`, `subagent_prompts`, `subagent_runner`, `subagents`, `system_prompt`, `tests`, `tool_dispatch`, `tool_groups`, `tools`, `tools_extra`, `web_commands`, `web_tools`, `workspace_agent`
- `agent_hooks`
- `agent_settings`
- `agents_layout`; declarations: `tests`
- `api_keys`
- `app_paths`; declarations: `test_support`, `tests`
- `browser_host`
- `clipboard`
- `commands`; declarations: `tests`
- `fs_entries`; declarations: `tests`
- `git_graph`; declarations: `tests`
- `git_info`; declarations: `tests`
- `git_status`; declarations: `tests`
- `git_sync`; declarations: `tests`
- `image` — submodules: `commands`, `generate`, `settings`; declarations: `commands`, `generate`, `settings`, `tests`
- `media_keys`
- `memory` — submodules: `architecture`, `frontmatter`, `graph`, `paths`, `store`, `types`, `wikilinks`; declarations: `architecture_guard_tests`, `frontmatter`, `graph`, `paths`, `pointer_tests`, `state`, `static_index`, `store`, `tests`, `types`, `wikilinks`; 2 deeper source files aggregated here
- `plans`; declarations: `tests`
- `pointers`; declarations: `tests`
- `pty_host`
- `skills_rules` — submodules: `commands`, `install`, `pointers`, `store`, `types`; declarations: `commands`, `install`, `pointers`, `store`, `tests`, `types`
- `tasks`; declarations: `tests`
- `updater`; declarations: `tests`
- `voice` — submodules: `commands`, `recorder`, `settings`, `stt`, `tts`; declarations: `commands`, `recorder`, `settings`, `stt`, `tts`
- `workbench_state`

### Source Paths

- `src-tauri/src/agent/anthropic.rs`
- `src-tauri/src/agent/environment.rs`
- `src-tauri/src/agent/git_agent.rs`
- `src-tauri/src/agent/mod.rs`
- `src-tauri/src/agent/openrouter.rs`
- `src-tauri/src/agent/pricing.rs`
- `src-tauri/src/agent/project_docs.rs`
- `src-tauri/src/agent/protocol.rs`
- `src-tauri/src/agent/provider.rs`
- `src-tauri/src/agent/session_orchestrator.rs`
- `src-tauri/src/agent/shell_exec.rs`
- `src-tauri/src/agent/state.rs`
- `src-tauri/src/agent/subagent_prompts.rs`
- `src-tauri/src/agent/subagent_runner.rs`
- `src-tauri/src/agent/subagents.rs`
- `src-tauri/src/agent/system_prompt.rs`
- `src-tauri/src/agent/tool_dispatch.rs`
- `src-tauri/src/agent/tool_groups.rs`
- `src-tauri/src/agent/tools.rs`
- `src-tauri/src/agent/tools_extra.rs`
- `src-tauri/src/agent/web_commands.rs`
- `src-tauri/src/agent/web_settings.rs`
- `src-tauri/src/agent/web_tools.rs`
- `src-tauri/src/agent/workspace_agent.rs`
- `src-tauri/src/agent_hooks.rs`
- `src-tauri/src/agent_settings.rs`
- `src-tauri/src/agents_layout.rs`
- `src-tauri/src/api_keys.rs`
- `src-tauri/src/app_paths.rs`
- `src-tauri/src/browser_host.rs`
- `src-tauri/src/clipboard.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/fs_entries.rs`
- `src-tauri/src/git_graph.rs`
- `src-tauri/src/git_info.rs`
- `src-tauri/src/git_status.rs`
- `src-tauri/src/git_sync.rs`
- `src-tauri/src/image/commands.rs`
- `src-tauri/src/image/generate.rs`
- `src-tauri/src/image/mod.rs`
- `src-tauri/src/image/settings.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/media_keys.rs`
- `src-tauri/src/memory/architecture/mod.rs`
- `src-tauri/src/memory/architecture/state.rs`
- `src-tauri/src/memory/architecture/static_index.rs`
- `src-tauri/src/memory/frontmatter.rs`
- `src-tauri/src/memory/graph.rs`
- `src-tauri/src/memory/mod.rs`
- `src-tauri/src/memory/paths.rs`
- `src-tauri/src/memory/store.rs`
- `src-tauri/src/memory/types.rs`
- `src-tauri/src/memory/wikilinks.rs`
- `src-tauri/src/plans.rs`
- `src-tauri/src/pointers/mod.rs`
- `src-tauri/src/pty_host.rs`
- `src-tauri/src/skills_rules/commands.rs`
- `src-tauri/src/skills_rules/install.rs`
- `src-tauri/src/skills_rules/mod.rs`
- `src-tauri/src/skills_rules/pointers.rs`
- `src-tauri/src/skills_rules/store.rs`
- `src-tauri/src/skills_rules/types.rs`
- `src-tauri/src/tasks.rs`
- `src-tauri/src/updater.rs`
- `src-tauri/src/voice/commands.rs`
- `src-tauri/src/voice/mod.rs`
- `src-tauri/src/voice/recorder.rs`
- `src-tauri/src/voice/settings.rs`
- `src-tauri/src/voice/stt.rs`
- `src-tauri/src/voice/tts.rs`
- `src-tauri/src/workbench_state.rs`
<!-- architecture:static:end -->
