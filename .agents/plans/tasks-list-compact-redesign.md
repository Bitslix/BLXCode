# Kompakte, dezente Tasks-Leiste (modernes AI-Chat-Design)

> Status: **planned**

## Summary

Die aktuelle Tasks-Anzeige (`TaskSection` in
[task_list.rs](../../src/workbench/agent_panel/task_list.rs)) ist eine
„klotzige" aufklappbare Sektion: ein `agent-section__head`-Toggle mit
`Tasks`-Titel + `Running/Idle` und darunter eine voluminöse, immer sichtbare
Gruppen-/Listenstruktur. Ziel ist eine **dezente, schmale Status-Leiste** im
Stil moderner AI-Chats (vergleichbar mit den „Searching the web…"-Status-Pills),
die im Normalzustand **eingeklappt** ist und nur das Nötigste zeigt:

1. **Zeile 1 (Summary-Bar)** — `X/Y · <Beschreibung des aktiven Tasks>` links,
   ein **Chevron** rechts zum Auf-/Zuklappen der vollen Liste.
2. **Zeile 2 (Progress-Bar)** — links ein kleines **Donut-/Kuchen-Diagramm**
   (erledigte vs. gesamte Tasks), Label `Tasks X/Y`, rechts die **Laufzeit**
   `RunTime 00:00`.
3. **Aufgeklappt** — darunter die bestehende Task-Liste (gruppiert nach Plan /
   Free Tasks), aber visuell verschlankt.

Skizze (Bild 2): eine zweizeilige Leiste direkt unter der Hero-Box.
`Tasklist aufklappen` = Chevron rechts in Zeile 1; `Kuchen diagram` = Donut in
Zeile 2; `Laufzeit` = `RunTime` rechts in Zeile 2.

## Kontext / Ist-Zustand (verifiziert)

- **Komponente:** [task_list.rs](../../src/workbench/agent_panel/task_list.rs)
  `TaskSection(snapshot, busy, tasks_open)`. Hardcodierte englische Strings
  (`"Tasks"`, `"Running"/"Idle"`, `"No tracked tasks yet"`).
- **Einbindung:** [mod.rs:416](../../src/workbench/agent_panel/mod.rs#L416)
  `<TaskSection snapshot=task_snapshot busy=busy tasks_open=tasks_open />`.
  `tasks_open` ist `RwSignal::new(false)` ([mod.rs:55](../../src/workbench/agent_panel/mod.rs#L55))
  und wird beim Eintreffen von Tasks automatisch geöffnet
  ([mod.rs:191](../../src/workbench/agent_panel/mod.rs#L191) `tasks_open.set(count > 0)`)
  — **das wollen wir ändern**: standardmäßig eingeklappt lassen.
- **Daten:** [agent_wire.rs:102](../../src/agent_wire.rs#L102)
  `TaskSnapshot { tasks: Vec<AgentTask>, active_task_id, active_plan_path }`;
  `AgentTask` ([agent_wire.rs:83](../../src/agent_wire.rs#L83)) mit
  `status: TaskStatus` (`Pending|InProgress|Blocked|Completed|Cancelled`),
  `title`, `description`, `completed_at`.
- **Laufzeit-Quelle:** Der Session-Start liegt bereits vor — `usage.session_started_at`
  (genutzt in [session_stats.rs](../../src/workbench/agent_panel/session_stats.rs)
  via `wb.chat_usage_for_workspace`) bzw. `session_started_from_timeline(doc)`.
  → **keine Backend-Änderung nötig**, RunTime = `now − session_started_at`.
- **CSS:** alle Regeln in [styles.css](../../styles.css) ab `~3224`
  (`.agent-section--tasks`, `.agent-task-list*`, `.agent-task*`) und der
  Toggle-Head ab `~3053` (`.agent-section__head`).

## Datenableitung (client-seitig, kein `src-tauri`)

Aus `snapshot.tasks` (Memo):

- `total` = `tasks.len()`.
- `done` = Anzahl `status == Completed`.
- `progress_pct` = `done / total` (für den Donut; `0` bei `total == 0`).
- `active_desc` = Titel/Beschreibung des aktiven Tasks: zuerst Task mit
  `id == active_task_id`, sonst erster `InProgress`, sonst erster `Pending`.
  Fallback bei leer → i18n-Leerzustandstext.
- `running` = `busy.get()` (für Live-Akzent/Pulsieren).

RunTime: neuer 1-Sekunden-`Interval` (oder `set_interval_with_handle`) der ein
`now_ms`-Signal tickt; `runtime = now_ms − session_started_at`, formatiert als
`MM:SS` (bzw. `HH:MM:SS` ab 1 h). Interval nur laufen lassen / Anzeige nur
zeigen, wenn `session_started_at.is_some()`; sauber `clear` beim Unmount
(`on_cleanup`).

## Umsetzung — Tasks

- **TASK-01 — Markup-Umbau `TaskSection`**
  Neue zweizeilige, eingeklappte Leiste statt `agent-section__head`-Klotz:
  - Zeile 1: Button (`aria-expanded`, `aria-controls="agent-task-list"`) mit
    `X/Y` + `active_desc` (Text-Ellipsis) + Chevron rechts.
  - Zeile 2: Donut-SVG + `Tasks X/Y` + `RunTime MM:SS`.
  - `<Show when=tasks_open>` umschließt weiterhin die bestehende
    gruppierte Liste (`#agent-task-list`).
  Leerzustand: Leiste bleibt sichtbar mit `0/0` + dezentem Hinweistext; volle
  Liste bleibt zu.

- **TASK-02 — Donut-Progress (SVG)**
  Kleines `<svg>` (~16–18 px) mit zwei `<circle>` (Track + Fortschritt via
  `stroke-dasharray`/`stroke-dashoffset` aus `progress_pct`). `aria-hidden`,
  da `X/Y` den Wert textuell trägt. Akzentfarbe aus Theme-Tokens
  (`--accent` / `--accent-cool`).

- **TASK-03 — RunTime-Ticker**
  Memo/Signal `runtime_label`. 1 s-Interval, `now_ms`-Signal, Formatter
  `MM:SS`/`HH:MM:SS`. Quelle `session_started_at` (über `wb`/`timeline` wie in
  `session_stats.rs`). `on_cleanup` zum Stoppen.

- **TASK-04 — Default eingeklappt**
  Auto-Open in [mod.rs:191](../../src/workbench/agent_panel/mod.rs#L191)
  entfernen/abschwächen: `tasks_open` standardmäßig `false` lassen; höchstens
  beim *ersten* Task einmalig öffnen oder gar nicht (Default dezent = zu).

- **TASK-05 — CSS verschlanken** ([styles.css](../../styles.css))
  Neue Klassen `.agent-tasks-bar`, `.agent-tasks-bar__summary`,
  `.agent-tasks-bar__progress`, `.agent-tasks-bar__donut`,
  `.agent-tasks-bar__runtime`, `.agent-tasks-bar__chev`. Schmale Paddings,
  `font-size ~0.7rem`, `--text-muted`, kein Card-Look, Hover/Focus dezent,
  `--radius-sm`. Bestehende `.agent-task*`-Listenregeln beibehalten, aber
  Abstände reduzieren. Live-Akzent bei `running` (Pulse) analog
  `agent-session-stats__state--live`.

- **TASK-06 — i18n**
  Hardcodierte Strings durch `I18nKey` ersetzen (Compile-Time-Exhaustiveness →
  **neue Keys in jeder** `src/i18n/locales/*.rs`). Neue Keys z. B.
  `AgTasksTitle`, `AgTasksCount` (`X/Y`), `AgTasksRuntime`, `AgTasksEmpty`,
  `AgTasksExpand`/`AgTasksCollapse` (aria). Nicht-englische Tabellen via
  `scripts/render_i18n_locales_from_en.py` (missing-keys) nachziehen.

- **TASK-07 — Verifikation**
  `cargo check -p blxcode-ui --target wasm32-unknown-unknown`,
  `cargo test --workspace`. Manuell in `cargo tauri dev`: leer / wenige / viele
  Tasks, Auf-/Zuklappen, RunTime tickt, Donut-Füllung korrekt, Live-Akzent bei
  laufendem Agent.

## Out of Scope

- Keine Backend-/Protokoll-Änderung (`src-tauri`, `agent_wire`,
  `TaskSnapshot`) — alles aus vorhandenen Signalen ableitbar.
- Kein Umbau der Plan-/Task-Status-Logik (separat:
  [agent-plan-status-lifecycle.md](agent-plan-status-lifecycle.md)).

## Betroffene Dateien

- `src/workbench/agent_panel/task_list.rs` — Markup, Donut, RunTime, Daten-Memos.
- `src/workbench/agent_panel/mod.rs` — `tasks_open`-Default (Auto-Open).
- `styles.css` — neue `.agent-tasks-bar*`-Klassen, Listen verschlankt.
- `src/i18n/locales/*.rs` + `src/i18n/lookup`/`I18nKey` — neue Keys.
