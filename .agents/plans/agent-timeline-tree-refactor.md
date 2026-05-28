# Agent-Timeline Tree-Refactor: stabile IDs, reaktives Streaming, verschachtelte Subagents

Status: done

## Kontext

Der AgentTab im rechten Panel hat drei strukturelle Bugs, die alle aus
derselben Wurzel kommen — eine **flache Event-Liste, ohne stabile IDs
und ohne pro-Part Reaktivität**:

1. **Live-Streaming friert ein.** `TimelineRow` bindet `text` / `done` /
   `metrics` *per Wert* — z. B.

   ```rust
   // src/workbench/agent_panel/timeline.rs:977
   <div class="workbench-agent-markdown"
        inner_html=render_markdown_to_html(&text)></div>
   ```

   Die `<For key=|row| row.idx>` Loop in
   `src/workbench/agent_panel/mod.rs:442–461` reused DOM, wenn der Key
   gleich bleibt. Nachfolgende `AssistantDelta` / `ThinkingDelta` werden
   in den Backing-Vec geschrieben, aber das DOM bleibt auf dem ersten
   Chunk eingefroren. App-Neustart zeigt den vollen persistierten Text
   — bestätigt, dass die Daten korrekt sind, nur die View nicht.

2. **`subagents.run`-Tool-Call fehlt im Event-Stream.**

   ```rust
   // src-tauri/src/agent/tool_dispatch.rs:26–45 (gekürzt)
   if name == "subagents.run" {
       return crate::agent::subagents::run(state, args, root, c).await;
   }
   // … erst HIER würde der ToolCall-Push laufen:
   state.push(AgentEvent::ToolCall { tool, call_id, args });
   ```

   Der frühe `return` umgeht das `ToolCall`-Event. Das Frontend sieht
   zuerst `SubagentStarted`/`SubagentStep`/`SubagentToolCall` und erst
   danach `ToolResult` für die Subagent-Action — der
   "Run subagents"-Eintrag taucht *nach* der Subagent-Bubble auf (siehe
   Screenshot Row 18 vor Row 19).

3. **Verschachtelung fehlt.** Subagent-Aktivität (Output + eigene
   Tool-Calls) wird als **Geschwister** des `Tool(subagents.run)`
   gerendert, nicht als Kinder. Ein Subagent-Tool-Call wirkt für den
   User wie ein Main-Agent-Round (Screenshot Row 21 "MODEL ROUND"
   gehört zum Subagent, nicht zum Main-Agent). Zusätzlich verzögert
   `subagent_debounce.rs` Subagent-Events um 50 ms — das verschärft
   Race-Conditions mit nicht-debouncten Main-Events.

Best-Practice-Recherche (Vercel AI SDK `UIMessage.parts`, assistant-ui,
Composio Agent-Tool-Calling Guide 2026, AG-UI, hackernoon
"tool-call render pattern") konvergiert auf dasselbe Modell:

- **Message = geordnete Liste typisierter Parts** (Text, Tool,
  Thinking, Sub-Conversation), nicht eine Folge unzusammenhängender
  Rows.
- Jeder Part hat eine **stabile ID** — provider-issued `call_id` für
  Tools, monoton steigende `seq` für alles andere. Keine Array-Indizes
  als `<For>`-Keys.
- Tool-Calls haben eine **State-Maschine**
  (`pending → running → success | error`); UI rendert pro State.
- **Parent-Child** via `parent_call_id` (Tool-Call → ToolResult,
  Subagent-Aktivität → ihr `subagents.run`-Tool-Call), nicht via
  Adjazenz im Vec.
- Streaming-Text-Deltas wachsen **in derselben Part-ID**, nicht als
  neue Rows. UI subscribed pro Part-Signal.

Ziel: vollständiger sauberer Refactor des Timeline-Datenmodells. Drei
Antworten vom User legen die Form fest:

- **Subagent-Render**: voll rekursiv (Sub-of-Sub möglich).
- **Migration**: verlustfrei auto-migrate alter `sessions.json`.
- **ID-Strategie**: backend-issued `seq` + provider-issued `call_id`.

## Architektur

### 1. Event-Stream-Vertrag: Envelope mit `seq` und `parent_call_id`

