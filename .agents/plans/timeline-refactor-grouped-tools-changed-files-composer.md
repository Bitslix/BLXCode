# Timeline-Refactor: Grouped Toolcalls · Changed-Files-Card · Moderner Composer

> Status: **done** — Phasen A, B, C umgesetzt & compile-verifiziert;
> Phase D: i18n + automatisierte Verifikation erledigt
> (`cargo check -p blxcode-ui --target wasm32-unknown-unknown`,
> `cargo test --workspace`, `scripts/lint_theme_tokens.sh`), manueller
> `cargo tauri dev`-UI-Durchlauf offen. Changelog-Eintrag unter `[Unreleased]`
> ergänzt. Umsetzung direkt auf `stage`, Commit pro Phase.
> Frontend-Schwerpunkt (`blxcode-ui`); Backend nur lesend (vorhandene
> Tauri-Commands), **keine** neuen `src-tauri`-Protokoll-Felder nötig.

## Summary

Die Agent-Chat-Timeline soll optisch & strukturell auf das Niveau moderner
Coding-Agents (Cursor/Claude-Style) gehoben werden. Drei Bausteine, exakt nach
den Referenzbildern, alles über **Theme-Tokens** + **i18n**:

1. **Grouped Toolcalls** — aufeinanderfolgende Tool-Calls eines Model-Rounds als
   saubere, schmale vertikale Liste von Status-Pills (Icon · Label · Arg ·
   Status), je Tool-Typ gruppiert mit `×N`-Count (Bild 1: „Searched files“,
   „Read file“, „Ran command …“).
2. **Changed-Files-Card** — am **Ende eines Model-Task-Turns**, sobald Dateien
   verändert wurden, eine zusammengefasste Karte
   `CHANGED FILES (N) · +adds / −dels` mit Verzeichnis-Baum, Per-Datei-Stats
   und `Collapse all` / `View diff` (Bilder 7–8).
3. **Moderner Composer** — neue Chat-`<textarea>` mit Auto-Grow, Footer-Leiste
   mit Model-Picker-Popover (Bild 2), Mode-/Access-Switch-Popover (Plan/Build,
   Supervised/Auto-accept/Full-access — Bilder 3–4), Thinking-Level und Send;
   Placeholder „Ask anything, @tag files/folders, …“.

## Referenzbilder → Feature-Mapping

- **Bild 1** (Status-Pills-Liste): Phase A — Grouped Toolcalls.
- **Bild 2** (Model-Suchpopover „Composer/Auto/Opus/…“ mit `Ctrl+n`): Phase C —
  Model-Picker.
- **Bild 3** (`Plan` Pill + `Full access` + Send-Orb): Phase C — Footer-Leiste.
- **Bild 4** (Popover Supervised/Auto-accept/Full-access): Phase C —
  Access-/Mode-Popover.
- **Bilder 7–8** (`CHANGED FILES (6) · +443/−505` Baum + Collapse all/View diff):
  Phase B — Changed-Files-Card.

## Ist-Zustand (verifiziert)

### Timeline / Tool-Rendering
- **Datenmodell:** [agent_timeline.rs](../../src/workbench/agent_timeline.rs)
  `ToolActivity { tool, label, args_summary, status, detail, call_id, metrics,
  paths: Vec<String>, merged_count }` — `paths` + `merged_count` existieren
  bereits (Grouping-Infrastruktur ist da).
