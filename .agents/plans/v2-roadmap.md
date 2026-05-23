# BLXCode V2 Roadmap

## Summary

V2 ist **Trust Repair**: dokumentierte V1-Luecken und Agent-Bugs zuerst, dann Kanban-MVP, danach Agent-Tiefe und Workbench-UX. Nach 2-Agent-Review (Technik + Produkt) neu priorisiert und verschlankt.

Kernproblem: README, CHANGELOG 0.2.0 und `docs/user/plans.md` behaupten Kanban und vollstaendige Web-Tools als shipped — Code liefert das nicht. Tavily `web_search` funktioniert; nur `web_fetch` und Brave Search sind Stubs.

Critical Path V2.0: `plan_context_*` → Konversations-Persistenz → Kanban MVP → `web_fetch` → Docs-Sync.

Siehe auch: [kanban-board-view.md](kanban-board-view.md) (Full Kanban; MVP-Scope ist Teilmenge).

## Decisions

- **V2.0 = Trust Repair**, kein Feature-Sprawl.
- **`plan_context_*` vor Kanban** — live Agent-Bug (~0.5 PR); Pattern existiert in `memory_context_*`.
- **Konversations-Persistenz Option A** — serialisierte Backend-Messages pro Workspace, Groessen-Cap, Clear bei Reset; UI-Timeline bleibt parallel in `workbench.json`.
- **Kanban MVP in V2.0** — Karten-DnD + Status-Writeback + feste Spalten; Spalten-DnD/hide/show/Quick-add defer V2.0.1 (Rest in `kanban-board-view.md`).
- **`web_fetch` in V2.0**; Brave Search defer (Tavily reicht fuer Search).
- **Docs-Sync ist Release-Gate** — Kanban/Web-Claims erst nach Merge.
- **Stub-Cleanup defer V2.3+** — `provider.rs`, `agent_provider_status` sind Dev-Hygiene, nicht user-facing.
- **MCP defer V2.3+** — Competitive Epic, nicht V2.0.
- **Non-Goals:** Cloud-Auth, Multi-Turn-Parallelitaet, frei definierbare Kanban-Status.

## Implementation Notes

### V2.0 Trust Repair

**1. `plan_context_*` Client-Tools**
- Registriert in `src-tauri/src/agent/tools.rs`; fehlende Handler in `src/workbench/agent_panel/client_tools.rs`.
- Fix analog `memory_context_*`; attach → `AgentContextKind::PlanFile`.

**2. Konversations-Persistenz**
- RAM heute: `AgentEngineState.conversation` in `src-tauri/src/agent/state.rs`.
- Serialisieren in Workspace-Snapshot / eigene Datei; Rehydrieren bei Submit; Cap + Reset-Clear.

**3. Kanban MVP**
- Backend: `plan_task_list_all`, Status-Update-Writeback, `.blxcode/kanban/index.json` (card order).
- Frontend: `src/workbench/plans_panel/kanban/`, View-Mode Toggle; Parser aus `src-tauri/src/plans.rs` wiederverwenden.

**4. Docs-Sync**
- `README.md`, `CHANGELOG.md`, `docs/user/plans.md` an Code-Wahrheit; Web: „Tavily search + fetch".

**5. `web_fetch`**
- `src-tauri/src/agent/web_tools.rs` — HTML→Text oder Tavily Extract (eine Strategie).

**6. CI**
- `.github/workflows/pr-check.yml` — `cargo test --workspace` ergaenzen.

### V2.1+ (Referenz)

- Push IPC: `emit`/`listen` statt Turn-Drain-Poll (`tauri_bridge.rs`; Pattern bei `git_status_dirty`).
- `write_workspace_file` / `edit_workspace_file` mit Sandbox.
- File-Tab Edit+Save (`src/workbench/file_preview/`).
- Voice: AWS Polly Stub in `src-tauri/src/voice/tts.rs` fixen oder UI entfernen.
- Provider Google/Mistral/Grok (`src-tauri/src/api_keys.rs`).
- i18n: `task_list.rs` hardcoded EN; Plans/Rules/Skills Chords in `harness_ui.rs`.