`src-tauri/src/agent/protocol.rs` bekommt einen Envelope, der die
flachen AgentEvent-Varianten umschließt — Wire-Format zum Frontend.
Bestehende Pull-Konsumenten (`agent_poll_events`) liefern dann
`Vec<EventEnvelope>` statt `Vec<AgentEvent>`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub seq: u64,                       // monoton, pro Turn resettet
    pub parent_call_id: Option<String>, // None = Top-Level; Some(cid) = Child von Tool cid
    #[serde(flatten)]
    pub event: AgentEvent,
}
```

`AgentEngineState` (`src-tauri/src/agent/state.rs`) verwaltet zwei
neue Felder:

```rust
pub struct AgentEngineState {
    // …
    next_seq: AtomicU64,
    parent_stack: Mutex<Vec<String>>,   // current parent call_id chain
}
```

`push(event)` wickelt automatisch:
- `seq = next_seq.fetch_add(1, Relaxed)`
- `parent_call_id = parent_stack.lock().last().cloned()`
- pusht `EventEnvelope { seq, parent_call_id, event }` in die Queue.

Reset: `start_turn()` setzt `next_seq` auf 0 und leert
`parent_stack`. So sind `seq`-Werte Turn-lokal aber innerhalb eines
Turns streng monoton — Tiebreaker für Render-Ordnung, Stable-IDs
ableitbar.

### 2. Parent-Stack Push/Pop um Tool-Dispatch

`src-tauri/src/agent/tool_dispatch.rs` wird umstrukturiert. Push
**immer** `ToolCall` zuerst, dann Stack-Push der `call_id`, dann
Dispatch, dann Stack-Pop, dann `ToolResult`. Sonderfall
`subagents.run` entfällt komplett — Subagent-Events erben automatisch
den Parent.

```rust
state.push(AgentEvent::ToolCall {
    tool: name.to_owned(),
    call_id: Some(call_id.to_owned()),
    args: Some(args.clone()),
});
state.push_parent(call_id.to_owned());

let outcome = if name == "subagents.run" {
    crate::agent::subagents::run(state, args, root, ctx).await
} else {
    /* bestehender Pfad */
};

state.pop_parent();
state.push(AgentEvent::ToolResult {
    tool: name.to_owned(),
    ok: outcome.ok,
    message: Some(truncate_for_ui(&outcome.content)),
});
```

Konsistenz mit allen anderen Tools, keine fehlenden `ToolCall`-Events
mehr.

### 3. Subagent-Runner unverändert lassen

`src-tauri/src/agent/subagent_runner.rs` und `subagents.rs` bleiben
strukturell. Jeder `state.push(...)` erbt automatisch den richtigen
`parent_call_id` aus dem Stack — keine Codepfad-Änderung dort. Falls
ein Subagent intern wieder Tools dispatched: der dortige
`tool_dispatch` macht ebenfalls Stack-Push/Pop, sodass tief
verschachtelte Tool-Bäume entstehen.

### 4. Frontend-Datenmodell: Tree-shaped `TimelineDoc`

`src/workbench/agent_timeline.rs` wird komplett neu strukturiert.
Statt einer flachen `Vec<TimelineItem>` ein **Baum** auf zwei Achsen:

- **Top-Level**: `TimelineDoc { version: u32, turns: Vec<TurnNode> }`.
- **TurnNode**: ein Konversations-Zyklus
  (`User`-Prompt + folgende Agent-Aktivität).
- **TurnPart**: typisierte Bausteine innerhalb eines Turns.

```rust
pub struct TimelineDoc {
    pub version: u32,            // Schema-Version, aktuell 2
    pub turns: Vec<TurnNode>,
}

pub struct TurnNode {
    pub id: String,              // "turn-<turn_generation>"
    pub user: UserPart,
    pub parts: RwSignal<Vec<TurnPart>>,
}

pub struct UserPart {
    pub id: String,              // "user-<seq>"
    pub text: String,            // immutable; nie streaming
}

