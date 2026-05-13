# State-Persistenz + Agent-Session-Resume

Stand: 2026-05-13. Ausgangsfrage: App-Zustand (Workspaces, Layout, cwds) bei
Schließen merken und beim Öffnen wiederherstellen — inklusive der laufenden
Claude- / Codex-CLI-Sessions in den einzelnen Terminals.

## Befund

### Claude Code `--resume`

- Sessions werden als JSONL unter
  `~/.claude/projects/<encoded-cwd>/<uuid>.jsonl` abgelegt.
- Pro Projekt gibt es eine `sessions-index.json` mit Summaries.
- Aufruf: `claude --resume <id>` für eine bestimmte Session,
  `claude -c` resumed die letzte Session im aktuellen `cwd`.
- Claude Code feuert einen **`SessionStart`-Hook** mit der `session_id` im
  JSON-Payload. Damit können wir die ID präzise pro Terminal einfangen,
  ohne JSONL-Dateinamen scannen zu müssen.

### Codex CLI `resume`

- Sessions liegen unter `~/.codex/sessions/`.
- Resume unterstützt:
  - `codex resume <SESSION_ID>`
  - `codex resume --last` (letzte Session im aktuellen `cwd`)
  - `codex resume --last --all` (ohne `cwd`-Filter)
- Codex hat einen offiziellen Hook-Mechanismus:
  - Hook-Quellen: `~/.codex/hooks.json`, `~/.codex/config.toml`,
    `<repo>/.codex/hooks.json`, `<repo>/.codex/config.toml`
  - Ereignisse enthalten u. a. `session_id`, `cwd`, `hook_event_name`
  - `SessionStart` liefert `source` (`startup`/`resume`)
  - Hooks laufen im Session-`cwd`

### blxcode heute

- `WorkbenchService` hält `workspaces: RwSignal<Vec<WorkspaceEntry>>` und
  diverse Panel-Signale (`active_id`, `sidebar_collapsed`,
  `right_collapsed`, `right_width_px`, `right_tab`, `browser_url`,
  `embedded_browser_tabs`, `harness_workspace_root`).
- `WorkspaceEntry` hält `id, title, cwd, terminal_count, grid_rows,
  grid_cols, next_terminal_id, slot_ids, slot_agent_labels`.
- LocalStorage speichert nur 4 Mini-Settings (EULA, Locale, Browser-URL,
  Sandbox-Root). Keine Workspaces, kein Grid-Layout.
- `exit_app` ruft direkt `app.exit(0)` — kein Save-on-Close-Hook.
- Auto-Launch in `src/workbench/terminal_cell.rs:192`:
  `let cmd = format!("{slug}\r");` — nur der Binary-Name, keine Args.

## Vorschlag

### Phase 1 — Workbench-Snapshot persistieren (Basis, in sich abgeschlossen)

Ziel: Workspaces, Layout, cwds und Agent-Slugs überleben Neustart. Beim
Wiederöffnen läuft jedes Cell mit dem gespeicherten `cwd`, der PTY startet,
und der Auto-Launch schreibt `<slug>\r` → **frische** Agent-Session.

Bausteine:

1. **Backend-Commands** in einem neuen Modul
   `src-tauri/src/workbench_state.rs`:
   - `workbench_save_state(json: String) -> Result<(), String>`
   - `workbench_load_state() -> Result<Option<String>, String>`
   - Speicherort: `app_config_dir()/workbench.json` (cross-platform via
     `tauri::Manager::path()`).
   - Atomisches Schreiben: `tempfile` daneben + `rename`.

2. **Serializable Snapshot-Typen** in `src/workbench/state.rs` (oder
   eigene `snapshot.rs`):
   - `WorkbenchSnapshot { workspaces, active_id, sidebar_collapsed,
     right_collapsed, right_width_px, right_tab, harness_workspace_root,
     embedded_browser_tabs }`
   - `WorkspaceSnapshot { id, title, cwd, terminal_count, grid_rows,
     grid_cols, slots: Vec<SlotSnapshot> }`
   - `SlotSnapshot { cell_id, cwd, agent_slug }`
   - `serde::{Serialize, Deserialize}` ableiten; `RwSignal` selbst nicht
     serialisieren — nur die `.get_untracked()`-Werte.