### Release-Phasen

| Phase | Fokus | PRs |
|-------|-------|-----|
| V2.0 | plan_context + conv persist + Kanban MVP + web_fetch + docs + CI | 4–5 |
| V2.0.1 | Kanban Full (Spalten-DnD, hide/show, quick-add) | 1–2 |
| V2.1 | Push IPC, write tools, file editor, voice TTS, 1–2 Provider | 3–5 |
| V2.2 | Restliche Provider, i18n/Shortcuts, Subagent steps, Brave | 2–3 |
| V2.3+ | MCP, Ollama, Linux browser, Stub-Cleanup | Multi-sprint |

## Tests

- **plan_context:** Agent attach/detach ohne Timeout; Tool-Result ok; Context sichtbar im Agent-Panel.
- **conv persist:** Nach App-Neustart Modell-Kontext = letzte Session (bis Cap); Reset leert beides.
- **Kanban MVP:** Karten-DnD aktualisiert Markdown-Marker (`[ ]`, `[>]`, `[!]`, `[x]`, `[-]`); `PLANS.md` ignoriert.
- **web_fetch:** Agent liest oeffentliche URL; Fehler bei ungueltiger URL / Timeout.
- **CI:** PR-Workflow fuehrt `cargo test --workspace` aus.
- **Docs:** Keine shipped-Claims fuer nicht-implementierte Features.
- Frontend: `cargo check -p blxcode-ui --target wasm32-unknown-unknown`; Backend: `cargo check -p blxcode`.

## Tasks

### V2.0 Trust Repair

- [ ] `plan-context-handlers` - plan_context_list/attach/detach in client_tools.rs (analog memory_context)
- [ ] `conv-persist-backend` - Backend-Konversation serialisieren, Cap, Rehydrierung bei Submit
- [ ] `conv-persist-reset` - Clear bei agent_clear_conversation; UI + Backend synchron
- [ ] `kanban-mvp-backend` - plan_task_list_all, Status-Writeback, .blxcode/kanban/index.json (card order)
- [ ] `kanban-mvp-frontend` - Kanban-View Toggle, Karten-DnD, feste Status-Spalten
- [ ] `web-fetch-impl` - web_fetch in web_tools.rs (HTML→Text oder Tavily Extract)
- [ ] `docs-sync-gate` - README/CHANGELOG/docs/user/plans.md an Code-Wahrheit anpassen
- [ ] `ci-cargo-test` - cargo test --workspace in pr-check.yml

### V2.0.1 Kanban Full

- [ ] `kanban-columns-dnd` - Spaltenreihenfolge per DnD persistieren
- [ ] `kanban-columns-hide` - Leere Spalten aus-/einblenden
- [ ] `kanban-quick-add` - Quick-add pro Spalte + Zielplan-Auswahl

### V2.1 Agent + Workbench

- [ ] `push-ipc-events` - Agent-Events via Tauri emit/listen statt Poll-Drain
- [ ] `workspace-write-tools` - write_workspace_file / edit_workspace_file mit Sandbox
- [ ] `file-tab-edit-save` - File-Preview-Tab Edit+Save-Flow
- [ ] `voice-tts-stub` - AWS Polly/OpenRouter TTS implementieren oder aus UI entfernen
- [ ] `providers-google-mistral-grok` - LLM-Provider coming_soon aktivieren
- [ ] `i18n-shortcuts` - Agent-Task-i18n, Terminal-Tab, Plans/Rules/Skills Chords

### V2.2 Polish

- [ ] `brave-search` - Brave Search in web_tools.rs (optional)
- [ ] `subagent-mid-run-steps` - Subagent Step-Instrumentation im Runner

### V2.3+ Epics

- [ ] `mcp-integration` - MCP-Server-Client, Tool-Registry-Bridge, Settings-UI
- [ ] `local-models-ollama` - Ollama/lokale Modelle via InferenceProvider-Refactor
- [ ] `linux-native-browser` - WebKitGTK Child-Webview evaluieren
- [-] `stub-cleanup-defer` - provider.rs / agent_provider_status — defer, Dev-Hygiene