#[derive(Clone)]
pub enum TurnPart {
    Thinking {
        id: String,              // "think-<seq>"
        text: RwSignal<String>,
        done: RwSignal<bool>,
    },
    Text {
        id: String,              // "text-<seq>"
        text: RwSignal<String>,
    },
    Tool {
        id: String,              // = call_id
        tool: String,
        args: Option<Value>,
        state: RwSignal<ToolState>,        // Pending|Running|Success|Error
        result: RwSignal<Option<String>>,
        metrics: RwSignal<ToolMetrics>,
        children: RwSignal<Vec<TurnPart>>, // verschachtelte Subagent-Parts
    },
    Subagent {
        id: String,              // = agent_id
        role: String,
        display_name: String,
        status: RwSignal<SubagentStatus>,  // Running|Done|Error
        parts: RwSignal<Vec<TurnPart>>,    // rekursiv — Subagent hat eigene Parts
        metrics: RwSignal<TurnMetrics>,
        summary: RwSignal<Option<String>>,
    },
    ModelRound {
        id: String,              // "round-<seq>"
        metrics: RwSignal<TurnMetrics>,
    },
    GeneratedImage { id: String, /* … */ },
    AskUser { id: String /* = call_id */, /* … */ },
}

pub enum ToolState { Pending, Running, Success, Error }
pub enum SubagentStatus { Running, Done, Error }
```

Jedes mutable Feld ist ein eigenes `RwSignal`. Render-Closures
subscribiren mit `move || sig.get()` — Streaming-Deltas updaten genau
das Signal, Leptos rendert im laufenden DOM. **Damit ist Bug 1
(Reaktivität) strukturell gelöst.**

### 5. Event-Reducer mit ID-Lookup

Neuer Reducer in `src/workbench/agent_panel/reducer.rs` (neu) ersetzt
`apply_agent_event`. Statt `rows.last_mut()` Pattern arbeitet er mit
ID-basiertem O(1)-Lookup. Routing:

- `parent_call_id == None` → Part landet im aktuellen Turn auf
  Top-Level (`turn.parts`).
- `parent_call_id == Some(cid)` → Part landet in
  - `Tool { id == cid }.children` (für Tool-interne Aktivität wie
    Subagent-Lauf), oder
  - `Subagent { id == cid }.parts`, falls cid einem
    Subagent-Identifier entspricht (nested case).

Reducer-Logik pro Event-Typ:

| Event | Verhalten |
|---|---|
| `ToolCall { call_id, … }` | Neue `Tool`-Part anhängen, `state = Pending` |
| `ToolResult { call_id, ok, message }` | Lookup Part by id; `state = Success/Error`, `result = Some(message)` |
| `TurnUsage { kind: ToolExec, call_id }` | Metrics in die zugehörige `Tool`-Part schreiben |
| `TurnUsage { kind: ModelRound, round_index }` | Neue `ModelRound`-Part im aktuellen Container |
| `SubagentStarted { agent_id }` | Neue `Subagent`-Part als Child des Tools mit `id = parent_call_id` |
| `SubagentStep` | Lookup Subagent, Status/Note update |
| `SubagentToolCall { agent_id, call_id, … }` | Neue `Tool`-Part in `subagent.parts` |
| `SubagentAssistantDelta { agent_id, delta }` | Lookup Subagent, append in dessen letzte/neue `Text`-Part |
| `SubagentThinkingDelta` / `SubagentThinkingDone` | Analog für `Thinking`-Part |
| `SubagentFinished { agent_id, status, summary }` | Subagent-Part: `status = Done/Error`, `summary = Some(...)` |
| `AssistantDelta { delta }` | Top-Level: Append in letzte `Text`-Part oder neue mit `id = "text-{seq}"` |
| `ThinkingDelta` / `ThinkingDone` | Analog für `Thinking`-Part |
| `Done` | Turn als abgeschlossen markieren; persistieren |

Stable IDs:
- `Tool.id = ToolCall.call_id` (provider-issued, immutable)
- `Subagent.id = SubagentStarted.agent_id`
- `Thinking.id = format!("think-{seq}")` — `seq` der **ersten**
  `ThinkingDelta` dieser Part; nachfolgende Deltas appenden in
  dieselbe Part bis `ThinkingDone`.
- `Text.id = format!("text-{seq}")` — analog.
- `ModelRound.id = format!("round-{seq}")`.

`seq` kommt aus Envelope → Reload-stabil, kollisionsfrei.

### 6. Subagent-Debounce löschen

`src/workbench/agent_panel/subagent_debounce.rs` und der Caller in
`src/workbench/agent_panel/mod.rs:683–685` werden entfernt. Mit
deterministischer `seq`-Reihenfolge ist Sortierung trivial, der
Reducer verarbeitet jeden Envelope synchron in
Backend-Push-Reihenfolge. Mutex-FIFO + monotone `seq` garantieren
korrekte Ordnung — Race-Conditions strukturell ausgeschlossen.

### 7. Stable `<For key>` everywhere

`src/workbench/agent_panel/mod.rs:442` und alle nested `<For>`-Loops
verwenden die stabilen IDs:

```rust
<For
    each=move || doc.turns.get()
    key=|t| t.id.clone()
    children=move |turn| view! { <TurnNodeView turn=turn /> }
