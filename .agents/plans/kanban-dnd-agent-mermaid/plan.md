# Kanban DnD → Agent + Mermaid-Shortcuts auf Rows/Cards

## Summary

Den interaktiven Workspace-Kanban (`src/workbench/workspace_kanban/`) in zwei
Richtungen erweitern:

1. **Drag&Drop in den BLXCode-Agent**: Ein Plan oder eine Task aus dem Kanban
   kann auf das Agent-Panel gezogen und dort als Agent-Kontext angehängt werden
   (Plan → `PlanFile`-Kontext wie der bestehende "Load into agent"-Button; Task →
   gezielter Task-Kontext). Die Kanban-internen DnD-Wege (Plan/Task umsortieren,
   Status/State wechseln) bleiben unverändert.
2. **Mermaid-Indikatoren auf Rows & Cards**: Plan-Cards (Rows) und Task-Cards
   zeigen an, ob Mermaid-Diagramme vorhanden sind, und bieten einen Shortcut-
   Button/Icon, der per Klick die zentrierte Mermaid-Galerie (`DiagramGallery`)
   im Center-Tab öffnet.

Beides nutzt vorhandene Bausteine: die Kanban-DnD-Infrastruktur
(`kanban_dnd.rs`), den Agent-Drop-Pfad (`agent_panel/image_context.rs`,
`DropZoneState`), die Plan-Diagramm-API (`mermaid_list_diagrams`) und die
Galerie (`open_center_diagram_gallery_tab`).

## Decisions

- **Eine DnD-MIME bleibt**: Kanban-Drags behalten `application/x-blxcode-kanban`
  (`KANBAN_DRAG_MIME`). Der Agent-Drop erkennt diese MIME zusätzlich zu den
  bestehenden Terminal-/Context-Drags, statt eine neue MIME einzuführen.
- **Cross-Workspace verhindern**: Ein Kanban-Drop in den Agent wird nur
  akzeptiert, wenn `payload.workspace_id == wb.active_id()`. Andernfalls
  `DropZoneState::Reject`.
- **Plan-Drop = bestehende "Load into agent"-Semantik**: Plan-Drop ruft die
  gleiche Logik wie [`load_plan_into_agent`](../../../src/workbench/plans_panel/mod.rs)
  (`plan_load` + `upsert_workspace_agent_context` mit `AgentContextKind::PlanFile`).
  Diese Logik wird in einen wiederverwendbaren Helper in
  `agent_context_handoff.rs` gezogen, damit Plans-Panel und Kanban/Agent-Drop sie
  teilen (Rule: keine Duplikate, `rule-reusable-components`).
- **Task-Drop = gezielter Task-Kontext**: Eine einzelne Task wird als kompakter
  Kontext-Eintrag angehängt (Kind: `AgentContextKind::PlanTaskGroup`, Label =
  Task-Titel, `paths` = Plan-Pfad, `source` = `plan_path#task_id`). Falls sich
  beim Implementieren zeigt, dass nur ganze Plans sinnvoll an den Agent gehen,
  wird Task-Drop auf den Plan-Kontext des Eltern-Plans gemappt (Decision dann hier
  festhalten).
- **Mermaid ist plan-slug-basiert**: `DiagramRecord` trägt ein optionales
  `task_id`. Daraus folgt:
  - Plan-Card-Indikator = Anzahl aller Diagramme des Slugs.
  - Task-Card-Indikator = vorhanden, wenn mindestens ein Diagramm
    `task_id == task.id` hat.
  - Beide Shortcuts öffnen denselben Galerie-Tab des Slugs
    (`open_center_diagram_gallery_tab(ws_id, slug)`). Ein Task-genaues Fokussieren
    des ersten passenden Diagramms ist optional (siehe Task `mermaid-task-focus`).
- **Drop-Overlay-Texte**: Neue `DropZoneState`-Varianten (`AcceptPlan`,
  `AcceptTask`) liefern hartkodierte englische Meldungen im Stil der bestehenden
  Varianten (die heutigen `message()`-Strings sind ebenfalls nicht i18n-isiert) —
  daher kein neuer I18n-Key für die Overlay-Texte nötig.
- **i18n nur wo nötig**: Für den Diagramm-Shortcut auf Kanban-Cards wird der
  bestehende Key `I18nKey::PlansOpenDiagrams` wiederverwendet. Nur falls ein
  Kanban-spezifischer Tooltip gewünscht ist, kommt ein neuer Key dazu — dann muss
  er in **allen** `src/i18n/locales/*.rs` ergänzt werden (`scripts/
  render_i18n_locales_from_en.py` für die Übersetzungen).

