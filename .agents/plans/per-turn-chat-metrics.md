# Per-Turn Chat Metrics & Session Cost

**Status:** planned

## Summary

Metriken (`in`, `out`, `ttft`, `tok/s`, `$cost`) pro Conversation-Turn statt globaler Footer-Zeile. Tool-Executions und Subagent-Runden/Tools gelten als eigene Turns. Session-Gesamtkosten erscheinen im Chatlog-Titel (Haupt-Agent + Subagents). Persistenz über bestehendes `workbench.json` (`WorkspaceEntry.agent_timeline` + `agent_chat_usage`).

## Decisions

- **Kein separates Cost-File** — Erweiterung von `workbench.json` via debounced auto-save (bestehendes Pattern).
- **Kostenberechnung:** OpenRouter-native `usage.cost` wenn vorhanden; sonst Token × Preis aus OpenRouter-Models-API-Cache. Direct Anthropic/OpenAI via Heuristik-Mapping auf OpenRouter-Preise.
- **Turn-Granularität:** Jede Provider-Runde = `ModelRound`; jede Tool-Execution = `ToolExec` (inkl. Subagents).
- **Subagent-Scope:** `TurnUsage.agent_id: Option<String>` — `None` = Haupt-Agent, `Some(id)` = Subagent-Karte.
- **Globaler Footer entfernen** — Metriken nur pro Zeile + Session-Total im Titel.

## Implementation Notes

### Ausgangslage

Heute: **ein** `AgentEvent::TurnUsage` pro User-Eingabe (Summe aller Provider-Runden in `openrouter.rs` / `anthropic.rs`). Frontend akkumuliert in `ChatUsageStats` und zeigt alles in `ChatUsageFooter`. `$cost` fehlt. Tool-Calls sind Timeline-Zeilen ohne Metriken, visuell als `ToolGroup` gebündelt (`compact_timeline`).

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

- `ToolActivity`: `call_id`, `metrics`
- `TimelineItem::Assistant`: `metrics`
- `SubagentCard`: `metrics` (aggregierte Modell-Runden)

**Session** (`state.rs` — `ChatUsageStats`):

- `total_cost_usd: f64`
- `turn_count` zählt alle Turns inkl. Tool-Executions und Subagents

### Protocol

`src-tauri/src/agent/protocol.rs` + `src/agent_wire.rs`:

```rust
TurnUsage {
    kind: TurnUsageKind,       // ModelRound | ToolExec
    agent_id: Option<String>,
    call_id: Option<String>,
    round_index: Option<u32>,
    input_tokens, output_tokens, ttft_ms, elapsed_ms,
    cost_usd: Option<f64>,
}
```

### Backend-Emission

**Haupt-Agent** (`openrouter.rs`, `anthropic.rs`):

- Nach jedem `run_one_round`: `TurnUsage { kind: ModelRound, ... }`
- Nach jedem `dispatch_tool`: `TurnUsage { kind: ToolExec, call_id, elapsed_ms }`
- Aggregiertes TurnUsage am Ende entfernen
- `StreamUsage`: `cost` parsen; Fallback via `pricing.rs`

**Subagents** (`subagent_runner.rs`):

- Nach jeder Provider-Runde: `TurnUsage { kind: ModelRound, agent_id: Some(id), ... }`
- Nach jedem Tool-Exec: `TurnUsage { kind: ToolExec, agent_id: Some(id), call_id, ... }`
- Token-Parsing für Subagents analog Haupt-Agent nachziehen (`OpenAiRoundResult` erweitern)

### Preis-Lookup

`src-tauri/src/agent/pricing.rs`:

- `ProviderModelEntry.pricing` aus OpenRouter Models API
- `resolve_cost(model_id, provider, input, output)`
- Direct-Provider ID-Mapping auf OpenRouter-Cache

### Frontend

`apply_agent_event` (`timeline.rs`):

| Event | Aktion |
|-------|--------|
| `ToolExec`, `agent_id: None` | Metriken an Haupt-Agent-`ToolActivity` (by `call_id`) |
| `ToolExec`, `agent_id: Some` | Metriken an Subagent-Tool (by `agent_id` + `call_id`) |
| `ModelRound`, `agent_id: None` | An Assistant-Zeile oder erstes Pending-Tool |
| `ModelRound`, `agent_id: Some` | An `SubagentCard.metrics` akkumulieren |
| Alle | `record_chat_turn_usage` + `total_cost_usd` |

**UI:**

- `TurnMetricsBar` unter User/Assistant/Tool/Subagent-Tool-Zeilen
- `compact_timeline`: Tools einzeln statt `ToolGroup`
- Chatlog-Titel: `$0.042 · N turns`
- `ChatUsageFooter` entfernen
- Subagent-Karten: Cost im Header + Metriken pro Tool

### Persistenz

- Alte Snapshots: `#[serde(default)]` → leere Anzeige
- Reset: Timeline-Metriken + Aggregat leeren

## Tests

- [ ] Langer Chat: Metriken pro Assistant-, Tool- und Subagent-Zeile sichtbar
- [ ] Subagent-Run: Kosten in Karte + Session-Titel
- [ ] Workspace-Reload: Metriken und Session-Cost aus `workbench.json` wiederhergestellt
- [ ] Chat-Reset: Titel-Cost und Zeilen-Metriken zurückgesetzt
- [ ] OpenRouter + Direct-Provider: Cost angezeigt oder `—` bei fehlendem Preis-Match
- [ ] `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
- [ ] `cargo test --workspace` (pricing/turn_metrics Unit-Tests)

## Tasks

- [ ] `turn-metrics-model` - TurnMetrics + TurnUsageKind; ToolActivity/SubagentCard/Assistant metrics; ChatUsageStats.total_cost_usd
- [ ] `pricing-module` - OpenRouter pricing in model cache; pricing.rs mit resolve_cost + Direct-Provider ID-Mapping
- [ ] `backend-emit` - TurnUsage pro ModelRound/ToolExec in openrouter.rs, anthropic.rs, subagent_runner.rs; StreamUsage.cost
- [ ] `frontend-apply` - apply_agent_event: Metriken an Timeline + SubagentCard; record_chat_turn_usage erweitern
- [ ] `ui-per-row` - TurnMetricsBar; Tools einzeln; Chat-Titel Session-Cost; Footer entfernen; Subagent-UI
- [ ] `i18n-styles` - I18n-Keys + CSS für per-row metrics bar; serde-Migration
