---
title: blxcode (rust)
enabled: true
tags: ["architecture"]
managed: static
kind: rust
stale: false
git_rev: 2bd555e33cd66810235467822b853e084e4b7a45
source_paths: ["src-tauri/src/agent/anthropic.rs", "src-tauri/src/agent/environment.rs", "src-tauri/src/agent/git_agent.rs", "src-tauri/src/agent/mod.rs", "src-tauri/src/agent/openrouter.rs", "src-tauri/src/agent/pricing.rs", "src-tauri/src/agent/project_docs.rs", "src-tauri/src/agent/protocol.rs", "src-tauri/src/agent/provider.rs", "src-tauri/src/agent/session_orchestrator.rs", "src-tauri/src/agent/shell_exec.rs", "src-tauri/src/agent/state.rs", "src-tauri/src/agent/subagent_prompts.rs", "src-tauri/src/agent/subagent_runner.rs", "src-tauri/src/agent/subagents.rs", "src-tauri/src/agent/system_prompt.rs", "src-tauri/src/agent/tool_dispatch.rs", "src-tauri/src/agent/tool_groups.rs", "src-tauri/src/agent/tools.rs", "src-tauri/src/agent/tools_extra.rs", "src-tauri/src/agent/web_commands.rs", "src-tauri/src/agent/web_settings.rs", "src-tauri/src/agent/web_tools.rs", "src-tauri/src/agent/workspace_agent.rs", "src-tauri/src/agent_hooks.rs", "src-tauri/src/agent_settings.rs", "src-tauri/src/agents_layout.rs", "src-tauri/src/api_keys.rs", "src-tauri/src/app_paths.rs", "src-tauri/src/browser_host.rs", "src-tauri/src/clipboard.rs", "src-tauri/src/commands.rs", "src-tauri/src/fs_entries.rs", "src-tauri/src/git_graph.rs", "src-tauri/src/git_info.rs", "src-tauri/src/git_status.rs", "src-tauri/src/git_sync.rs", "src-tauri/src/image/commands.rs", "src-tauri/src/image/generate.rs", "src-tauri/src/image/mod.rs", "src-tauri/src/image/settings.rs", "src-tauri/src/lib.rs", "src-tauri/src/main.rs", "src-tauri/src/media_keys.rs", "src-tauri/src/memory/architecture/common.rs", "src-tauri/src/memory/architecture/detect.rs", "src-tauri/src/memory/architecture/indexers/cmake.rs", "src-tauri/src/memory/architecture/indexers/generic.rs", "src-tauri/src/memory/architecture/indexers/go.rs", "src-tauri/src/memory/architecture/indexers/jai.rs", "src-tauri/src/memory/architecture/indexers/make.rs", "src-tauri/src/memory/architecture/indexers/mod.rs", "src-tauri/src/memory/architecture/indexers/node.rs", "src-tauri/src/memory/architecture/indexers/python.rs", "src-tauri/src/memory/architecture/indexers/rust.rs", "src-tauri/src/memory/architecture/indexers/zig.rs", "src-tauri/src/memory/architecture/mod.rs", "src-tauri/src/memory/architecture/state.rs", "src-tauri/src/memory/architecture/static_index.rs", "src-tauri/src/memory/architecture/unit.rs", "src-tauri/src/memory/frontmatter.rs", "src-tauri/src/memory/graph.rs", "src-tauri/src/memory/mod.rs", "src-tauri/src/memory/paths.rs", "src-tauri/src/memory/store.rs", "src-tauri/src/memory/types.rs", "src-tauri/src/memory/wikilinks.rs", "src-tauri/src/plans.rs", "src-tauri/src/pointers/mod.rs", "src-tauri/src/pty_host.rs", "src-tauri/src/skills_rules/commands.rs", "src-tauri/src/skills_rules/install.rs", "src-tauri/src/skills_rules/mod.rs", "src-tauri/src/skills_rules/pointers.rs", "src-tauri/src/skills_rules/store.rs", "src-tauri/src/skills_rules/types.rs", "src-tauri/src/tasks.rs", "src-tauri/src/updater.rs", "src-tauri/src/voice/commands.rs", "src-tauri/src/voice/mod.rs", "src-tauri/src/voice/recorder.rs", "src-tauri/src/voice/settings.rs", "src-tauri/src/voice/stt.rs", "src-tauri/src/voice/tts.rs", "src-tauri/src/workbench_state.rs"]
---
# blxcode (rust)

Manual notes about this unit can live above or below the generated block.

<!-- architecture:static:begin -->
## `blxcode`