/>

// in TurnNodeView:
<For
    each=move || turn.parts.get()
    key=|p| p.id().to_string()
    children=move |part| view! { <TurnPartView part=part /> }
/>
```

Kein Array-Index mehr — beim Reload sind alle IDs identisch zu
denen aus dem Live-Stream, kein DOM-Reuse mit stale Content.

### 8. Rekursive Render-Komponente

Eine einzige `TurnPartView`-Komponente, die per Match auf `TurnPart`
rendert und für `Tool.children` / `Subagent.parts` **rekursiv** sich
selber aufruft (visuell eingerückt + linker Indikator-Strich). Memo
pro Part vermeidet teures Re-Render.

`Tool`-Render zeigt state-basiert:
- `Pending` → Spinner + Tool-Name + Args-Preview
- `Running` → Spinner + abgekürzte Args
- `Success` → ✓ + truncated Result-Preview, expandierbar
- `Error` → ⚠ + Message

`Subagent`-Render zeigt:
- Header: `display_name (role)` + Status-Badge
- Body: rekursive `TurnPartView` über `subagent.parts.get()`
- Footer: Metrics + optionale Summary

`Thinking`-Render: Spinner während `done == false`, Body als `<pre>`
über das `text`-Signal subscribed (reaktiv!).

`Text`-Render: `inner_html` über ein Memo gebunden:

```rust
let html = Memo::new(move |_| render_markdown_to_html(&text.get()));
view! { <div inner_html=move || html.get()></div> }
```

Damit wird bei jedem Delta neu gerendert — aber Leptos diffed nur das
betroffene Element.

### 9. Migration alter Sessions verlustfrei

`sessions.json` enthält `Vec<TimelineItem>` im alten Schema. Neuer
Migrator in `src/workbench/agent_timeline_migration.rs` (neu):

- Lese alte Sequenz. Detektion: kein `schema_version` Feld → Version 1
  → migrieren.
- Walk linear:
  - `TimelineItem::User { text }` → öffnet neuen `TurnNode`.
  - Folgende `Assistant`/`Thinking`/`Tool`/`ModelDecision` Items
    werden zu Parts im aktuellen Turn.
  - **Spezialfall**: `Tool(subagents.run)` gefolgt von
    `SubagentGroup(g)` (direkt benachbart) → werden zu einem
    `Tool { children: [Subagent {…}, …] }` zusammengeführt. Die
    Subagent-Card-Felder (`live_text`, `live_thinking`, `tools`) werden
    in `Subagent.parts` flachgelegt (Tools werden zu `Tool`-Parts).
  - `SubagentGroup` ohne vorangehenden `Tool(subagents.run)` →
    fallback als top-level `Subagent`-Part (defensive).
- Texte/Done sind bereits final → Signals werden mit dem statischen
  Endwert befüllt.
- Beim ersten `workbench_load_state` Aufruf in
  `src/workbench/state.rs` Migration triggern, `schema_version = 2`
  setzen, dann `workbench_save_state` zur Persistierung.

Alte Daten werden **nicht** vor erfolgreicher Migration gelöscht —
falls Migration scheitert, kann der Code unverändert geladen werden
(Fallback-Lesung beider Schemas via `untagged`-Enum).

### 10. Persistenz-Schema

`src/workbench/state.rs:69` `agent_timeline: Vec<TimelineItem>` wird
zu `agent_timeline: TimelineDoc`. Serialisierung flattet Signals zu
Werten beim Speichern, beim Laden werden Signals neu erzeugt.

Persistenz-Format (vereinfacht):

```json
{
  "version": 2,
  "turns": [
    {
      "id": "turn-0",
      "user": { "id": "user-0", "text": "..." },
      "parts": [
        { "kind": "text", "id": "text-3", "text": "Ja. In diesem ..." },
        { "kind": "tool", "id": "call_abc", "tool": "subagents.run",
          "state": "success",
          "result": "Subagent finished",
          "children": [
            { "kind": "subagent", "id": "sa-1", "role": "scout",
              "displayName": "Scout 1", "status": "done",
              "parts": [
                { "kind": "thinking", "id": "think-9", "text": "...", "done": true },
                { "kind": "tool", "id": "call_def", "tool": "read_workspace_file",
                  "state": "success", "result": "...", "children": [] }
              ]
            }
          ]
        }
      ]
    }
  ]
}
```

### 11. Was sich **nicht** ändert

- Backend-Persistenz `conversation: Vec<Value>`
  (`src-tauri/src/agent/state.rs:26`) bleibt — das ist die
  LLM-Provider-API-History, nicht die UI-Timeline.
- `TurnUsage`-Event-Felder strukturell unverändert; werden jetzt nur
  über `parent_call_id` korrekt zugeordnet (None = Main-Agent,
  Some(cid) = innerhalb Subagent).
- Bestehende `subagents`-Tool-Implementierung
  (`src-tauri/src/agent/subagent_runner.rs`) und ihre Streaming-Pfade
  — nur Push-Order und Envelope-Wrapping ändern sich.
- API zwischen Backend (`agent_poll_events`) und Frontend
  (`tauri_bridge::agent_drain_turn_opts`) — nur der Element-Typ
  wandert von `AgentEvent` zu `EventEnvelope`. Drain-Loop selbst
  bleibt.

## Zu ändernde / neue Dateien

### Backend

| Datei | Änderung |
|---|---|
| `src-tauri/src/agent/protocol.rs` | `EventEnvelope { seq, parent_call_id, event }`; ggf. `#[serde(untagged)]` Variante für Legacy-Lesung |
| `src-tauri/src/agent/state.rs` | `AtomicU64 next_seq` + `Mutex<Vec<String>> parent_stack`; `push()` wickelt automatisch; `push_parent/pop_parent`; `start_turn()` resettet |
| `src-tauri/src/agent/tool_dispatch.rs` | ToolCall-Push vor Sonderfall-Verzweigung; parent-Stack push/pop um jeden Dispatch; subagents.run-Spezialfall entfällt strukturell |
| `src-tauri/src/agent/subagent_runner.rs` | unverändert (Events erben parent_call_id automatisch); `subagents.rs` analog |
| `src-tauri/src/agent/anthropic.rs` + `openrouter.rs` | `start_turn()` am Anfang von `run_chat_turn` aufrufen, sodass `seq` per Turn von 0 startet |
| `src-tauri/src/agent/session_orchestrator.rs` | Keine Änderung — `dispatch_user_turn` arbeitet weiter mit `AgentEvent`-Push (Envelope wird in `state.push` gewrappt) |

