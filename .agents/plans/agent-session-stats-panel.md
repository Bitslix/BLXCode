# Agent-Session-Stats in der linken Hero-Grid-Box

> Status: **done** (umgesetzt & verifiziert. Datenquellen im Code bestätigt —
> kein Backend nötig; nur ein bestehender Backend-Test-Initializer wurde für
> `orb_mode` nachgezogen.)

## Summary

Der Agent-Header (`.agent-hero`) ist ein 2-Spalten-Grid. **Layout getauscht**
(umgesetzt in `styles.css`): **rechts** der Drobo-Orb (Spalte 2, Zeile 1) mit dem
**State** („Standby/Running", Zeile 2) darunter, **links** die `1fr`-Spalte
(Spalte 1) für die Statistik-Karte. Diese **linke** Box soll eine **moderne,
clean gestaltete, live-aktualisierte Chat-Session-Statistik-Karte** werden — mit
**Icons** je Zeile und einem **einheitlichen Tooltip** (1:1 der
Sidebar-Voice-Orb-Tooltip-Style, siehe unten) auf jeder Zeile.

Anzuzeigende Werte:

1. **Model / Provider** — oben als Header-Chip (z. B. `anthropic/claude-…`)
2. **Session Start Time** — Startzeitpunkt der aktuellen Session
3. **Session Context** — Fenster-Belegung (`9.6k / 400k · 2%`)
4. **User Turns** — Anzahl User-Eingaben
5. **Model Turns** — Anzahl Modell-Runden
6. **Tool Calls (x, y, z)** — Gesamt + Aufschlüsselung (open/read/edit/rm)
7. **Costs** — aufgelaufene USD-Kosten der Session

(Aktive Subagents bleiben optional als Zusatz-Sektion am Boden der Karte.)

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
  `turn_count`, `total_cost_usd` (**→ Costs**), `last_round_input_tokens`
  (= Live-Context-Belegung **→ Session Context**).
- `context_length: RwSignal<Option<u64>>` (Fenstergröße, Default ~400k) — liegt
  bereits in `AgentPanelDock` (`mod.rs:61`).
- **Model/Provider**: `model_label: RwSignal<String>` (`mod.rs:54`), gesetzt als
  `format!("{provider}/{model_id}")` aus `agent_settings_get` (`mod.rs:113`) —
  direkt wiederverwendbar.
- **User Turns** = `doc.turns.len()` (jeder `TurnNode` = eine User-Eingabe,
  `agent_timeline.rs:344`).
- **Model Turns** = Anzahl `TurnPart::ModelRound` im Hauptagenten (Walk über
  `doc.turns[*].parts`, Subagent-Runden ausgenommen).
- `busy: RwSignal<bool>` für Standby/Running.
- Vorbild für Stats-Lesen: `SessionCostChip` (`agent_panel/mod.rs:843`) nutzt
  `chat_usage_for_workspace` + `Memo` + i18n exakt so.
- Tool-Namen-Inventar: siehe `tool_icon()` (`timeline.rs:893`) und
  `file_arg_path()` (`agent_timeline.rs:787`).

### Datenlücke: Session Start Time (muss ergänzt werden)

Es existiert **kein** Session-Start-Zeitstempel — weder `ChatUsageStats` noch
`TurnNode`/`UserPart` tragen einen. Lösung: neues Feld
`session_started_at: Option<f64>` (Epoch-ms via `js_sys::Date::now()`) in
`ChatUsageStats` (`state.rs:198`), gesetzt in `record_chat_turn_usage` beim
Übergang `turn_count 0 → 1`, zurückgesetzt in `agent_clear_conversation`.
`#[serde(default)]` → abwärtskompatibel; **rein im Frontend-Crate** (`src/`),
kein `src-tauri`. Anzeige als lokale Uhrzeit (z. B. `14:32`) bzw. „—" solange
keine Session läuft.

## Einheitlicher Tooltip (TIP)

Vorbild ist der Sidebar-Voice-Orb-Tooltip `.sidebar-ptt-tooltip`
(`styles.css:1237–1321`, Markup `sidebar.rs:1007–1013`): rein CSS-basiertes
Hover/Focus-Popover mit
- `__eyebrow` (Uppercase, Muted, mit Akzent-Punkt `__spark`)
- `__main` (fetter Titel)
- `__hint` (gedämpfter Zusatztext)
- Pfeil via `::after` (45°-Rotation), Akzent-Border, Panel-Gradient,
  `box-shadow`, Einblende-Transition (`opacity`+`transform`).

Bisher ist er an `.sidebar-ptt-orb:hover + .sidebar-ptt-tooltip` gekoppelt
(Sibling-Selektor, fest in der Sidebar). Ziel: **generalisieren** in ein
wiederverwendbares Set `.blx-tooltip` (+ `__eyebrow/__spark/__main/__hint`) mit
Anchor-Wrapper `.blx-tip-anchor { position: relative }`, das per
`:hover`/`:focus-within` einblendet — unabhängig von der DOM-Reihenfolge. Dazu
eine Leptos-Hilfskomponente:

```rust
#[component]
pub fn InfoTip(
    eyebrow: String, main: String,
    #[prop(optional)] hint: Option<String>,
    children: Children,            // der Trigger (Icon/Wert)
) -> impl IntoView
```

→ rendert `<span class="blx-tip-anchor">{trigger}<span class="blx-tooltip"
role="tooltip">…</span></span>`. Sidebar wird auf die generischen Klassen
umgestellt (alte `.sidebar-ptt-tooltip*`-Regeln dünnen sich auf reine
Positions-/Trigger-Overrides aus, Optik bleibt 1:1).

### App-global verbindlich

Dieser Tooltip-**Color/Style/Theme** ist ab sofort der **app-weite Standard**:

- Farben/Border/Gradient/Shadow ausschließlich über die **Theme-Tokens**
  (`--accent`, `--border-strong`, `--bg-panel`, `--bg-raised`, `--text`,
  `--scrim-bg`, `--radius-lg`) → passt sich **automatisch jedem Theme** an
  (kein Hardcode).
- `.blx-tooltip` lebt zentral (nicht panel-lokal) und ist von überall nutzbar.
- **Migrationsrichtung**: native `title=`-Tooltips (z. B. `turn_metrics_bar`,
  Chat-Action-Buttons, Tool-Rows) werden schrittweise auf `InfoTip`/`.blx-tooltip`
  umgestellt. Neue UI nutzt **ausschließlich** den globalen Tooltip; native
  `title=` nur noch als Fallback für reine A11y-Kurzlabels.
- Optional als eigene Tooltip-Richtungen (`--top/--bottom/--left/--right`) für
  spätere Wiederverwendung (Pfeil-`::after` je Richtung).

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

## Layout (modern / clean)

Karte in der **linken** Grid-Zelle (Spalte 1), Drobo-Orb rechts daneben
(Spalte 2). Von oben nach unten:

```
┌─────────────────────────────────────┐
│  ◆ anthropic / claude-…   (Header)   │  ← Provider/Model-Chip, klickbar→Settings
├─────────────────────────────────────┤
│  🕘  Started        14:32            │
│  ◔  Context        9.6k / 400k · 2% │  (+ schmaler Meter-Balken)
│  👤  User turns     3                │
│  🤖  Model turns    7                │
│  🔧  Tool calls     12  (4·6·1·1)    │  open·read·edit·rm
│  $   Costs          $0.019           │
├─────────────────────────────────────┤
│  ⚙ Subagents (optional, bei aktiv)   │
└─────────────────────────────────────┘
```

- Jede Zeile = Icon + Label + Wert; **gesamte Zeile** ist `InfoTip`-Anchor →
  einheitlicher Tooltip (eyebrow = Label, main = Wert/Kurzform, hint = Erklärung).
- Icons (lucide / `icondata::Lu*`): Started→`LuClock`, Context→`LuGauge`,
  User turns→`LuUser`, Model turns→`LuBot`, Tool calls→`LuWrench`,
  Costs→`LuCircleDollarSign`, Header→`LuCpu`/`LuBrain`. (Namen bei Umsetzung
  gegen die vorhandene `icondata`-Version prüfen.)
- Tool-Calls-Aufschlüsselung `(open·read·edit·rm)` als dezente Mini-Badges.
- Context mit schmalem Belegungs-Balken (Reuse `context_meter`-Logik/Klassen).

## Tasks / Phasen

**TIP-01 — Einheitlicher Tooltip (Vorbedingung, app-global)**
- `.sidebar-ptt-tooltip`-Optik in **zentrale, app-globale** Klassen
  `.blx-tooltip` (`__eyebrow/__spark/__main/__hint`) + `.blx-tip-anchor`
  extrahieren (`:hover`/`:focus-within`-getriggert, nur Theme-Tokens → folgt
  jedem Theme automatisch).
- Leptos-Hilfskomponente `InfoTip` (s. o.), z. B. `src/workbench/harness_ui.rs`
  oder neue `src/workbench/info_tip.rs`.
- Sidebar-Voice-Orb auf `InfoTip`/`.blx-tooltip` umstellen — Optik 1:1 erhalten.
- Damit ist der globale Tooltip-Standard etabliert; bestehende `title=`-Tooltips
  wandern schrittweise nach (s. „App-global verbindlich").

**STATS-01 — Aggregator (testbar, isoliert)**
Neue Datei `src/workbench/agent_panel/session_stats.rs`:
- `struct SessionStats { user_turns, model_turns, tool_total, open, read, edit, rm: u32, active_subagents: Vec<(String, String)> }`
- `fn compute_session_stats(doc: &TimelineDoc) -> SessionStats`: `user_turns =
  doc.turns.len()`; rekursiver Part-Walk (Tool-`children` + Subagent-`parts`)
  für Model-Turns (`ModelRound`-Zähler Hauptagent), `tool_category`-Buckets und
  laufende Subagents (`SubagentStatus::Running`).
- Unit-Tests analog `reducer.rs`/`timeline.rs`.

**STATS-02 — Session Start Time (State)**
- Feld `session_started_at: Option<f64>` in `ChatUsageStats` (`state.rs`),
  `#[serde(default)]`. Setzen in `record_chat_turn_usage` bei `0 → 1`,
  Reset in `agent_clear_conversation`. Getter `chat_usage_for_workspace`
  liefert es mit. Format-Helfer (Epoch-ms → lokale `HH:MM`).

**STATS-03 — Komponente `AgentSessionStats`**
- Props: `timeline`, `wb`, `context_length`, `model_label`, `busy`.
- `Memo` auf `timeline` → `SessionStats`; Costs/Context/Start aus
  `chat_usage_for_workspace`; Provider/Model aus `model_label`.
- Rendert die Karte (s. Layout) mit `InfoTip` je Zeile in die **linke**
  Grid-Zelle.

**STATS-04 — Einbau in den Header**
- In `agent_panel/mod.rs` die **linke** Spalte (Grid-Spalte 1) mit
  `<AgentSessionStats .. />` füllen. Orb/State stehen rechts (Spalte 2) — der
  Grid-Swap ist in `styles.css` bereits umgesetzt.

**STATS-05 — CSS**
- `.agent-session-stats` in `styles.css`: clean Card (Border, `--radius-lg`,
  dezenter Panel-Gradient/Shadow im Tooltip-Stil), Icon-Label-Wert-Zeilen,
  Mini-Badges, Context-Meter, optionale Subagent-Sektion mit Puls.
- Compact-Mode (`.agent-hero--compact`) blendet die Karte aus (nur State bleibt).

**STATS-06 — i18n**
- Neue `I18nKey`s: `AgStatsModel`, `AgStatsStarted`, `AgStatsContext`,
  `AgStatsUserTurns`, `AgStatsModelTurns`, `AgStatsToolCalls`, `AgStatsOpen`,
  `AgStatsRead`, `AgStatsEdit`, `AgStatsRm`, `AgStatsCosts`, `AgStatsSubagents`
  + zugehörige `*Tip`-Hint-Keys.
- Pflicht in **allen** `i18n/locales/*.rs`. EN setzen, Rest via
  `scripts/render_i18n_locales_from_en.py`.

**STATS-07 — Verifikation**
- `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
- `cargo test --workspace` (Aggregator-Tests)
- Live-Test: Werte reaktiv (auch während „Thinking"), Tooltips erscheinen
  einheitlich, Session-Start nach erstem Turn gesetzt + nach Clear zurück,
  Compact-Mode blendet aus.

Verifiziert:
- `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
- `cargo test -p blxcode-ui session_stats --target x86_64-unknown-linux-gnu`
- `cargo test --workspace`

Hinweis: `scripts/tools/render_i18n_locales_from_en.py` konnte nicht laufen,
weil `deep_translator` in der Umgebung fehlt. Nicht-englische Locales enthalten
deshalb vorerst englische Fallbacks für die neuen Stats-Keys.

## Designentscheidungen (festgelegt)

Vom Nutzer bestätigt — gelten als verbindlich:

- **Session Start = Zeitpunkt des ersten Turns** der Session (gesetzt beim
  Übergang `turn_count 0 → 1`, Reset bei `agent_clear_conversation`).
- **Anzeigeformat = lokale Uhrzeit `HH:MM`** (kein relatives „vor 12 min").
- **Subagents-Sektion = in dieselbe Karte** (unten, nur sichtbar wenn aktive
  Subagents vorhanden sind).

## Risiken / Notizen

- Reiner **Frontend**-Change (inkl. `ChatUsageStats`-Feld in `src/`), keine
  `src-tauri`-/IPC-/Protokoll-Änderung → geringes Risiko.
- `TIP-01` berührt die Sidebar (Tooltip-Refactor) — Optik muss 1:1 erhalten
  bleiben; visuell gegen Vorher abgleichen.
- Tool-Klassifizierung ist Heuristik; bei neuen Tools `tool_category` pflegen.
- `compute_session_stats` läuft pro `timeline`-Update — bei sehr langen
  Sessions ggf. memoisieren (Memo deckt das ab).
- i18n-Keys deutlich gewachsen (~12 + Hint-Keys) — alle Locales füllen.
