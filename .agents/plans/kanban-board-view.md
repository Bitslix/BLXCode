# Kanban Board View fuer Plan-Tasks

## Summary

Ergaenze im bestehenden Plans-Panel eine schnell umschaltbare Kanban-Ansicht fuer alle Plan-Tasks eines Workspaces. Die Datenquelle bleiben die Markdown-Plaene unter `.agents/plans/*.md`; `PLANS.md` ist weiter der geschuetzte Index und wird fuer Kanban-Tasks ignoriert.

Die Kanban-Spalten sind fest an die vorhandenen Task-Statuswerte gebunden: pending, in progress, blocked, completed und cancelled. User koennen Spalten per Drag and Drop umsortieren und leere Spalten ausblenden oder wieder einblenden. Task-Karten koennen per Drag and Drop zwischen Spalten verschoben werden; das aktualisiert den Task-Status und schreibt den passenden Marker zurueck in die jeweilige Plan-Markdown-Datei.

## Decisions

- Es gibt keine frei definierbaren neuen Statuswerte.
- "Boards anlegen/entfernen/bewegen" wird als Spaltenverwaltung interpretiert, nicht als mehrere unabhaengige Boards.
- Kanban zeigt alle Tasks aus Nicht-Index-Plaenen im aktiven Workspace.
- Freie Tasks ohne `planPath` bleiben ausserhalb dieser Kanban-View.

## Implementation Notes

### Backend

- Neues fokussiertes Kanban/Plan-Task-Modul anlegen, statt `plans.rs` weiter wachsen zu lassen.
- Workspace-Konfiguration unter `.blxcode/kanban/index.json` speichern:
  - `version`
  - `columnOrder: TaskStatus[]`
  - `hiddenColumns: TaskStatus[]`
  - `cardOrder: { cardKey: string, rank: u32 }[]`
- Neue Tauri-Kommandos ergaenzen:
  - `kanban_settings_get(workspaceCwd)`
  - `kanban_settings_save(workspaceCwd, settings)`
  - `plan_task_list_all(workspaceCwd)`
  - `plan_task_create(workspaceCwd, planPath, status, title)`
  - `plan_task_update(workspaceCwd, planPath, planTaskId, patch)`
  - `plan_task_delete(workspaceCwd, planPath, planTaskId)`
- Plan-Task-Mutationen muessen die vorhandenen Parser/Rewriter aus `plans.rs` nutzen, damit Markdown-Syntax und Statusmarker konsistent bleiben.
- Wenn ein Kanban-Task bereits im `.blxcode/tasks`-Store gespiegelt ist, soll der Store best-effort mit aktualisiert werden.

### Frontend

- `PlansPanel` um einen View-Mode `Editor | Preview | Kanban` erweitern.
- Kanban als eigenen Komponentenordner unter `src/workbench/plans_panel/kanban/` umsetzen, inklusive eigener CSS-Datei.
- Typed IPC-Wrappers und Wire-Typen in `tauri_bridge.rs` ergaenzen.
- Kanban-Toggle in die bestehende Plans-Toolbar setzen.
- Board-Layout:
  - horizontale Status-Spalten mit Scroll bei schmalem Right-Panel
  - moderne, dichte Workspace-UI im bestehenden dunklen Theme
  - Statusfarben: blau aktiv, gelb blocked, gruen completed, rot/gedaempft cancelled
  - Karten zeigen Task-Titel, Plan-Badge/Pfad und Status-Akzent
- Spaltenverwaltung:
  - Spalten-Drag-and-Drop persistiert `columnOrder`
  - Ausblenden ist nur fuer leere Spalten erlaubt
  - Wiedereinblenden erfolgt ueber ein kleines Columns-Menue
- Task-Management:
  - Quick-add pro Spalte erstellt eine Task im Zielplan
  - Zielplan-Auswahl im Kanban-Header
  - Default-Zielplan: aktuell gewaehlter Nicht-Index-Plan, sonst `activePlanPath`, sonst erster Nicht-Index-Plan
  - Karten-Drop in andere Spalte aktualisiert den Status im Plan-Markdown
  - Karten-Drop innerhalb derselben Spalte aktualisiert nur die Kanban-Reihenfolge

## Public Interfaces

- Neue IPC-Typen:
  - `KanbanSettings`
  - `KanbanColumnStatus`
  - `PlanTaskCard`
  - `PlanTaskCreateInput`
  - `PlanTaskUpdatePatch`
- Bestehende `TaskStatus`-Werte bleiben unveraendert.
- Bestehende `plan_load`, `plan_sync_from_tasks` und `tasks_update` bleiben kompatibel.
- I18n-Keys fuer Kanban-Toggle, Columns-Menue, Empty-State, Zielplan-Auswahl, Add/Delete und Drag-Fehler in allen Locale-Tabellen ergaenzen.

## Tests

- Backend:
  - Kanban-Settings werden initial mit Default-Spalten erzeugt und persistiert.
  - `plan_task_list_all` liest Tasks aus mehreren Nicht-Index-Plaenen und ignoriert `PLANS.md`.
  - Status-Update schreibt korrekte Marker: `[ ]`, `[>]`, `[!]`, `[x]`, `[-]`.
  - Task-Erstellung erzeugt stabile eindeutige `planTaskId`s und haengt an `## Tasks` an.
  - Task-Loeschung entfernt nur die passende Task-Zeile und erhaelt andere Markdown-Abschnitte.
  - Pfad-Sandboxing verhindert absolute Pfade, `..` und Nicht-Markdown-Ziele.
- Frontend:
  - `cargo check -p blxcode-ui --target wasm32-unknown-unknown`
  - `cargo check -p blxcode`
  - Manuelle UI-Pruefung in Tauri/Trunk: Toggle Editor/Kanban, Karten-DnD, Spalten-DnD, Spalten aus-/einblenden, Quick-add.

## Tasks

- [ ] `backend-kanban-store` - Workspace Kanban settings store unter `.blxcode/kanban/index.json` implementieren.
- [ ] `backend-plan-task-api` - Plan-Task IPC fuer Listen, Erstellen, Aktualisieren und Loeschen ergaenzen.
- [ ] `backend-sync-tests` - Parser/Rewriter-, Sandbox- und Markdown-Writeback-Tests fuer Kanban-Mutationen abdecken.
- [ ] `frontend-kanban-module` - Kanban-Komponentenordner, typed IPC-Wrappers und View-Mode im Plans-Panel ergaenzen.
- [ ] `frontend-dnd-columns` - Spaltenreihenfolge, Ausblenden und Wiedereinblenden mit Drag and Drop und persistierter Config bauen.
- [ ] `frontend-dnd-cards` - Karten zwischen Status-Spalten und innerhalb von Spalten verschiebbar machen.
- [ ] `frontend-task-actions` - Quick-add, Zielplan-Auswahl und Delete/Edit-Aktionen fuer Kanban-Karten anbinden.
- [ ] `i18n-docs` - I18n-Keys und User/Developer-Dokumentation zur Kanban-View ergaenzen.
- [ ] `verification` - Cargo-Checks und manuelle UI-Pruefung durchfuehren.
