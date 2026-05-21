# Per-Turn Chat Metrics & Session Cost

**Status:** planned

## Summary

Metriken (`in`, `out`, `ttft`, `tok/s`, `$cost`) pro Conversation-Turn statt globaler Footer-Zeile. Tool-Executions und Subagent-Runden/Tools gelten als eigene Turns. Session-Gesamtkosten erscheinen im Chatlog-Titel (Haupt-Agent + Subagents). Persistenz über bestehendes `workbench.json` (`WorkspaceEntry.agent_timeline` + `agent_chat_usage`).

## Decisions

- **Kein separates Cost-File** — Erweiterung von `workbench.json` via debounced auto-save (bestehendes Pattern).
- **Kostenberechnung:** OpenRouter-native `usage.cost` wenn vorhanden (benötigt `usage: { include: true }` im Request); sonst Token × Preis aus OpenRouter-Models-API-Cache. Direct Anthropic/OpenAI über feste ID-Mapping-Tabelle auf OpenRouter-Preise; bei Miss → `—`.
- **Turn-Granularität:** Jede Provider-Runde = `ModelRound`; jede Tool-Execution = `ToolExec` (inkl. Subagents). User-Zeilen bekommen **keine** Metriken-Bar.
- **Subagent-Scope:** `TurnUsage.agent_id: Option<String>` — `None` = Haupt-Agent, `Some(id)` = Subagent-Karte.
- **Globaler Footer entfernen** — Metriken nur pro Zeile + Session-Total im Titel.
- **ModelRound-Zuordnung bei Tool-only-Runden:** ModelRound wird **immer** an die letzte Assistant-Zeile dieser Runde gehängt. Existiert keine (reine Tool-Runde), erhält die Runde eine dünne synthetische „decision"-Zeile direkt vor der ersten Tool-Zeile. Tools tragen nie ModelRound-Metriken — nur `ToolExec` (elapsed_ms, optional cost_usd).
- **TTFT-Semantik geändert:** Bisher nur First-Round-TTFT, fortgetragen. Künftig pro `ModelRound` separat (Zeit bis erstem Delta dieser Runde).
- **Late-Event-Guard:** `TurnUsage` nach `agent_clear_conversation` wird via Turn-Generation-Counter verworfen.

## Implementation Notes

### Ausgangslage