## Implementation Notes

### Phase A — Mermaid-Indikatoren auf Rows & Cards

- **Plan-Card** (`KanbanPlanCard`, `workspace_kanban/mod.rs`):
  - `has_diagrams`/`diagram_count`-Signal analog zu
    [`plans_panel/mod.rs:742-766`](../../../src/workbench/plans_panel/mod.rs):
    `Effect` auf `wb.plans_epoch()`, `mermaid_list_diagrams(&ws, &plan.meta.slug)`.
    Slug ist über `plan.meta.slug` bereits vorhanden.
  - Im `__head`-Bereich (neben `__stats`) ein Badge + Shortcut-Button rendern,
    nur wenn Diagramme existieren. Klick → `ev.stop_propagation()` (Header-Toggle
    nicht auslösen) + `wb.open_center_diagram_gallery_tab(ws_id, slug)`.
  - Button bekommt `data-kanban-no-card-drag="true"` und `prop:draggable=false`,
    damit der Klick nicht den Plan-Drag startet
    (vgl. `drag_started_from_control`).
  - Icon: `icondata::LuWorkflow` (konsistent mit Plans-Panel).
- **Task-Card** (`KanbanTaskCardView`): pro Task prüfen, ob der Plan Diagramme mit
  `task_id == task.id` hat. Um N+1-Requests pro Karte zu vermeiden, die
  Diagramm-Liste **auf Plan-Ebene einmal laden** und task-gefiltert an die Lanes/
  Cards durchreichen (z. B. `RwSignal<Vec<DiagramRecord>>` im `KanbanPlanCard`,
  als Prop/StoredValue an `KanbanTaskLane`/`KanbanTaskCardView`). Task-Badge +
  kleiner Button im `__foot`, Klick öffnet die Galerie (optional task-fokussiert).
- **CSS** in `workspace-kanban.css`: Badge-/Button-Styles im Komponentenordner,
  nur `var(--token)` aus `themes/tokens.css` (Rule `rule-theme-tokens`).

### Phase B — Drag Plan/Task in den Agent

- **Reusable Helper** in `agent_context_handoff.rs`:
  - `attach_plan_into_agent(wb, ws_id, ws_cwd, plan_path)` — kapselt `plan_load`
    + `PlanFile`-`AgentContextItem` + Task-Snapshot-Store (heute inline in
    `load_plan_into_agent`). `plans_panel` ruft danach denselben Helper.
  - `plan_task_context_item(plan_path, task_id, title)` → `AgentContextItem`
    (Kind `PlanTaskGroup`) analog zu den anderen `*_context_item`-Helpern.
- **Agent-Drop-Pfad** (`agent_panel/image_context.rs`):
  - `DropZoneState` um `AcceptPlan` / `AcceptTask` erweitern (+ `is_active`,
    `message()`).
  - `handle_dom_drag_event` zusätzlich `KanbanDragService` annehmen: wenn
    `is_kanban_drag(dt)` bzw. `kanban_dnd.session_active()` und
    `payload.workspace_id == active_id` → `AcceptPlan`/`AcceptTask`
    (nach `payload.kind`), `set_drop_effect("copy")`, Overlay-Pos setzen; bei
    fremdem Workspace → `Reject`.
  - `handle_dom_drop` zusätzlich `KanbanDragService` annehmen: zuerst
    `read_drag_payload`(kanban) bzw. aktiver Payload prüfen; Plan-Drop →
    `attach_plan_into_agent`, Task-Drop → `plan_task_context_item` +
    `upsert_workspace_agent_context`; danach `kanban_dnd.clear()`.
  - Reihenfolge in `handle_dom_drop`: Kanban-Payload **vor** Terminal/Context
    prüfen (eigene MIME, keine Überschneidung).
- **Aufrufstellen** (`agent_panel/mod.rs`): `KanbanDragService` via
  `expect_context` holen und an `handle_dom_drag_event`/`handle_dom_drop`
  durchreichen; `class`-Matcharm um die neuen Accept-States erweitern (Drop-
  Active-Styling).
