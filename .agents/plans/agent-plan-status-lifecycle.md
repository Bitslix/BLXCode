# Agent-getriebener Plan-/Task-Status-Lebenszyklus

## Summary

Der BLXCode Agent soll Plan-Tasks waehrend der Abarbeitung sichtbar und
zuverlaessig auf `in_progress` (Running) setzen und danach automatisch auf
den passenden Folgezustand (`pending`, `blocked`, `completed`, `cancelled`)
zuruecksetzen. Die Statusaenderung muss in die jeweilige Plan-Markdown-Datei
unter `.agents/plans/*.md` zurueckgeschrieben werden, damit Plan-Karten,
Buckets und Filter im Plans-Panel den realen Fortschritt zeigen.

Parallel fehlt im Plans-Panel ein eigener **In Progress / Running**-Filter
und eine erkennbare Running-Markierung auf den Karten. Aktuell existiert nur
der Sammelfilter `Active` (= `in_progress` + `pending`), wodurch ein gerade
laufender Plan optisch nicht von einem nur wartenden unterscheidbar ist.

## Ausgangslage (bereits vorhanden)

- `TaskStatus` kennt `Pending | InProgress | Blocked | Completed | Cancelled`
  (`src-tauri/src/tasks.rs`).
- Der Agent hat die Tools `task_list`, `task_get`, `task_create`,
  `task_update`, `task_delete`, `task_reorder` sowie `plan_load` und
  `plan_sync_from_tasks` (`src-tauri/src/agent/tool_groups.rs`).
- `plan_sync_from_tasks_inner` schreibt Statusmarker zurueck in die
  Plan-Markdown (`[ ]` pending, `[>]` in progress, `[!]` blocked, `[x]`
  completed, `[-]` cancelled) — `src-tauri/src/plans.rs`.
- Der System-Prompt fordert bereits Status-Updates an
  (`src-tauri/src/agent/system_prompt.rs`, u. a. Zeilen ~69 und ~230).
- `PlanBucket::InProgress` existiert inkl. Icon `LuCirclePlay`, wird aber im
  Filter nur ueber `PlanFilter::Active` mit `Pending` zusammengefasst
  (`src/workbench/plans_panel/mod.rs`).

## Decisions

- Keine neuen Statuswerte: der Lebenszyklus nutzt ausschliesslich die fuenf
  vorhandenen `TaskStatus`-Werte.
- Single Source of Truth bleibt die Plan-Markdown. Der Task-Store spiegelt,
  schreibt aber via `plan_sync_from_tasks` immer in die Markdown zurueck.
- Genau **ein** Task pro aktivem Plan darf gleichzeitig `in_progress` sein
  (entspricht der vorhandenen `active_task_id`-Semantik in `tasks.rs`).
- Wird ein Turn abgebrochen/unterbrochen, faellt ein nicht abgeschlossener
  `in_progress`-Task auf `pending` zurueck (kein "haengender" Running-State).
- UI-seitig wird `In Progress` ein eigener, sichtbarer Filter neben `Active`.

## Implementation Notes

### Backend / Agent

- Lebenszyklus im System-Prompt schaerfen (`system_prompt.rs`):
  - Beim Beginn der Arbeit am topmost `pending`-Task sofort `task_update`
    auf `in_progress`, *bevor* die eigentliche Arbeit startet.
  - Bei Blockern `blocked` + Begruendung in `notes`; bei Abschluss
    `completed`; bei Verwerfen `cancelled`.
  - Nach jedem Statuswechsel auf einem plan-verknuepften Task implizit
    sicherstellen, dass der Marker in der Plan-Markdown steht.
- `task_update` so absichern, dass bei gesetztem `plan_path` der
  Markdown-Writeback (`plan_sync_from_tasks`) verlaesslich ausgeloest wird,
  nicht nur best-effort. Verhalten in `tasks.rs` / Tool-Handler pruefen.
- Stale-Running-Reset: beim Turn-Start bzw. beim `plan_load` einen
  `in_progress`-Task, der nicht der aktuell aktive ist, auf `pending`
  zuruecksetzen (Logik in `apply_status` / `tasks.rs` ist hier Anker).
- Tests: Markdown-Writeback pro Statuswechsel, Single-Running-Invariante,
  Stale-Reset nach simuliertem Abbruch.

### Frontend (Plans-Panel)

- `PlanFilter` um `InProgress` erweitern und in `PlanFilter::ALL` einsortieren
  (`src/workbench/plans_panel/mod.rs`):
  - `key = "in-progress"`, `icon = LuCirclePlay`,
    `label_key = PlansTaskStatInProgress`.
  - `filter_matches` fuer `InProgress` auf `bucket == PlanBucket::InProgress`.
  - `Active` bleibt als kombinierter Filter bestehen.
- Running-Markierung auf Plan-Karten: Karten mit `in_progress > 0` eine
  sichtbare, ggf. pulsierende Markierung geben (CSS in `plans-panel.css`,
  z. B. `blx-plans-card[data-state="in-progress"]` erweitern).
- Optional: dezenter Live-Refresh des Plans-Panels, solange ein Plan
  `in_progress` ist, damit der Status ohne manuelles Refresh aktuell bleibt.
- i18n: vorhandenen Key `PlansTaskStatInProgress` nutzen; nur fuer neue
  Texte (z. B. Tooltip "Running") neue Keys in `keys.rs` + alle Locales
  via `scripts/tools/render_i18n_locales_from_en.py` ergaenzen.

## Tasks

- [ ] `prompt-lifecycle` - System-Prompt um expliziten in_progress->Folgestatus-Lebenszyklus erweitern.
- [ ] `backend-writeback` - task_update-Writeback in Plan-Markdown bei plan-verknuepften Tasks verlaesslich erzwingen.
- [ ] `backend-stale-reset` - Stale-Running-Reset bei Turn-Start/plan_load implementieren.
- [ ] `backend-tests` - Tests fuer Writeback, Single-Running-Invariante und Stale-Reset ergaenzen.
- [ ] `frontend-filter` - PlanFilter::InProgress als eigenen Filter im Plans-Panel ergaenzen.
- [ ] `frontend-running-mark` - Sichtbare Running-Markierung auf in_progress-Plan-Karten umsetzen.
- [ ] `frontend-live-refresh` - Optionalen Live-Refresh fuer laufende Plaene anbinden.
- [ ] `verification` - Cargo-Checks und manuelle UI-Pruefung des Lebenszyklus durchfuehren.
