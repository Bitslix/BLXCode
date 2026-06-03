# HeartBeat And Memory Indexer

## Summary

Implement a global BLXCode HeartBeat runtime and settings surface, then register a Memory Indexer service as the first internal HeartBeat service. HeartBeat is disabled by default, has a user-configurable interval clamped from 10 minutes to 24 hours, and runs asynchronously away from the main UI thread.

The Memory Indexer processes all open workspaces on each eligible tick. It builds managed Memory notes for Rules, Skills, and Plans so the existing Memory graph and Graph3D category clustering can consume them without a second graph source.

## Decisions

- HeartBeat v1 exposes only internal BLXCode services in the UI, but the backend registry is plugin-ready with service metadata such as `kind`, `source`, and `capabilities`.
- HeartBeat default is off. Memory Indexer default is off to avoid surprise LLM usage.
- HeartBeat interval default is 60 minutes and is persisted globally.
- Memory Indexer settings live under Settings -> Memory and use an independent provider/model. On first load they copy the current BLXCode Agent provider/model, then diverge.
- Memory Indexer uses an isolated one-shot LLM channel with its own system prompt. It does not mutate agent chat history and does not run tools.
- Indexing input is metadata plus bounded excerpts, not full unbounded file content.
- Generated index notes are harness-managed with frontmatter such as `managed: memory-indexer`, so user-authored notes are not overwritten.
- Memory categories are derived from the first folder under the memory root. Therefore generated notes must be written directly under category folders, not under an `index` folder.

## Implementation Notes

- Add a Tauri backend module for HeartBeat runtime state, persisted settings, service registry, scheduler loop, service snapshots, and IPC commands.
- Register `memory_indexer` as the first internal service. Its status view includes name, short description, enabled state, status, last call, next call, last response/status, and skip count.
- Keep one Memory Indexer run active per workspace. If a workspace run is still active on the next tick, skip that workspace. After three consecutive skips, mark the active run as `stalled` and surface that state.
- HeartBeat processes all open workspaces. The frontend sends the current open workspace list to the backend whenever workspace state changes.
- Workspace generated notes should use direct Memory categories:
  - `.agents/memory/rules/...`
  - `.agents/memory/skills/...`
  - `.agents/memory/plans/...`
- Global aggregate notes should use the same direct categories under global memory:
  - `~/.blxcode/memory/rules/...`
  - `~/.blxcode/memory/skills/...`
  - `~/.blxcode/memory/plans/...`
- Generate Hub plus Item notes for v1:
  - one managed overview/hub note per category and scope
  - one managed note per indexed Rule, Skill, and Plan
  - wikilinks between overviews and items, and between related items when detectable from metadata/excerpts
- Reuse the existing Memory graph behavior where the first path segment is the category and category hubs are built from available categories.
- Add Settings -> HeartBeat with global HeartBeat toggle, interval control, and the registered services table.
- Extend Settings -> Memory with Memory stats and Memory Indexer provider/model controls.
- Add a left-statusline process rotator that displays one visible left-side process item every three seconds when more than one is active. Include Memory Indexer while running or stalled without hiding center/right statusline content.

## Public APIs And Types

- Add Tauri commands:
  - `heartbeat_settings_get`
  - `heartbeat_settings_save`
  - `heartbeat_services_list`
  - `heartbeat_service_set_enabled`
  - `heartbeat_service_run_now`
  - `heartbeat_set_open_workspaces`
  - `memory_index_settings_get`
  - `memory_index_settings_save`
  - `memory_index_stats`
- Add frontend wire types:
  - `HeartbeatSettings { enabled, intervalMinutes }`
  - `HeartbeatServiceView { id, name, description, kind, source, capabilities, enabled, status, lastCall, nextCall, lastResponse, skipCount }`
  - `MemoryIndexSettings { provider, modelId }`
  - `MemoryIndexStats { workspaceCount, globalCount, lastIndexedAt, generatedFiles, warnings }`
- Use `memory_indexer` as the stable service id.

## Tests

- Backend unit tests for interval clamping, default settings, save/load persistence, and service enable/disable persistence.
- Scheduler tests for next-call calculation, disabled service skips, manual `run_now`, and all-open-workspaces dispatch.
- Memory Indexer tests for per-workspace concurrency, third-skip stalled state, and independent status per workspace.
- Rendering tests that generated note paths land under `rules`, `skills`, and `plans` categories, never under `index`.
- Source collection tests for Rules, Skills, and Plans metadata, including disabled entries, missing skill docs, plan task summaries, and unsafe path rejection.
- Frontend tests or manual verification for HeartBeat Settings, Memory Settings provider/model save flow, service table status updates, and left statusbar rotation.
- Manual integration: enable HeartBeat and Memory Indexer, trigger `Run now`, verify generated notes appear in Memory list and Graph3D clusters under Rules, Skills, and Plans.

## Tasks

- [ ] `heartbeat-backend` - Implement HeartBeat settings, scheduler runtime, service registry, status snapshots, and IPC commands.
- [ ] `workspace-sync` - Send all open frontend workspaces to the HeartBeat backend and keep the backend workspace set current.
- [ ] `memory-index-settings` - Add independent Memory Indexer provider/model settings and reuse existing provider/model/key infrastructure.
- [ ] `memory-indexer-service` - Implement the Memory Indexer service with per-workspace concurrency, skip counting, stalled marking, and manual run support.
- [ ] `memory-index-rendering` - Generate managed Rules, Skills, and Plans memory notes directly under their category folders with Graph3D-friendly wikilinks.
- [ ] `heartbeat-settings-ui` - Add Settings -> HeartBeat with global controls and registered service list.
- [ ] `memory-settings-ui` - Extend Settings -> Memory with stats and Memory Indexer provider/model controls.
- [ ] `statusbar-rotator` - Add left statusline rotation and Memory Indexer running/stalled indicator.
- [ ] `tests-and-verification` - Add backend/frontend tests and run the integration verification flow.