Heute: **ein** `AgentEvent::TurnUsage` pro User-Eingabe (Summe aller Provider-Runden in [openrouter.rs:382](src-tauri/src/agent/openrouter.rs#L382) / [anthropic.rs:314](src-tauri/src/agent/anthropic.rs#L314)). Frontend akkumuliert in `ChatUsageStats` ([state.rs:100](src/workbench/state.rs#L100)) und zeigt alles in `ChatUsageFooter` ([mod.rs:665](src/workbench/agent_panel/mod.rs#L665)). `$cost` fehlt. Tool-Calls sind Timeline-Zeilen ohne Metriken, visuell als `ToolGroup` gebündelt ([timeline.rs:531](src/workbench/agent_panel/timeline.rs#L531)). `call_id` ist auf `ToolCall` / `SubagentToolCall` bereits `Option<String>`.

### Datenmodell

Neues Modul `src/turn_metrics.rs` (+ Backend-Spiegel):

```rust
pub struct TurnMetrics {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub ttft_ms: Option<u64>,
    pub elapsed_ms: u64,
    pub cost_usd: Option<f64>,
}
```

**Timeline** (`agent_timeline.rs`):

- `ToolActivity`: `call_id`, `metrics` (alle `#[serde(default)]`)
- `TimelineItem::Assistant`: `metrics`
- `TimelineItem::ModelDecision` (neu, optional): synthetische Zeile für tool-only Rounds, trägt `metrics`
- `SubagentCard`: `metrics` (aggregierte Modell-Runden des Subagents)

**Session** (`state.rs` — `ChatUsageStats`):

- `total_cost_usd: f64` (+ `#[serde(default)]`)
- `turn_count` zählt alle Turns inkl. Tool-Executions und Subagents
- `ttft_sum_ms` / `ttft_sample_count` **entfallen** (TTFT lebt jetzt pro Zeile). Schema-Migration: Felder bei Deserialisierung ignorieren, beim nächsten Save fallen sie raus.
- `record_chat_turn_usage` bekommt neue Signatur: `(workspace_id, kind, input, output, ttft, elapsed, cost_usd, turn_generation)`.

### Protocol

`src-tauri/src/agent/protocol.rs` + `src/agent_wire.rs`:

```rust
TurnUsage {
    kind: TurnUsageKind,            // ModelRound | ToolExec
    agent_id: Option<String>,
    call_id: Option<String>,        // bei ToolExec gesetzt
    round_index: Option<u32>,
    turn_generation: u64,           // gegen Late-Events nach Reset
    input_tokens, output_tokens, ttft_ms, elapsed_ms,
    cost_usd: Option<f64>,
}
```

### Backend-Emission

**Haupt-Agent** ([openrouter.rs](src-tauri/src/agent/openrouter.rs), [anthropic.rs](src-tauri/src/agent/anthropic.rs)):

- Nach jedem `run_one_round`: `TurnUsage { kind: ModelRound, round_index, ttft_ms, ... }`
- Nach jedem `dispatch_tool`: `TurnUsage { kind: ToolExec, call_id, elapsed_ms, cost_usd: None }`
- Aggregiertes End-`TurnUsage` entfernen.
- OpenRouter-Request: `usage: { include: true }` setzen.
- `StreamUsage` ([openrouter.rs:93](src-tauri/src/agent/openrouter.rs#L93)) um `cost: Option<f64>` erweitern; Fallback via `pricing.rs::resolve_cost`.
- `turn_generation` aus `AgentEngineState` lesen (neues atomares Feld, wird bei `agent_clear_conversation` inkrementiert).

**Subagents** ([subagent_runner.rs](src-tauri/src/agent/subagent_runner.rs)):

- `OpenAiRoundResult` um Token-/TTFT-/Cost-Felder erweitern (Stream-Parser nachziehen — bisher null Usage-Erfassung).
- Nach jeder Provider-Runde: `TurnUsage { kind: ModelRound, agent_id: Some(id), ... }`.
- Nach jedem Tool-Exec: `TurnUsage { kind: ToolExec, agent_id: Some(id), call_id, ... }`.
- `call_id`-Garantie absichern: Falls Provider keinen liefert, synthetischen `subagent-{idx}` setzen, damit `(agent_id, call_id)`-Korrelation im Frontend eindeutig bleibt.

### Preis-Lookup

Neu `src-tauri/src/agent/pricing.rs`:

- `ProviderModelEntry` ([agent_settings.rs:47](src-tauri/src/agent_settings.rs#L47)) um `pricing: Option<{ prompt: f64, completion: f64 }>` erweitern; aus OpenRouter Models API (`/api/v1/models`) übernehmen.
- `resolve_cost(model_id, provider, input_tokens, output_tokens) -> Option<f64>`.
- **Direct-Provider-Mapping-Tabelle** (statisch, dokumentiert in `pricing.rs`):
  - `claude-opus-4-7` → `anthropic/claude-opus-4.7`
  - `claude-sonnet-4-6` → `anthropic/claude-sonnet-4.6`
  - `gpt-5` → `openai/gpt-5`
  - … (weitere bei Bedarf)
- Bei unbekanntem Mapping: `None` → UI zeigt `—`.

### Frontend

`apply_agent_event` ([timeline.rs:347](src/workbench/agent_panel/timeline.rs#L347)):

| Event | Aktion |
|-------|--------|
| `ToolExec`, `agent_id: None` | Metriken an Haupt-Agent-`ToolActivity` (by `call_id`) |
| `ToolExec`, `agent_id: Some` | Metriken an Subagent-Tool (by `agent_id` + `call_id`) |
| `ModelRound`, `agent_id: None` | An letzte Assistant-Zeile der Runde; falls keine → `ModelDecision`-Zeile einfügen |
| `ModelRound`, `agent_id: Some` | An `SubagentCard.metrics` akkumulieren |
| Alle | `record_chat_turn_usage` + `total_cost_usd`; **wenn `turn_generation` veraltet → drop** |

**UI:**

- Neue Komponente `workbench/agent_panel/turn_metrics_bar/` (eigener Ordner gem. `rule-reusable-components.md`, eigenes CSS).
- Bar unter Assistant-/Tool-/Subagent-Tool-/`ModelDecision`-Zeilen. User-Zeilen ohne Bar.
- `compact_timeline`: Tools einzeln rendern; `DisplayTimelineItem::ToolGroup` **entfernen** inkl. Match-Arme in [timeline.rs:779](src/workbench/agent_panel/timeline.rs#L779).
- Chatlog-Titel ([mod.rs:289](src/workbench/agent_panel/mod.rs#L289)): zusätzlich `$0.042 · N turns` (Format-String i18n).
- `ChatUsageFooter` ([mod.rs:665](src/workbench/agent_panel/mod.rs#L665)) entfernen inkl. Mount in [mod.rs:425](src/workbench/agent_panel/mod.rs#L425).
- Subagent-Karten: Cost im Header + Metriken pro Tool.

### i18n

Neue `I18nKey`-Enum-Einträge (alle 12 Locale-Dateien wegen Compile-Time-Exhaustiveness):

- `AgMetricsIn`, `AgMetricsOut` — Label-Prefix
- `AgMetricsTtft` — Label
- `AgMetricsTokPerSec` — Einheit/Suffix
- `AgMetricsCost` — Label
- `AgMetricsCostUnknown` — `—` Fallback
- `AgMetricsTurnsOne`, `AgMetricsTurnsMany` — Pluralisierung (zwei Keys, kein ICU)
- `AgMetricsTooltipIn`, `AgMetricsTooltipOut`, `AgMetricsTooltipTtft`, `AgMetricsTooltipSpeed`, `AgMetricsTooltipCost` — `title=`-Attribute (bisher hardcodiert englisch)
- `AgSessionCostTitle` — Format-String für Titel-Zeile (`{cost} · {turns}`)

Übersetzungen via `scripts/render_i18n_locales_from_en.py` (zuerst `--full` für die neue Sektion, dann manuell verfeinern).

### Persistenz

- Alte Snapshots: neue Felder via `#[serde(default)]` → leere Anzeige.
- `ttft_sum_ms` / `ttft_sample_count` in alten `ChatUsageStats`-Snapshots werden beim Deserialisieren ignoriert (Felder entfernt) und beim nächsten Save weggeschrieben.
- Reset (`clear_chat_usage` [state.rs:2257](src/workbench/state.rs#L2257)): Timeline-Metriken + `total_cost_usd` + `turn_count` zurück auf `default()`; `turn_generation` inkrementieren, damit in-flight Events verworfen werden.

## Tests

- [ ] Langer Chat: Metriken pro Assistant-, Tool- und Subagent-Zeile sichtbar
- [ ] Subagent-Run: Kosten in Karte + Session-Titel
- [ ] Tool-only Round: `ModelDecision`-Zeile wird eingefügt, Metriken landen dort und nicht beim Tool
- [ ] Workspace-Reload: Metriken und Session-Cost aus `workbench.json` wiederhergestellt
- [ ] Chat-Reset mit laufender Runde: späte `TurnUsage`-Events werden verworfen (Generation-Counter)
- [ ] Chat-Reset: Titel-Cost und Zeilen-Metriken zurückgesetzt
- [ ] OpenRouter mit `usage.cost`: nativer Wert > Token-Fallback
- [ ] OpenRouter ohne Pricing-Eintrag: `—` angezeigt
- [ ] Direct Anthropic mit gemappter ID: Cost via OpenRouter-Preise
- [ ] Direct Provider ohne Mapping: `—`
- [ ] Pluralisierung `turn` / `turns` in allen Locales
- [ ] `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
- [ ] `cargo test --workspace` (pricing/turn_metrics Unit-Tests)
- [ ] Pricing-Unit-Tests: id-Mapping hit/miss, OpenRouter-`usage.cost` Override schlägt Token-Berechnung

## Tasks

- [ ] `turn-metrics-model` — `TurnMetrics` + `TurnUsageKind`; `ToolActivity`/`SubagentCard`/`Assistant`/`ModelDecision` metrics-Felder; `ChatUsageStats.total_cost_usd`; `ttft_sum_ms`/`ttft_sample_count` entfernen (Schema-Migration); `record_chat_turn_usage`-Signaturwechsel (Breaking, alle Aufrufer mit)
- [ ] `pricing-module` — `ProviderModelEntry.pricing`; OpenRouter Models-API um Preise erweitern; `pricing.rs` mit `resolve_cost` + dokumentierter Direct-Provider-ID-Mapping-Tabelle + Unit-Tests
- [ ] `backend-emit-main` — `TurnUsage` pro `ModelRound`/`ToolExec` in `openrouter.rs` und `anthropic.rs`; `usage: { include: true }` für OpenRouter; `StreamUsage.cost` parsen; finales aggregiertes Event entfernen
- [ ] `backend-emit-subagent` — Token-/TTFT-/Cost-Erfassung in `subagent_runner.rs` (Greenfield, bisher keine Usage); `call_id`-Garantie; per-Round + per-Tool Events
- [ ] `late-event-guard` — `turn_generation` in `AgentEngineState` (atomic), Inkrement in `agent_clear_conversation`; Event-Drop im Frontend bei veralteter Generation
- [ ] `frontend-apply` — `apply_agent_event`: 4 Routing-Fälle gem. Tabelle; `record_chat_turn_usage` mit neuer Signatur; `ModelDecision`-Insertion-Logik
- [ ] `ui-per-row` — `turn_metrics_bar/` Komponente (eigener Ordner + CSS); Tools einzeln (`ToolGroup`-Variant entfernen); Chat-Titel Session-Cost; `ChatUsageFooter` entfernen; Subagent-Card-Header mit Cost
- [ ] `i18n-keys` — Neue Keys (siehe Liste) + Strings in allen 12 Locale-Dateien via Render-Skript; Pluralisierungs-Keys; Tooltip-Strings; Titel-Format-String
