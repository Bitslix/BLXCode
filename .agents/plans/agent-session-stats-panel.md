# Agent-Session-Stats in der rechten Hero-Grid-Box

> Status: **planned** (recherchiert & verifiziert; Umsetzung noch nicht
> begonnen. Datenquellen im Code bestätigt — kein Backend nötig.)

## Summary

Der Agent-Header (`.agent-hero`) ist inzwischen ein 2-Spalten-Grid: **links**
der Drobo-Orb (Zeile 1) mit dem **State** („Standby/Running", Zeile 2) darunter,
**rechts** eine bislang leere `1fr`-Spalte. Diese rechte Box soll eine
kompakte, live-aktualisierte **Chat-Session-Statistik** anzeigen:

- **Context** — Fenster-Belegung (`9.6k / 400k · 2%`)
- **Turns** — Anzahl abgerechneter Runden
- **Tool calls** — Gesamtzahl
- **open / read / edit / rm** — Tool-Calls gebucketet nach Kategorie
- **Aktive Subagents** — laufende Subagents (Name + Rolle, mit Puls)

## Kontext / Ist-Zustand (verifiziert)

Alle Werte sind **client-seitig ableitbar** aus Daten, die `AgentPanelDock`
bereits hält — **keine `src-tauri`-Änderung nötig**:

- `timeline: RwSignal<TimelineDoc>` → `turns: Vec<TurnNode>` mit
  `parts: Vec<TurnPart>`. Relevante Varianten (`agent_timeline.rs:381`):
  - `TurnPart::Tool { tool, state, children: Vec<TurnPart>, .. }`
  - `TurnPart::Subagent { role, display_name, status: SubagentStatus, parts, .. }`
  - Tool-Calls können **verschachtelt** sein (Tool-`children` + Subagent-`parts`)
    → rekursiver Walk nötig.
- `wb.chat_usage_for_workspace(id) -> ChatUsageStats` (`state.rs:198`):
  `turn_count`, `total_cost_usd`, `last_round_input_tokens` (= Live-Context-Belegung).
- `context_length: RwSignal<Option<u64>>` (Fenstergröße, Default ~400k) — liegt
  bereits in `AgentPanelDock` (`mod.rs:61`).
- `busy: RwSignal<bool>` für Standby/Running.
- Vorbild für Stats-Lesen: `SessionCostChip` (`agent_panel/mod.rs:843`) nutzt
  `chat_usage_for_workspace` + `Memo` + i18n exakt so.
- Tool-Namen-Inventar: siehe `tool_icon()` (`timeline.rs:893`) und
  `file_arg_path()` (`agent_timeline.rs:787`).

## Tool-Klassifizierung (Heuristik nach Tool-Name)

Zentrale `fn tool_category(tool: &str) -> Option<ToolCategory>`:

- **read**: `read_workspace_file`, `memory_read|search|list|backlinks|graph`,
  `rules_read`, `skills_read`, `task_get|list`, `list_tools`,
  `harness.read_terminal_output`, `harness.list_terminals`
- **open**: `list_workspace_files`, `harness.open_terminal`,
  `harness.create_workspace`, `memory_context_attach`
- **edit**: `memory_create|write|rename|category_update`,
  `task_create|update|reorder`, `harness.send_terminal_keys`,
  `harness.send_agent_context`, `memory_context_detach`
- **rm**: `memory_delete`, `task_delete`

Nicht zugeordnete Tools zählen in `tool_total`, aber in keinen Bucket.
Feinschliff der Zuordnung bei der Umsetzung.

## Tasks / Phasen

**STATS-01 — Aggregator (testbar, isoliert)**
Neue Datei `src/workbench/agent_panel/session_stats.rs`:
- `struct SessionStats { tool_total, open, read, edit, rm: u32, active_subagents: Vec<(String, String)> }`
- `fn compute_session_stats(doc: &TimelineDoc) -> SessionStats` mit rekursivem
  Part-Walk (Tool-`children` + Subagent-`parts`), `tool_category`-Buckets und
  Sammeln laufender Subagents (`SubagentStatus::Running`).
- Unit-Tests analog `reducer.rs`/`timeline.rs`-Tests.

**STATS-02 — Komponente `AgentSessionStats`**
- Props: `timeline`, `wb`, `context_length`, `busy`.
- `Memo` auf `timeline` → `SessionStats`; Context/Turns aus
  `chat_usage_for_workspace` + `context_length` (gleiche Format-Helfer wie
  Chat-Header / `context_meter`).
- Rendert das Label/Wert-Raster + Subagent-Liste in die **rechte** Grid-Zelle.

**STATS-03 — Einbau in den Header**
- In `agent_panel/mod.rs` die rechte Spalte mit `<AgentSessionStats .. />`
  füllen (Grid-Spalte 2). Orb (Spalte 1, Zeile 1) und State (Spalte 1, Zeile 2)
  bleiben unverändert.

**STATS-04 — CSS**
- `.agent-session-stats` in `styles.css`: kompaktes Label/Wert-Raster, dezente
  Mini-Badges für open/read/edit/rm, Subagent-Liste mit Puls bei „running".
- Compact-Mode (`.agent-hero--compact`, maximierter Chat) blendet die Stats aus.

**STATS-05 — i18n**
- Neue `I18nKey`s: `AgStatsContext`, `AgStatsTurns`, `AgStatsToolCalls`,
  `AgStatsOpen`, `AgStatsRead`, `AgStatsEdit`, `AgStatsRm`, `AgStatsSubagents`.
- Pflicht in **allen** `i18n/locales/*.rs` (Compile-Exhaustiveness). EN-Strings
  setzen, Rest via `scripts/render_i18n_locales_from_en.py`.

**STATS-06 — Verifikation**
- `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
- `cargo test --workspace` (Aggregator-Tests)
- Live-Test im Tauri-Fenster: Werte aktualisieren reaktiv (auch während
  „Thinking"), Subagent-Puls sichtbar, Compact-Mode korrekt.

## Offene Designfrage

- Sollen **Cost/Tokens** zusätzlich in die Box (bisher im Chat-Header), oder
  bleibt das getrennt? Vorschlag: getrennt lassen, Box fokussiert auf
  Context/Turns/Toolcalls/Subagents.

## Risiken / Notizen

- Reiner Frontend-Change, geringes Risiko (keine IPC-/Protokoll-Änderung).
- Tool-Klassifizierung ist Heuristik; bei neuen Tools `tool_category` pflegen.
- `compute_session_stats` läuft pro `timeline`-Update — bei sehr langen
  Sessions ggf. memoisieren (Memo deckt das ab).