- **Overlay**: Während eines Kanban-Drags zeigt bereits `kanban_drag_overlay.rs`
  die Ghost-Karte; das Agent-Drop-Overlay (`agent-drop-overlay`) greift über
  `drop_state`. Prüfen, dass beide gleichzeitig sauber aussehen (kein doppelter
  Cursor-Geist), ggf. Overlay-Text kurz halten.
- **Kanban-Seite**: Plan-/Task-`<article>` sind bereits `draggable` und stempeln
  den Payload. Es ist **keine** Änderung an `start_kanban_drag` nötig — der Agent
  ist nur eine neue Drop-Senke. `on:dragend=kanban_dnd.clear()` bleibt; der
  Agent-Drop ruft zusätzlich `clear()` nach erfolgreichem Attach.

## Tests

- **Frontend-Build**: `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
  fehlerfrei (insb. i18n-Exhaustiveness, falls neue Keys ergänzt werden).
- **Workspace-Build**: `cargo check -p blxcode` (Helper-Signaturen).
- **Unit**: Reine Helper testen, wo möglich — `plan_task_context_item` (Kind,
  Label, `source`, `paths`), und ein DropZone-Klassifizierer für Kanban-Payloads
  (Plan vs Task vs fremder Workspace → Reject) ohne DOM.
- **Manuell (Tauri-Shell)**:
  - Plan-Card auf den Agent ziehen → Plan erscheint als Agent-Kontext, Tasks
    werden geladen (wie "Load into agent").
  - Task-Card auf den Agent ziehen → Task-Kontext angehängt; falsche Lane/Plan
    nicht verändert.
  - Drag aus Workspace A auf Agent von Workspace B (falls möglich) → Reject-Overlay.
  - Plan-/Task-Umsortieren und Status-/State-Wechsel im Kanban funktionieren
    weiterhin unverändert.
  - Plan mit Diagrammen: Badge + Shortcut auf der Row; Klick öffnet Galerie-Tab,
    re-fokussiert bei erneutem Klick statt zweiten Tab zu öffnen.
  - Task mit verknüpftem Diagramm (`task_id`): Badge auf der Task-Card; ohne
    Diagramm kein Badge.
  - Plan ohne Diagramme: kein Badge/Button.
  - Theme-Wechsel: Badges/Buttons nutzen Tokens (kein hartkodiertes Farb-Flackern).

## Tasks

- [x] `mermaid-plan-card` - Plan-Card (Row): has_diagrams/Count laden und Badge + Galerie-Shortcut im Header rendern (no-card-drag), öffnet `open_center_diagram_gallery_tab`
- [x] `mermaid-task-card` - Diagramm-Liste auf Plan-Ebene einmal laden und task-gefiltert an Lanes/Task-Cards durchreichen; Task-Badge + Shortcut bei `task_id`-Match
- [ ] `mermaid-task-focus` - Optional: Galerie task-fokussiert öffnen (erstes Diagramm der Task vorselektieren) via erweiterten Scope/Fokus-Param
- [x] `mermaid-card-css` - Badge-/Button-Styles in `workspace-kanban.css` mit Theme-Tokens
- [x] `agent-context-helper` - `attach_plan_into_agent` + `plan_task_context_item` in `agent_context_handoff.rs` extrahieren; `plans_panel::load_plan_into_agent` auf Helper umstellen
- [x] `dropzone-states` - `DropZoneState::AcceptPlan`/`AcceptTask` inkl. `is_active`/`message` ergänzen
- [x] `agent-drag-detect` - `handle_dom_drag_event` um Kanban-Drag erweitern (Workspace-Match → Accept, sonst Reject), `KanbanDragService` durchreichen
- [x] `agent-drag-drop` - `handle_dom_drop` um Kanban-Payload erweitern (Plan→PlanFile, Task→PlanTaskGroup), `kanban_dnd.clear()` nach Attach
- [x] `agent-panel-wire` - `agent_panel/mod.rs`: `KanbanDragService` injizieren, Handler-Calls anpassen, Drop-Active-Klassenarm um neue States erweitern
- [x] `dnd-tests` - Unit-Test für `plan_task_context_item` (Kind/Label/source/paths). Der Accept/Reject-Pfad ist inline im DOM-Handler (Services + DragEvent) und wird manuell geprüft statt künstlich extrahiert
- [>] `dnd-mermaid-verify` - `cargo check` beider Crates + Token-Lint grün; manuelle Tauri-Checks aus dem Tests-Abschnitt stehen noch aus (GUI)