3. **Auto-Save** (Frontend, in `WorkbenchService` oder im Host-Component):
   - Debounced Effect (`gloo_timers`, ~500 ms) der auf allen relevanten
     Signalen `with`-trackt → Snapshot baut → `invoke_typed(
     "workbench_save_state", { json })` aufruft.
   - Zusätzlich Save direkt vor `exit_app_ipc()` in `src/quit.rs`.

4. **Auto-Load** (Frontend):
   - In `src/app.rs` nach Login-Gate, vor Mount der `WorkbenchShell`:
     `workbench_load_state()` aufrufen, Snapshot deserialisieren, in
     `WorkbenchService::hydrate_from(snapshot)` einfüllen.
   - Falls leer/Fehler: aktueller Default-Pfad (leerer Workspace-State).

5. **Migration / Versionierung**:
   - `WorkbenchSnapshot` bekommt ein `version: u32`-Feld. Beim Load:
     unbekannte Version → ignorieren (Default-State), nicht crashen.

Erwartetes Resultat aus User-Sicht: App schließen, App öffnen, alle Tabs
und Terminals stehen wieder da, jedes Cell läuft seinen Agent-Slug neu
hoch (fresh session). Workbench fühlt sich „persistent“ an, auch ohne
Session-Restore.

### Phase 2 — Agent-Sessions resumen (oben aufgesetzt)

#### Claude (präzise, mit `SessionStart`-Hook)

1. Neues Hook-Skript `content/hooks/claude_session_capture.py`
   (analog zu `claude_title.py`), gebunden an das `SessionStart`-Event in
   `~/.claude/settings.json`.
2. PTY-Spawn erweitern: `PtyManager::spawn_session` in
   `src-tauri/src/pty_host.rs` muss eine optionale `env: HashMap<String,
   String>` annehmen. `portable_pty::CommandBuilder::env(k, v)`
   unterstützt das.
3. Beim Spawn injizieren wir `BLX_TERMINAL_ID=<uuid>` und
   `BLX_WORKSPACE_ID=<id>`. Der Hook liest die env-Vars + `session_id`
   aus dem stdin-Payload und schreibt nach
   `app_config_dir/sessions.json` als
   `{ terminal_id → { agent: "claude", session_id, last_seen } }`.
   - Schreib-Race vermeiden: temp + rename + advisory-Lock (oder pragma:
     fcntl `flock` auf POSIX, `LockFileEx` auf Windows). Für v1 reicht
     temp+rename ohne Lock.
4. `agent_hooks.rs::install_agent_hooks` erweitern: kopiert auch
   `claude_session_capture.py` rüber und trägt einen `SessionStart`-
   Eintrag in `settings.json` ein (zusätzlich zum bestehenden
   `UserPromptSubmit`-Eintrag für Title).
5. Auto-Launch in `terminal_cell.rs` ändern: vor `format!("{slug}\r")`
   prüfen ob `terminal_id → session_id` im Mapping liegt. Falls ja:
   `format!("{slug} --resume {id}\r")`. Slug bleibt String-Property —
   Args werden nur on-the-fly im Auto-Launch-Builder zusammengesetzt.

#### Codex (präzise, mit offiziellem `SessionStart`-Hook)

Codex hat laut offizieller Hooks-Doku einen `SessionStart`-Event mit
`source` (u. a. `startup`, `resume`, `clear`) und `session_id`. Genau das
nutzen wir für ein deterministisches Resume pro Terminal-Slot.