- Kind: `rust`
- Manifest: `src-tauri/Cargo.toml`
- Root: `src-tauri`
- Source root: `src-tauri/src`
- Source files: 86
- Root declarations: `agent`, `agent_hooks`, `agent_settings`, `agents_layout`, `api_keys`, `app_paths`, `browser_host`, `clipboard`, `commands`, `fs_entries`, `git_graph`, `git_info`, `git_status`, `git_sync`, `image`, `media_keys`, `memory`, `plans`, `plans_index`, `pointers`, `pty_host`, `skills_rules`, `tasks`, `updater`, `voice`, `workbench_state`

### Top-Level Modules

- `agent` (24 files) — submodules: `anthropic`, `environment`, `git_agent`, `openrouter`, `pricing`, `project_docs`, `protocol`, `provider`, `session_orchestrator`, `shell_exec`, `state`, `subagent_prompts`, `subagent_runner`, `subagents`, `system_prompt`, `tool_dispatch`, `tool_groups`, `tools`, `tools_extra`, `web_commands`, `web_settings`, `web_tools`, `workspace_agent`; declarations: `anthropic`, `environment`, `git_agent`, `openrouter`, `project_docs`, `provider`, `session_orchestrator`, `shell_exec`, `subagent_prompts`, `subagent_runner`, `subagents`, `system_prompt`, `tests`, `tool_dispatch`, `tool_groups`, `tools`, `tools_extra`, `web_commands`, `web_tools`, `workspace_agent`
- `agent_hooks` (1 files)
- `agent_settings` (1 files)
- `agents_layout` (1 files); declarations: `tests`
- `api_keys` (1 files)
- `app_paths` (1 files); declarations: `test_support`, `tests`
- `browser_host` (1 files)
- `clipboard` (1 files)
- `commands` (1 files); declarations: `tests`
- `fs_entries` (1 files); declarations: `tests`
- `git_graph` (1 files); declarations: `tests`
- `git_info` (1 files); declarations: `tests`
- `git_status` (1 files); declarations: `tests`
- `git_sync` (1 files); declarations: `tests`
- `image` (4 files) — submodules: `commands`, `generate`, `settings`; declarations: `commands`, `generate`, `settings`, `tests`
- `media_keys` (1 files)
- `memory` (23 files) — submodules: `architecture`, `frontmatter`, `graph`, `paths`, `store`, `types`, `wikilinks`; declarations: `architecture_guard_tests`, `cmake`, `common`, `detect`, `frontmatter`, `generic`, `go`, `graph`, `indexers`, `jai`, `make`, `node`, `paths`, `pointer_tests`, `python`, `rust`, `state`, `static_index`, `store`, `tests`, `types`, `unit`, `wikilinks`, `zig`; 15 deeper source files aggregated here
- `plans` (1 files); declarations: `tests`
- `plans_index` (1 files); declarations: `tests`
- `pointers` (1 files); declarations: `tests`
- `pty_host` (1 files)
- `skills_rules` (6 files) — submodules: `commands`, `install`, `pointers`, `store`, `types`; declarations: `commands`, `install`, `pointers`, `store`, `tests`, `types`
- `tasks` (1 files); declarations: `tests`
- `updater` (1 files); declarations: `tests`
- `voice` (6 files) — submodules: `commands`, `recorder`, `settings`, `stt`, `tts`; declarations: `commands`, `recorder`, `settings`, `stt`, `tts`
- `workbench_state` (1 files)

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
- `src-tauri/src/memory/architecture/common.rs`
- `src-tauri/src/memory/architecture/detect.rs`
- `src-tauri/src/memory/architecture/indexers/cmake.rs`
- `src-tauri/src/memory/architecture/indexers/generic.rs`
- `src-tauri/src/memory/architecture/indexers/go.rs`
- `src-tauri/src/memory/architecture/indexers/jai.rs`
- `src-tauri/src/memory/architecture/indexers/make.rs`
- `src-tauri/src/memory/architecture/indexers/mod.rs`
- `src-tauri/src/memory/architecture/indexers/node.rs`
- `src-tauri/src/memory/architecture/indexers/python.rs`
- `src-tauri/src/memory/architecture/indexers/rust.rs`
- `src-tauri/src/memory/architecture/indexers/zig.rs`
- `src-tauri/src/memory/architecture/mod.rs`
- `src-tauri/src/memory/architecture/state.rs`
- `src-tauri/src/memory/architecture/static_index.rs`
- `src-tauri/src/memory/architecture/unit.rs`
- `src-tauri/src/memory/frontmatter.rs`
- `src-tauri/src/memory/graph.rs`
- `src-tauri/src/memory/mod.rs`
- `src-tauri/src/memory/paths.rs`
- `src-tauri/src/memory/store.rs`
- `src-tauri/src/memory/types.rs`
- `src-tauri/src/memory/wikilinks.rs`
- `src-tauri/src/plans.rs`
- `src-tauri/src/plans_index.rs`
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
- ... 6 more source paths omitted
<!-- architecture:static:end -->