### Frontend (Bridge)

| Datei | Änderung |
|---|---|
| `src/agent_wire.rs` | `EventEnvelope` spiegeln; `AgentEvent` bleibt; serde-defaults für `seq`/`parentCallId` falls altes Backend (nicht relevant da gleichzeitig deployed, aber defensiv) |
| `src/tauri_bridge.rs` | `agent_poll_events` Rückgabetyp `Vec<EventEnvelope>`; `agent_drain_turn_opts` Callback bekommt `Vec<EventEnvelope>` |

### Frontend (Daten + Reducer + Render)

| Datei | Änderung |
|---|---|
| `src/workbench/agent_timeline.rs` | **Komplett neu**: `TimelineDoc`, `TurnNode`, `TurnPart`, `ToolState`, `SubagentStatus`, Helper-IDs. Signals statt by-value Felder. |
| `src/workbench/agent_panel/reducer.rs` (neu) | `apply_envelope(doc, env)` Tree-Reducer mit ID-Lookup; HashMap-Index für O(1) Lookup zwischen Top-Level und Subagent-Parts |
| `src/workbench/agent_panel/timeline.rs` | Alte `TimelineRow` weg, ersetzt durch `TurnNodeView` + `TurnPartView` (rekursiv, Signal-subscribed). Helpers wie `render_markdown_to_html`, `TurnMetricsBar`, `ToolActivityRow`, `ThinkingRow` werden refactored — Texte über Signal/Memo statt by-value. `compact_timeline` und `merge_consecutive_model_rounds` entfallen (Tree macht ModelRounds direkt korrekt). |
| `src/workbench/agent_panel/mod.rs` | Stable `<For key=turn.id>` außen + `<For key=part.id>` innen; Drain-Konsument ruft `apply_envelope` statt `apply_agent_event`; Debounce-Aufrufe entfernt |
| `src/workbench/agent_panel/subagent_debounce.rs` | **Löschen** (Datei + `mod` in `mod.rs`) |
| `src/workbench/agent_timeline_migration.rs` (neu) | One-shot Migrator alter Vec → neuer Tree, Version-Bump auf 2; defensive Fallback wenn Migration fehlschlägt |
| `src/workbench/state.rs` | `agent_timeline: TimelineDoc` mit `version` Feld; `workbench_load_state` triggert Migrator wenn version<2; `set_workspace_agent_timeline` arbeitet auf neuem Typ |
| `src/workbench/agent_context_handoff.rs` | Falls Timeline-Inhalte gelesen werden (für "send to chat" Kontext) — Reader anpassen auf neuen Tree |
| `styles.css` | Neue Klassen: `.timeline-tree`, `.timeline-tree__children` (Einrückung + linker Indikator-Strich), `.tool-state-badge--{pending,running,success,error}`, `.subagent-card`; alte `agent-chat-line--*` Klassen aufräumen |