1. Projektweite Hook-Konfiguration einführen:
   - Datei: `<repo>/.codex/hooks.json`
   - Ziel: auf jedem `SessionStart` ein Capture-Skript ausführen.
   - Beispiel:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume|clear",
        "hooks": [
          {
            "type": "command",
            "command": "python3 ./.codex/hooks/blx_session_capture.py"
          }
        ]
      }
    ]
  }
}
```

2. Capture-Skript für Mapping implementieren:
   - Datei: `<repo>/.codex/hooks/blx_session_capture.py`
   - Liest Hook-Payload von `stdin` (JSON) und env:
     - Payload: `session_id`, `source`, `cwd`, `hook_event_name`
     - Env: `BLX_TERMINAL_ID`, `BLX_WORKSPACE_ID`, optional `BLX_AGENT_SLUG`
   - Schreibt atomisch in `app_config_dir/sessions.json`, Schema:

```json
{
  "terminals": {
    "<terminal-id>": {
      "agent": "codex",
      "session_id": "<id>",
      "workspace_id": "<workspace-id>",
      "cwd": "</abs/path>",
      "source": "startup|resume|clear",
      "updated_at": "2026-05-13T17:20:00Z"
    }
  }
}
```

3. PTY-Spawn um Env-Injection erweitern (wie Claude):
   - `BLX_TERMINAL_ID=<slot-id>`
   - `BLX_WORKSPACE_ID=<workspace-id>`
   - `BLX_AGENT_SLUG=codex`

4. Auto-Launch-Resume-Logik in `terminal_cell.rs`:
   - Mapping vorhanden: `codex resume <session_id>\r`
   - kein Mapping: `codex\r`
   - optionaler Fallback bei Resume-Fehler: einmalig `codex resume --last\r`
     und danach Mapping beim nächsten `SessionStart` aktualisieren.

5. Guardrails:
   - Mapping nur updaten, wenn `BLX_TERMINAL_ID` gesetzt ist (verhindert
     Vermischung mit extern gestarteten Codex-Sessions).
   - Beim Workspace-/Terminal-Close Mapping-Eintrag entfernen.
   - Bei fehlender oder kaputter `sessions.json` immer "best effort":
     Terminal normal mit `codex` starten, niemals UI blockieren.

Vorteil: keine `mtime`-Heuristik, keine Kollisionen bei mehreren Codex-
Terminals im selben `cwd`, reproduzierbares Resume pro Slot.

## Trade-offs / Risiken

- **Phase 1 risk-arm**: rein additiv, lokaler JSON-File-Write, kein neuer
  Prozess. Worst case bei korrupter Datei: User startet mit leerem
  Workbench (gleiches Verhalten wie heute).
- **Phase 2 invasiver**: PTY-Env-Plumbing, neue Hook-Dateien, Race
  Conditions bei parallelen Sessions.
- **Datenmenge**: bei vielen Workspaces wird `workbench.json` größer.
  Solange wir nur Metadaten (kein Terminal-Scrollback!) speichern, bleibt
  das in einstelligen KB.
- **PII**: `cwd` enthält Userpfade. Datei liegt im app-config-Dir des
  Users → akzeptabel.

## Empfehlung

Phase 1 jetzt komplett bauen — fühlt sich aus User-Sicht schon nach
„App merkt sich alles“ an, auch ohne tiefes Session-Resume. Phase 2 als
separate Aufgabe mit einheitlichem Hook-basiertem Resume für Claude und
Codex (gleiche Mapping-Pipeline, unterschiedliche CLI-Resume-Commands).

## Umsetzungs-Checklist (Codex Hook)

1. `.codex/hooks.json` und `.codex/hooks/blx_session_capture.py` anlegen.
2. `src-tauri/src/pty_host.rs` um Env-Map für Spawn erweitern.
3. `src-tauri/src/workbench_state.rs` um `sessions.json` read/write APIs ergänzen.
4. `src/workbench/terminal_cell.rs` auf `codex resume <id>` bei Mapping umstellen.
5. Terminal/Workspace-Close-Pfade an Mapping-Cleanup anbinden.
6. E2E-Testfall: 2 Codex-Terminals im selben `cwd`, App neu starten, beide
   Slots resumieren auf ihre jeweilige vorherige Session-ID.

## Quellen

- [Claude Code Hooks Reference](https://code.claude.com/docs/en/hooks)
- [Claude Code Settings](https://docs.claude.com/en/docs/claude-code/settings)
- [Codex Hooks](https://developers.openai.com/codex/hooks)
- [Codex CLI command-line reference](https://developers.openai.com/codex/cli/reference)
- [Codex CLI features (resume)](https://developers.openai.com/codex/cli/features)
