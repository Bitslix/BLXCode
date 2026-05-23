# Performance Optimization

## Summary

Zwei parallele Audits (Rust/Tauri-Backend + Leptos/WASM-Frontend) fuer schnellere Cargo-App. Groesste Latenz-Quellen:

1. **Agent-Streaming** — jedes Token-Delta persistiert Timeline → `workspaces`-Update → Auto-Save + Full-Rebuild + Markdown-Reparse
2. **Backend** — blockierende Tools auf Tokio-Worker; Mutex pro Stream-Chunk; 50 ms IPC-Poll
3. **Cold Start** — 13+ CSS-Links, Google Fonts, xterm von CDN/esm.sh, voller `workbench.json`-Load
4. **Terminal-Layout** — Refit-Sturm pro Zelle bei Sidebar/Panel-Resize

Positiv: statischer Boot-Screen, `wasm-opt=z`, lazy hljs/mermaid/graph3d, atomische Workbench-Writes.

Verwandt: Push-IPC in [v2-roadmap.md](v2-roadmap.md) (`push-ipc-events`); unabhaengig von [security-hardening.md](security-hardening.md).

Phasen P0 (Agent UX) → P1 (Backend/Boot) → P2 (Terminal/Git) → P3 (Polish).

## Decisions

- **P0 zuerst** — User-perceived speed beim Agent-Chat; kein Backend-Risiko.
- **Timeline-Persist:** Debounce 500–1000 ms waehrend Stream; immer flush bei `Done`/`Error`/App-Close.
- **Auto-Save:** Nicht auf gesamtes `workspaces().get()` — Timeline waehrend `agent.busy()` aus Snapshot-Trigger ausschliessen.
- **Timeline-UI:** Stabile Item-IDs + `<For key>`; Markdown nur streaming Row (debounced) oder Plaintext bis Turn-Ende.
- **Backend:** Kurzfristig `spawn_blocking`; mittelfristig `tokio::process` fuer Shell.
- **Push IPC:** Mit v2 `push-ipc-events` zusammenfuehren wenn moeglich.
- **Boot:** xterm lokal vendoren (wie hljs); Fonts self-host oder defer.
- **Terminal refit:** Globaler debounced ResizeObserver (~100 ms), max ~10 Versuche.

## Implementation Notes

### Befunde (PERF-01 … PERF-17)

| ID | Sev | Befund | Dateien |
|----|-----|--------|---------|
| PERF-01 | Critical | `persist_agent_timeline` bei jedem Delta | `agent_panel/timeline.rs` |
| PERF-02 | Critical | Auto-Save auf `workspaces().get()` | `workbench/mod.rs` |
| PERF-03 | Critical | Timeline ohne `<For key>`, MD-Reparse all rows | `agent_panel/mod.rs` |
| PERF-04 | Critical | `execute_server_tool` sync in async Turn | `tool_dispatch.rs`, `shell_exec.rs` |
| PERF-05–17 | High–Low | IPC poll, conversation clone, boot CDN, terminal refit, git watcher, etc. | diverse |

Hot path: `agent_poll_events` → `apply_agent_event` → `persist_agent_timeline` → `workspaces.update` → Auto-Save + Sidebar Effects + Timeline rebuild.

### P0 — Agent streaming (~2 PRs)

- Debounce `persist_agent_timeline`; flush on Done/Error.
- Auto-Save von timeline entkoppeln (`mod.rs`).
- `TimelineItem` stable id; `<For each key>`; inkrementelles Markdown.
- Compose-Draft separat debouncen.

### P1 — Backend + boot (~3 PRs)

- `spawn_blocking` fuer server tools; optional async shell.
- Adaptive poll 16/50/100 ms oder push-ipc.
- `Arc` conversation; xterm local vendor; split workbench load.
- `read_workspace_file` byte-cap; `memory_search` via rg/Index.

### P2 — Terminal + Git + caches (~2 PRs)

- Zentraler terminal refit debounce.
- Git status cache; settings/keyring cache; lazy keyring init.
- Notification events statt 1s poll; file-preview request token.

### P3 — Polish

- Tool catalog `OnceLock`; fsync on quit; chat autoscroll stream; benchmark doc.

### Erfolgskriterien

- Cold start −30–50%; token→UI p95 < 32 ms; Auto-save waehrend 30s Stream 0–2 IPC calls; resize < 200 ms stabil.

## Tests

- 100 rapid `AssistantDelta` → max 2 timeline persists (debounce + done).
- Agent busy → no `workbench_save_state` from timeline-only changes.
- Mid-stream message → earlier rows stable (For keys / thinking_open).
- Long `shell_exec` does not stall unrelated IPC.
- Offline boot → terminal without esm.sh.
- `cargo test --workspace`; wasm check.

## Tasks

### P0 Critical

- [ ] `perf-timeline-persist-debounce` - persist_agent_timeline debouncen; flush on Done/Error/quit
- [ ] `perf-autosave-decouple` - Auto-Save von agent_timeline/workspaces entkoppeln
- [ ] `perf-timeline-for-keys` - TimelineItem stable id + For key; thinking_open an id
- [ ] `perf-streaming-markdown` - Inkrementelles Markdown/Plaintext waehrend Stream
- [ ] `perf-compose-draft-debounce` - Compose-Draft persist entkoppeln/debouncen

### P1 High

- [ ] `perf-spawn-blocking-tools` - execute_server_tool via spawn_blocking
- [ ] `perf-shell-async` - shell_exec auf tokio::process (optional nach spawn_blocking)
- [ ] `perf-ipc-adaptive-poll` - Poll 16/50/100 ms; langfristig push-ipc (v2-roadmap)
- [ ] `perf-conversation-arc` - Arc/append-only conversation statt full clone
- [ ] `perf-xterm-vendor-local` - xterm JS/CSS lokal; terminal_bootstrap ohne esm.sh
- [ ] `perf-boot-split-snapshot` - workbench load: layout first, timeline lazy
- [ ] `perf-read-file-cap` - read_workspace_file byte-cap vor read
- [ ] `perf-memory-search-rg` - memory_search via rg oder Index

### P2 Medium

- [ ] `perf-terminal-refit-central` - Global debounced ResizeObserver; reduce retry loops
- [ ] `perf-git-status-cache` - Git status cache + unified watcher debounce
- [ ] `perf-settings-key-cache` - Settings/Keyring cache; lazy keyring init
- [ ] `perf-notify-events` - Notification event-driven statt 1s poll
- [ ] `perf-file-preview-token` - Async load request-generation guard

### P3 Polish

- [ ] `perf-tool-catalog-cache` - OnceLock tool catalog
- [ ] `perf-workbench-fsync` - sync_all nur on quit; laengeres debounce
- [ ] `perf-chat-autoscroll-stream` - Scroll waehrend Token-Streaming
- [ ] `perf-benchmark-doc` - Baseline-Metriken dokumentieren