Regel `rule-no-monolith-structure.md` eingehalten: Reducer in eigene
Datei, Migration in eigene Datei, Rendering in fokussierte
Komponenten — keine 1000-Zeilen-Datei.

## Migration / Kompatibilität

- `EventEnvelope` ist beim Frontend-Empfang verpflichtend nach
  Deployment; Bridge-Wrapper macht Fallback (`seq = 0,
  parent_call_id = None`) wenn ein altes Backend gepollt wird (für
  CI/Dev mit gemischten Versionen).
- `sessions.json` wird beim ersten Laden migriert (Schema-Version
  Bump auf 2). Backup nicht nötig — Migrator ist verlustfrei und
  idempotent; bei Fehler bleibt alter Wert, Logs warnen.
- Live-Tests: parallel zwei Sessions öffnen (alte sessions.json laden,
  neuen Turn fahren) — beide rendern korrekt im neuen Schema.

## Verifikation

### Backend Unit-Tests (`src-tauri/src/agent/`)

1. `tool_dispatch_pushes_toolcall_for_subagents_run`: nach
   `dispatch_tool(name="subagents.run", call_id="cid-1", …)` enthält
   die Event-Queue als allererstes Element ein
   `ToolCall { tool: "subagents.run", call_id: Some("cid-1") }` und am
   Ende ein `ToolResult` mit demselben call_id.

2. `subagent_events_inherit_parent_call_id`: simulierter Subagent-Run
   pusht `SubagentStarted`, `SubagentToolCall`, `SubagentFinished`.
   Alle drei Envelopes haben `parent_call_id == Some("cid-1")` —
   identisch zum `ToolCall.call_id` des umschließenden
   `subagents.run`.

3. `nested_subagent_pushes_two_parent_levels`: ein Subagent ruft
   intern `subagents.run` auf. Innere Subagent-Events haben
   `parent_call_id == Some("cid-inner")`, der äußere `ToolCall`
   `parent_call_id == Some("cid-outer")`.

