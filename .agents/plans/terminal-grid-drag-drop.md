# Terminal Drag & Drop (Grid + Sidebar)

## Summary

Terminal-Slots im Workspace-Grid per Drag-Handle umsortieren und per Drop auf einen Sidebar-Workspace in einen anderen Workspace transferieren. Cross-Workspace-Transfer migriert `sessions.json` und `notifications.json` (Prefix-Rewrite) und erhält laufende PTY-Sessions über Remount hinweg (xterm neu, gleiche `session_id`).

Grid-Reihenfolge folgt `WorkspaceEntry.slot_ids` (+ parallele `slot_agent_labels` / `slot_pane_states`). Referenzmuster: bestehendes Workspace-Reorder-DnD in `sidebar.rs`.

## Decisions

- **Letzter Slot:** Transfer blockieren, wenn der Quell-Workspace danach 0 Terminals hätte.
- **PTY:** Cross-Workspace-Transfer darf `pty_kill` nicht aufrufen; Adoption-Pfad in `terminal_cell.rs`.
- **Transfer-Ziel (anderer Workspace):** Slot ans Ende der Ziel-Liste; danach `select_workspace(to_id)`.
- **Drag-Einheit:** Ganzer Slot inkl. aller Split-Panes.
- **Grid-Drop:** Einfügen an Drop-Position (kein Swap), analog `reorder_workspaces`.
- **CWD-Mismatch:** Shell behält aktuelles cwd; kein `cd`, kein Blockieren.
- **Notifications:** v1 Pflicht — Prefix-Rewrite via `workbench_rewrite_notifications_prefix`.
- **Drop auf gleichen Sidebar-Workspace:** Reorder ans Ende desselben Workspaces.
- **Defaults:** Drag deaktiviert bei collapsed Sidebar, Wizard/Configure-Modus und Fullscreen-Slot (Fullscreen vor Drag aufheben).

## Implementation Notes

### Neues Modul `src/workbench/terminal_slot_dnd/`

- MIME: `application/x-blxcode-terminal-slot`
- Payload: `{ "fromWorkspaceId", "slotId", "fromIndex" }`
- Helfer: `set_drag_payload`, `read_drag_payload`, `is_terminal_drag`
- Optional: `TerminalSlotDragService` (Leptos-Context) für Grid-Feedback

### State (`state.rs`)

**`reorder_terminal_slots(workspace_id, from_index, to_index)`** — parallele Permutation von `slot_ids`, `slot_agent_labels`, `slot_pane_states`; kein PTY-Impact.

**`transfer_terminal_slot(from, to, slot_id) -> Result<(), String>`**

- Validierung: `from != to`, Quelle > 1 Slot, Ziel < 16 Slots, `slot_id` vorhanden
- Atomisch: Slot extrahieren, Quelle/Ziel `set_count_and_dims`, `PendingSlotTransfer` mit `pane_id → session_id` aus `pty_sessions`
- Async: `workbench_extract_sessions_prefix` + `workbench_merge_sessions_workspace` + notification rewrite
- Fokus: `focused_terminal_by_workspace` auf neuen `terminal_key` umschreiben
- Helfer: `is_pty_preserved_on_unmount`, `take_pty_adoption`

### PTY (`terminal_cell.rs`)

- Cleanup bei Transfer: `terminal_dispose`, kein `pty_kill`
- Bootstrap: `take_pty_adoption` → `pty_peek_output` + neuer Drain-Loop + `register_pty_session`

**Akzeptierte v1-Einschränkungen:** `BLX_TERMINAL_KEY` in der Shell-Umgebung bleibt alt; cwd des Prozesses bleibt unabhängig vom Ziel-Workspace.

### UI

- **`workspace_panel.rs`:** Drag-Handle (z. B. `LuGripVertical`) in Titlebar; Drop auf Slot → `reorder_terminal_slots`; Transfer nur über Sidebar
- **`sidebar.rs`:** Terminal-Payload in `drop`/`dragover` neben Workspace-Reorder; gleicher Workspace → Reorder ans Ende

### Backend + Bridge

- `workbench_rewrite_notifications_prefix` in `src-tauri/src/workbench_state.rs`
- Bridge in `src/tauri_bridge.rs`

### CSS + i18n

- Klassen: `.ws-term-slot--drag-source/over`, `.ws-term-cell__drag-handle`, `.workbench-sidebar__item--terminal-drop-target`
- Neue Keys in `keys.rs` + alle `locales/*.rs` (letzter Slot, Ziel voll, optional Erfolg/Fehler)

### Betroffene Dateien

| Datei | Änderung |
|---|---|
| `src/workbench/terminal_slot_dnd/mod.rs` | neu |
| `src/workbench/state.rs` | Reorder/Transfer + PendingTransfer |
| `src/workbench/workspace_panel.rs` | Grid-DnD |
| `src/workbench/sidebar.rs` | Terminal-Drop |
| `src/workbench/terminal_cell.rs` | PTY preserve/adopt |
| `src/workbench/mod.rs` | Mod registrieren |
| `src-tauri/src/workbench_state.rs` | notification rewrite |
| `src/tauri_bridge.rs` | Bridge |
| `styles.css` | Drag-Feedback |
| i18n | Fehlermeldungen |

## Tests

| Szenario | Erwartung |
|---|---|
| 2+ Slots, Grid-Drag | Reihenfolge persistiert; PTY läuft weiter |
| Drop auf anderen Sidebar-Workspace | Slot am Ende; aktiver Workspace wechselt |
| Letzter Slot → Sidebar | Blockiert + Fehlermeldung |
| Ziel mit 16 Slots | Blockiert |
| Cross-Workspace + laufende Shell | Eingabe funktioniert; kein Prozess-Neustart |
| Cross-Workspace + Agent-Slot | `sessions.json` unter neuem Prefix |
| Cross-Workspace + unterschiedliches cwd | Shell-cwd unverändert |
| Drop auf gleichen Sidebar-Workspace | Reorder ans Ende |
| Notifications nach Transfer | Unread-Badges erhalten |
| Sidebar Workspace-Reorder | Unabhängig vom Terminal-Drag (MIME-Typen) |
| Grid-Resize-Handles | Weiter nutzbar |

- Manuell: `cargo tauri dev` — zwei Workspaces, je 2 Terminals, `sleep 999`, Transfer + Reload
- Automatisch: Unit-Tests für `reorder_terminal_slots` und Transfer-Validierung in `state.rs`

## Tasks

- [ ] `dnd-module` - Neues Modul `terminal_slot_dnd/` mit MIME-Payload, Drag-Context und Hilfsfunktionen
- [ ] `state-reorder` - `reorder_terminal_slots` + Unit-Tests für parallele Vektor-Permutation
- [ ] `state-transfer` - `transfer_terminal_slot` mit Validierung, PendingTransfer, Session-Migration und Fokus-Update
- [ ] `pty-adopt` - PTY-Erhalt in `terminal_cell.rs`: preserve on cleanup, adopt + peek_tail on bootstrap
- [ ] `grid-ui` - Drag-Handle + Drop-Handler auf `TerminalSlotSurface` in `workspace_panel.rs`
- [ ] `sidebar-drop` - Sidebar `drop`/`dragover` für Terminal-Payload neben Workspace-Reorder
- [ ] `css-i18n` - Drag-Feedback-CSS + i18n-Fehlermeldungen (letzter Slot, Ziel voll)
- [ ] `notifications-migrate` - `workbench_rewrite_notifications_prefix` für Unread-Erhalt beim Transfer