- **Display-Aufbereitung:** [timeline.rs](../../src/workbench/agent_panel/timeline.rs)
  `DisplayTimelineItem::ModelRound { metrics, tools: Vec<ToolActivity> }`
  ([timeline.rs:46](../../src/workbench/agent_panel/timeline.rs#L46)); Merge-Logik
  für aufeinanderfolgende gleiche Tools ab
  [timeline.rs:708](../../src/workbench/agent_panel/timeline.rs#L708) und
  [timeline.rs:824](../../src/workbench/agent_panel/timeline.rs#L824).
- **Rendering:** `ModelRound`-Zweig ab
  [timeline.rs:1088](../../src/workbench/agent_panel/timeline.rs#L1088)
  (`.model-round-header`, `.model-round-tools`, `.agent-tool-row*`). `×N`-Badge
  schon vorhanden ([timeline.rs:1164](../../src/workbench/agent_panel/timeline.rs#L1164)).
- **Problem:** `timeline.rs` ist mit **2401 Zeilen** ein Monolith
  (verletzt [rule-no-monolith-structure.md](../rules/rule-no-monolith-structure.md));
  Rendering muss in Subkomponenten + eigene CSS-Ordner zerlegt werden
  ([rule-reusable-components.md](../rules/rule-reusable-components.md)).

### Composer
- **Aktuell:** [mod.rs:625-697](../../src/workbench/agent_panel/mod.rs#L625-L697):
  separate `.agent-mode-toolbar` (3 `ModeButton`) **über** dem `<form
  class="agent-compose">` mit einzeiligem `<input type="text">` +
  Send/Cancel-Button. `ModeButton` ab
  [mod.rs:702](../../src/workbench/agent_panel/mod.rs#L702).
- `chat_mode: RwSignal<AgentChatMode>` bereits verdrahtet (`AskEdits | AllowAll |
  Plan`, [agent_wire.rs:24](../../src/agent_wire.rs#L24)); per-Workspace
  persistiert (`set_workspace_agent_chat_mode`). `model_label` Signal existiert
  ([mod.rs:77](../../src/workbench/agent_panel/mod.rs#L77),
  Format `provider/model_id`).

### Daten für Model-Picker & Changed-Files (vorhanden — kein neues Backend)
- **Model-Liste:** `agent_provider_models(provider)` →
  `ProviderModelsResponse { entries: Vec<ProviderModelEntry { id, label,
  description, pricing, context_length } }`
  ([tauri_bridge.rs:698](../../src/tauri_bridge.rs#L698)).
- **Aktive Settings:** `agent_settings_get()` →
  `AgentProviderSettingsView { provider, model_id, thinking_level, … }`
  ([tauri_bridge.rs:533](../../src/tauri_bridge.rs#L533)); Speichern
  `agent_settings_save(...)` ([tauri_bridge.rs:555](../../src/tauri_bridge.rs#L555)).
- **Thinking-Level:** `ThinkingLevel { Off, Low, Medium, High, Max }`
  ([agent_settings.rs:82](../../src-tauri/src/agent_settings.rs#L82)).
- **Changed Files:** `git_status_changes(cwd, connection_id)` →
  `Vec<ChangedFile { rel_path, status, staged, unstaged, staged_stats,
  unstaged_stats: Option<LineStats { added, removed }> }>`
  ([tauri_bridge.rs:2822](../../src/tauri_bridge.rs#L2822)). Bereits genutzt von
  [file_diff_section/mod.rs](../../src/workbench/file_diff_section/mod.rs) (nur
  staged/unstaged-Gruppierung, **kein** Verzeichnis-Baum → für die Card neu).

## Abgrenzung zu bestehenden Plänen

- **Mode-Logik** (Ask Edits/Allow all/Plan, Permission-Gate, Harness-Tools) ist
  fachlich in [agent-chat-modes-harness-control.md](agent-chat-modes-harness-control.md)
  abgehandelt. **Dieser Plan ändert nur die UI-Darstellung** der Modes (Popover
  statt Segment-Leiste) — **keine** Änderung an Permission-Semantik oder
  `AgentChatMode`-Werten. Die im Bild gezeigten Access-Stufen
  (Supervised/Auto-accept/Full-access) werden 1:1 auf die existierenden
  `AskEdits`/`AskEdits`+Auto-Edit/`AllowAll` gemappt (siehe Decisions).
- **Tasks-Leiste / RunTime** sind separat
  ([tasks-list-compact-redesign.md](tasks-list-compact-redesign.md)) — bleiben
  unangetastet.

## Decisions (zu bestätigen)

1. **Access-Popover-Mapping (ENTSCHIEDEN: auf bestehende Modes mappen):**
   Bild 4 zeigt 3 Access-Stufen, gemappt auf die **vorhandenen** drei
   `AgentChatMode`-Werte — **kein** neuer Mode-Wert, **kein** Backend-Eingriff:
   - `Supervised` → `AskEdits`
   - `Auto-accept edits` → **Alias auf `AskEdits`** (gleicher Mode-Wert, nur
     eigenes Label/Beschreibung im Popover). Echtes Auto-Accept-Verhalten kommt
     erst, wenn der [Chat-Modes-Plan](agent-chat-modes-harness-control.md) einen
     vierten `AgentChatMode` liefert — dann hier nur das Label umhängen.
   - `Full access` → `AllowAll`
   - `Plan`/`Build` Pill (Bild 3) = Plan-vs-Build-Toggle → `Plan` vs.
     „nicht-Plan“ (Build = aktueller Edit/Build-Modus).
2. **Model-Wechsel-Scope:** Composer-Model-Picker schreibt **global** über
   `agent_settings_save` (wie Settings→Agent), kein per-Workspace-Override —
   konsistent mit `model_label`-Quelle. (Per-Workspace-Override = Out of Scope.)
3. **Changed-Files-Quelle:** Working-Tree-Stand via `git_status_changes` zum
   Zeitpunkt des Turn-Endes (kein Pro-Turn-Delta-Tracking im MVP). Liefert die
   im Bild gezeigten `+adds/−dels` aus `staged_stats`+`unstaged_stats`. Card nur
   anzeigen, wenn der Turn **mutierende** Tools ausgeführt hat **und** das
   Workspace ein Git-Repo ist; sonst ausblenden.
4. **Kein neues Backend-Protokoll:** Alles aus vorhandenen Commands/Signalen.

## Phasen & Tasks

### Phase A — Grouped Toolcalls (Bild 1) — Main **und** Subagents

> **Eine** wiederverwendbare `ToolRow`/`ToolGroupCard`-Komponente für **beide**
> Kontexte: den Haupt-Agent-`ModelRound` **und** die Subagent-Karten. Heute
> rendern Subagent-Tools nur als nackte `<li><span>{label}</span>` +
> `TurnMetricsBar` ([timeline.rs:1327-1338](../../src/workbench/agent_panel/timeline.rs#L1327-L1338)) —
> ohne Icon, Arg-Summary, Status oder Grouping. Das wird angeglichen.
>
> **Design-Constraint (ausdrücklich):** Der **bestehende dezente, schmale,
> farbcodierte Pill-Look bleibt erhalten** — kein Redesign zu etwas Klobigerem.
> Referenz aus dem Ist-Zustand: das `rules_list`-Kärtchen mit kleinem
> `workflow`-Typ-Badge + `enabled/updatedAt`-Metazeile + `truncated`-Hinweis,
> sowie der eingeklappte `rules_read ×6`-Pill mit Tool-Icon links und
> Chevron/Häkchen rechts. Tokens, Größen (`font-size ~0.7rem`, `--text-muted`),
> Status-Farben und die `×N`-Count-Logik werden 1:1 weitergeführt; die Arbeit
> ist **Extraktion + Vereinheitlichung + Grouping**, nicht visuelle Neuerfindung.

- [x] `TL-A1` — **Komponenten-Extraktion**: `ModelRound`-Rendering aus
  `timeline.rs` in neuen Ordner
  `src/workbench/agent_panel/tool_group/` (`mod.rs` + `tool-group.css`)
  auslagern: `ToolGroupCard(metrics, tools, tool_detail_open)` +
  `ToolRow(tool, context: ToolRowContext)`. `timeline.rs` ruft nur noch die
  Komponente auf. `context` unterscheidet Main vs. Subagent (Metrik-Bar-Kontext
  `BarContext::Subagent` bleibt erhalten).
- [x] `TL-A2` — **Pill-Look** wie Bild 1: schmale Zeilen, Icon links
  (`leptos_icons`/`icondata` analog vorhandener Tool-Icons), Label, dezenter
  Arg-Text (Ellipsis), Status-Indikator rechts (pending puls / ok / fail),
  `×N`-Badge bei `merged_count > 1`. Klick expandiert `detail`
  (bestehendes `tool_detail_open`-Map-Muster beibehalten).
- [x] `TL-A3` — **Subagent-Tools angleichen**: `SubagentCard.tools` (gleicher
  `ToolActivity`-Typ) über dieselbe `ToolRow` rendern statt bare-`<span>`; die
  Subagent-Karte behält ihr `<details>`/Summary, der Tool-Body nutzt jetzt die
  gruppierte Pill-Liste. `attach_subagent_tool_exec`
  ([timeline.rs:236](../../src/workbench/agent_panel/timeline.rs#L236)) bleibt
  Metrik-Quelle. Grouping (`merged_count`/`paths`) auch hier anwenden, falls
  Subagents mehrere gleiche Tools feuern.
- [x] `TL-A4` — **Grouping prüfen/verfeinern**: bestehende Merge-Logik
  ([timeline.rs:708](../../src/workbench/agent_panel/timeline.rs#L708),
  [:824](../../src/workbench/agent_panel/timeline.rs#L824)) abdecken: gleiche
  Tools zusammenfassen, `paths` akkumulieren, Reihenfolge stabil. Unit-Tests für
  Merge (mehrere `read`, gemischte Tools, fail dazwischen) — Main **und**
  Subagent-Pfad.
- [x] `TL-A5` — **CSS** in `tool-group.css`, nur Theme-Tokens
  (`--surface-*`, `--text-muted`, `--accent*`, `--radius-sm`), keine
  `#literal`-Fallbacks; alte `.model-round-*`/`.agent-tool-row*`/
  `.agent-subagent-card__tools`-Regeln aus `styles.css` hierher migrieren bzw.
  entfernen. **Verschachtelung respektiert die bestehende `timeline-tree`-
  Konvention** (siehe Tree-View-Hinweis unten) — Subagent-Tools sitzen unter
  `.timeline-tree__children` (Tiefen-Einzug + linke Führungslinie).

### Phase B — Changed-Files-Card am Turn-Ende (Bilder 7–8)

- [x] `TL-B1` — **Komponente** `src/workbench/agent_panel/changed_files_card/`
  (`mod.rs` + `changed-files-card.css`): `ChangedFilesCard(changes:
  Vec<ChangedFile>)`. Header `CHANGED FILES (N) · +adds/−dels`, Buttons
  `Collapse all` + `View diff`.
- [x] `TL-B2` — **Verzeichnis-Baum (typische Tree-View)**: `Vec<ChangedFile>`
  nach `rel_path`-Segmenten in eine Baumstruktur gruppieren (Ordnerknoten
  auf-/zuklappbar, Datei-Blätter mit Per-Datei `+/−` aus
  `staged_stats`+`unstaged_stats`), Ordner-/Datei-Typ-Icon (Chevron + Folder,
  Datei-Icon nach Extension). Reine Frontend-Ableitung (eigenes
  `tree.rs`-Helfermodul im Ordner). **Visuell die bestehende `timeline-tree`-
  Konvention nutzen** ([styles.css:4783-4795](../../styles.css#L4783-L4795)):
  Tiefen-Einzug via `--timeline-depth` + `.timeline-tree__children`
  (linke Führungslinie `border-left`), damit der Baum identisch zu Subagent-/
  Model-Round-Verschachtelung wirkt. Single-Level-Ordner zusammenfalten
  (z. B. `src-tauri/src` als ein Knoten, wie im Bild).
- [x] `TL-B3` — **Trigger / Datenfluss**: neue
  `DisplayTimelineItem::ChangedFiles { changes }` **am Ende eines Model-Turns**
  emittieren, wenn (a) Turn mutierende Tools enthielt und (b)
  `git_is_repository`. Erhebung via `git_status_changes(cwd)` bei `Done`-Event
  (in `mod.rs`-Turn-Abschluss oder Reducer). Card als letztes Item des Turns
  rendern. Persistenz analog anderer Items (klein halten).
- [x] `TL-B4` — **Aktionen**: `View diff` öffnet die bestehende Git-Diff-Ansicht
  (Harness/View-Tool bzw. `wb`-Navigation wie in
  [file_diff_section](../../src/workbench/file_diff_section/mod.rs)); Datei-Klick
  öffnet die Datei/Diff. `Collapse all` klappt alle Ordnerknoten zu.
- [x] `TL-B5` — **CSS** `changed-files-card.css`, nur Tokens; grün/rot der
  Stats über vorhandene Diff-Tokens (kein Hardcode; ggf.
  [docs/THEME_EXCEPTIONS.md](../../docs/THEME_EXCEPTIONS.md) prüfen, wie
  `file_diff_section` add/del löst — wiederverwenden).

### Phase C — Moderner Composer (Bilder 2–4)

- [x] `TL-C1` — **Komponenten-Ordner** `src/workbench/agent_panel/composer/`
  (`mod.rs` + `composer.css`): ersetzt `.agent-mode-toolbar` + `.agent-compose`
  in [mod.rs:625-697](../../src/workbench/agent_panel/mod.rs#L625-L697). Props:
  `draft`, `chat_mode`, `busy`, Submit-Callback (kein Verlust bestehender
  `submit_turn`-Verdrahtung).
- [x] `TL-C2` — **Textarea** mit Auto-Grow (1→N Zeilen, Max-Height + Scroll),
  Enter=Senden / Shift+Enter=Newline, Placeholder i18n
  („Ask anything, @tag files/folders, …“). Draft-Persistenz wie bisher
  (`set_workspace_agent_compose_draft`).
- [x] `TL-C3` — **Footer-Leiste** (Bild 3): links Model-Pill (zeigt
  `model_label`) → öffnet Model-Popover; Mode/Build-Pill; Access-Pill; rechts
  runder Send-Orb (Sparkles / Stop bei `busy`). Wiederverwendbares
  Popover-Primitiv (vorhandenes Popover-Muster im Workbench nutzen, sonst kleines
  generisches `Popover` im composer-Ordner).
- [x] `TL-C4` — **Model-Picker-Popover** (Bild 2): Suchfeld + Liste aus
  `agent_provider_models(provider)`; Auswahl → `agent_settings_save` + lokales
  `model_label` aktualisieren. Loading-/Fehlerzustand. (Optional: `Ctrl+n`
  Shortcuts — Out of Scope MVP.)
- [x] `TL-C5` — **Access-/Mode-Popover** (Bild 4): Liste der Modes mit Titel +
  Beschreibung + Häkchen für aktiv; schreibt `chat_mode` (bestehende
  `set_workspace_agent_chat_mode`-Verdrahtung), **disabled while busy**.
  Mapping nach Decisions §1.
- [x] `TL-C6` — **Thinking-Level-Control**: kleines Segment/Popover (Off/Low/
  Medium/High/Max) → `agent_settings_save` (analog Model). Im Footer dezent.
- [x] `TL-C7` — **CSS** `composer.css`, nur Tokens; runder Send-Orb,
  Pills mit `--radius-pill`/`--radius-sm`, glasige Popover wie übrige App.
  Alte `.agent-compose`/`.agent-mode-toolbar`/`.workbench-agent-input*`-Regeln
  migrieren bzw. entfernen.

### Phase D — i18n & Verifikation

- [x] `TL-D1` — **i18n**: alle neuen Strings als `I18nKey` (Compile-Time-
  Exhaustiveness → **neue Keys in jeder** `src/i18n/locales/*.rs`). Kandidaten:
  `AgChangedFilesTitle`, `AgChangedFilesCollapseAll`, `AgChangedFilesViewDiff`,
  `AgComposerPh` (falls vom bestehenden `AgPromptPh` abweichend),
  `AgComposerModelSearchPh`, `AgComposerSelectModel`,
  `AgComposerThinking*`, `AgModeSupervised`/`AgModeAutoAccept`/`AgModeFullAccess`
  (+ Beschreibungen). Nicht-englische Tabellen via
  `scripts/tools/render_i18n_locales_from_en.py` (missing-keys) nachziehen.
- [x] `TL-D2` — **Verifikation**:
  `cargo check -p blxcode-ui --target wasm32-unknown-unknown`,
  `cargo test --workspace`, `scripts/lint_theme_tokens.sh`. Manuell in
  `cargo tauri dev`: Tool-Grouping (mehrere reads, gemischt), Changed-Files-Card
  nach Edit-Turn in Git-Repo / nicht in Nicht-Repo, Composer Auto-Grow, Model-
  Wechsel persistiert, Mode-/Thinking-Popover, busy-Disable, Send/Stop.

## Betroffene Dateien

- **Neu (Komponenten-Ordner je [rule-reusable-components.md](../rules/rule-reusable-components.md)):**
  - `src/workbench/agent_panel/tool_group/{mod.rs, tool-group.css}`
  - `src/workbench/agent_panel/changed_files_card/{mod.rs, tree.rs, changed-files-card.css}`
  - `src/workbench/agent_panel/composer/{mod.rs, composer.css}` (ggf. + `popover.rs`)
- **Geändert:**
  - `src/workbench/agent_panel/timeline.rs` — `ModelRound`-Render auslagern,
    Subagent-Karten-Tools auf `ToolRow` umstellen
    ([:1327-1338](../../src/workbench/agent_panel/timeline.rs#L1327-L1338)),
    neuer `ChangedFiles`-Display-Zweig (`DisplayTimelineItem`).
  - `src/workbench/agent_timeline.rs` — `TimelineItem::ChangedFiles` (persistente
    Variante) + Display-Mapping.
  - `src/workbench/agent_panel/mod.rs` — Composer einbinden (Toolbar+Form
    ersetzen), Changed-Files bei `Done` erheben.
  - `src/workbench/agent_panel/reducer.rs` — falls Turn-Abschluss/`Done` dort
    verarbeitet wird (Changed-Files-Trigger).
  - `styles.css` — alte `.model-round-*`/`.agent-tool-row*`/`.agent-compose`/
    `.agent-mode-toolbar`-Regeln entfernen/migrieren.
  - `src/i18n/locales/*.rs` + `I18nKey` — neue Keys.

## Out of Scope

- Keine neuen `src-tauri`-Protokoll-/Settings-Felder; kein neuer `AgentChatMode`
  (echtes „Auto-accept edits“ gehört in den Chat-Modes-Plan).
- Kein Pro-Turn-Diff-Delta-Tracking (Card zeigt Working-Tree-Stand).
- Keine Permission-Semantik-Änderung (siehe Chat-Modes-Plan).
- Per-Workspace-Model-Override; `@tag`-Autocomplete im Composer (nur
  Placeholder-Text), Model-Shortcuts (`Ctrl+n`).