4. `seq_monotonic_per_turn`: 100 zufällige Push-Events sind streng
   monoton in `seq` (`seq[i+1] == seq[i] + 1`). Neuer Turn (via
   `start_turn`) startet wieder bei 0.

### Frontend Unit-Tests (`src/workbench/`)

5. `reducer_nests_subagent_under_tool`: Sequenz
   `ToolCall(run, cid_run) → SubagentStarted(sa1, parent=cid_run) →
   SubagentToolCall(sa1, parent=cid_run, cid=cid_t1) →
   SubagentFinished(sa1, parent=cid_run) → ToolResult(cid_run)`
   produziert genau **eine** Top-Level Tool-Part mit
   `id = cid_run` und Status `Success`; deren `children` enthält **eine**
   Subagent-Part mit `id = sa1`, Status `Done`; deren `parts`
   enthält **eine** Tool-Part `id = cid_t1`. Keine Geschwister-Rows.

6. `reducer_streams_into_existing_text_part`: mehrere
   `AssistantDelta`-Envelopes ohne dazwischen liegende Tool-Events
   landen alle im selben `Text`-Part (id stabil über Stream);
   Endinhalt = Konkatenation.

7. `reducer_new_text_part_after_tool`: Stream
   `AssistantDelta("a") → ToolCall → ToolResult → AssistantDelta("b")`
   produziert **zwei** Text-Parts (vor + nach Tool) mit
   unterschiedlichen IDs.

8. `migration_old_subagent_layout_to_tree`: fixture mit alten Items
   `[User, Tool(subagents.run), SubagentGroup{agents:[card1,card2]},
   Assistant]` migriert zu Turn mit Parts
   `[Tool{id, children:[Subagent(card1), Subagent(card2)]}, Text]`.
   Alle Texte/Done-Werte preserved.

9. `migration_orphan_subagent_group`: fixture
   `[User, SubagentGroup{...}]` (ohne vorangehenden `Tool(run)`)
   migriert zu Turn mit Top-Level Subagent-Part (defensiv).

10. `for_key_stable_across_streaming`: Snapshot eines `TurnNode`
    während mid-stream (3 Deltas) und nach `Done` hat identische
    `id`-Strings auf allen Parts → Leptos `<For>` kein Re-Mount.

### Manuell (`cargo tauri dev`)

11. **Reaktivität live**: User-Prompt "erkläre mir alles über
    Rust-Lifetimes in 3 Absätzen". DOM wird **live** beim Streaming
    aktualisiert (Wörter erscheinen progressiv). Reload nicht
    nötig. Spinner verschwindet sobald `ThinkingDone` ankommt.

12. **Reihenfolge**: User-Prompt der `subagents.run` triggert
    ("nutze subagents.run mit role=scout für Datei XYZ"). Im UI
    erscheint zuerst `Run subagents`-Tool-Row, **darunter eingerückt**
    die Subagent-Aktivität (Thinking, Text, Tools). Reihenfolge ist
    stabil, kein "Subagent vor Tool"-Effekt.

13. **Doppelte Verschachtelung**: Subagent, der wiederum
    `subagents.run` aufruft. UI rendert **zwei** Einrückungsebenen
    klar erkennbar.

14. **Persistenz**: Während Stream App schließen, neu öffnen — Chat
    erscheint identisch zum Zeitpunkt vor Schließen (sofern Turn
    fertig war).

15. **Migration**: Vor Upgrade einen Chat mit Subagent-Lauf führen
    und persistieren. Nach Upgrade neu starten — alte Session muss als
    sauber verschachtelter Baum dargestellt werden, kein Datenverlust.

### Smoke

16. `cargo test --workspace` — alle bestehenden Tests grün.
17. `cargo check -p blxcode --target wasm32-unknown-unknown` —
    Frontend kompiliert.
18. Browser-Devtools beim Streaming: DOM-Mutations sollten gezielt am
    betroffenen Element stattfinden (Profiler), nicht Vollumbau der
    Row.

## Risiken & Mitigationen

- **Risiko**: Performance bei großen Bäumen — `RwSignal` pro Part
  kostet Allocation.
  *Mitigation*: Subscriben nur dort wo wirklich gerendert; tiefe
  Bäume cap-en (z. B. `MAX_NESTING_DEPTH = 5` mit "+N more"-Indikator
  drüber).

- **Risiko**: Migration scheitert bei exotischen Altdaten.
  *Mitigation*: Try-Block um Migrator; bei Fehler alten Wert
  beibehalten und Schema-Version auf 1 lassen (Datei kann später
  manuell repariert oder erneut versucht werden). Migrator-Test
  deckt die häufigen Schemas ab.

- **Risiko**: `EventEnvelope`-Wechsel bricht laufende Sessions
  während Hot-Reload während Dev.
  *Mitigation*: serde-defaults für `seq`/`parentCallId`; bei
  Type-Mismatch Drain-Loop loggen, nicht crashen.

- **Risiko**: `Subagent.id == agent_id` kollidiert wenn dieselbe
  Agent-ID in zwei Turns auftaucht.
  *Mitigation*: ID innerhalb Turn-Scope unique; tatsächlicher
  `<For key>` ist `format!("{turn_id}:{agent_id}")` oder einfach
  `format!("{seq}:{agent_id}")` mit `seq` der ersten
  `SubagentStarted`.

## Out-of-Scope

- LLM-Provider-API-History (`conversation: Vec<Value>`) wird **nicht**
  refactored — sie bleibt flach, weil Provider sie so erwarten.
- Plan-Tasks und Memory-Notes-Rendering — separat. Dieser Refactor
  betrifft nur den AgentTab.
- Multi-Cursor-Editor für Assistant-Text — nice-to-have, separater
  Plan.

## Tasks

- [x] `backend-event-envelope` - Wrap backend agent events in seq/parent envelopes and expose them through `agent_poll_events`
- [x] `backend-subagents-toolcall` - Emit `ToolCall` before `subagents.run` dispatch and propagate parent call IDs while the tool runs
- [x] `frontend-envelope-drain` - Mirror `EventEnvelope` in the frontend bridge and drain envelope batches
- [x] `remove-subagent-debounce` - Remove delayed subagent event buffering from the agent panel
- [x] `streaming-render-refresh` - Ensure timeline rendering reflects streaming updates instead of freezing keyed row props
- [x] `timeline-tree-doc` - Replace flat `Vec<TimelineItem>` persistence with tree-shaped `TimelineDoc`
- [x] `tree-reducer` - Add ID-based reducer that nests subagents and tool children by `parent_call_id`
- [x] `recursive-renderer` - Replace row renderer with recursive turn/part components and stable part IDs
- [x] `session-migration` - Add lossless migration for old session timelines
- [x] `frontend-tree-tests` - Cover reducer streaming, nesting, and migration behavior

## Quellen (Best-Practice-Recherche)

- [The 'tool-call' Render Pattern (Hackernoon)](https://hackernoon.com/the-tool-call-render-pattern-turning-your-ai-from-a-chatty-bot-into-a-doer)
  — State-basiertes Rendering von Tool-Calls (`call → result`),
  `call_id` als Korrelationsfeld.
- [Tool Calling Explained (Composio, 2026)](https://composio.dev/content/ai-agent-tool-calling-guide)
  — `trace_id` über Agent-Boundaries, `span_id` für
  Streaming-Token-Korrelation, Sequenznummern für asynchrone
  Reihenfolge.
- [Agent UX: designing UI for AI agents 2026 (Fuselab)](https://fuselabcreative.com/ui-design-for-ai-agents/)
  — Plan-Visibility, Real-time Progress-Tracking, Intervention-Points.
- [AI Chat UI Best Practices 2026 (thefrontkit)](https://thefrontkit.com/blogs/ai-chat-ui-best-practices)
  — Inline Tool-Visualization, automatische Intermediate-Step-Cards.
- [LangGraph + assistant-ui](https://www.langchain.com/blog/assistant-ui)
  — Streaming-first Tool-Call-Rendering, Approval-Flows.
- AG-UI (Open-Protocol für Agent ↔ Frontend Event-Streams) —
  Referenz-Format für `seq` + typisierte Events.
- Vercel AI SDK `UIMessage.parts: Array<TextPart | ToolPart |
  ReasoningPart>` — direkter Vorbild für `TurnPart`-Enum.
