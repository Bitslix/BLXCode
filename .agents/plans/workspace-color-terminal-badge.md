# Workspace-Farbe und Terminal-Badge in der Sidebar

## Summary

Workspaces erhalten ein persistentes `color`-Feld (analog Memory-Kategorien), änderbar über den bestehenden Kontextmenü-Dialog. In der Sidebar erscheinen ein Farbpunkt links, eine Terminal-Slot-Zahl vor dem Namen (nur bei mehr als einem Slot) und ein farblich passendes Unread-Badge rechts. Persistenz über `workbench.json`; keine Backend-Änderungen.

## Decisions

- **Terminal-Zahl:** zählt Terminal-Slots (`slot_ids.len()`), nicht Split-Panes oder laufende PTY-Sessions.
- **Anzeige Terminal-Badge:** nur wenn `slot_ids.len() > 1` (bei einem Slot ausblenden).
- **Default-Farbe:** `stable_category_color(&storage_key)` für neue und backgefüllte Workspaces.
- **Farb-Presets:** bestehende `memory_color_presets()` wiederverwenden (kein neuer localStorage-Key).
- **Kontextmenü:** Rename-Dialog wird zu „Workspace bearbeiten" mit Name + Farbe (wie `MemoryCategoryEditDialog`).
- **Terminal-Badge-Farbe:** fest orange `#e8954a` (Mockup); Unread-Badge rechts nutzt Workspace-Farbe.

## Implementation Notes

### Datenmodell — [`src/workbench/state.rs`](../../src/workbench/state.rs)

- `WorkspaceEntry.color: String` mit `#[serde(default)]` (leer = noch nicht gesetzt).
- `workspace_effective_color(entry)` — gespeicherte Farbe oder `stable_category_color(&entry.storage_key)`.
- Backfill leerer `color`-Felder beim Laden (`backfill_workspace_colors()` oder in `backfill_storage_keys()`).
- `create_workspace`: initiale Farbe setzen.
- Setter: `set_workspace_display(id, title, color)` oder `rename_workspace` + `set_workspace_color` erweitern.
- `normalize_memory_color` aus [`memory_panel.rs`](../../src/workbench/memory_panel.rs) nach shared Modul (`state.rs` oder `color_util.rs`) verschieben.

### Kontextmenü & Dialog — [`src/workbench/sidebar.rs`](../../src/workbench/sidebar.rs)

- Rechtsklick-Menü: Eintrag „Bearbeiten" öffnet Dialog mit Name, `<input type="color">`, Hex-Feld, Swatches aus `wb.memory_color_presets()`.
- Speichern schreibt Titel + normalisierte Farbe → debounced Auto-Save in [`mod.rs`](../../src/workbench/mod.rs).

### Sidebar-Rendering — [`src/workbench/sidebar.rs`](../../src/workbench/sidebar.rs)

Zeilenlayout (expanded):

```mermaid
flowchart LR
  dot[Farbpunkt] --> termBadge[Terminal-Zahl] --> name[Name] --> unreadBadge[Unread]
```

- Farbpunkt: `.workbench-sidebar__color-dot` mit `--workspace-color`.
- Terminal-Badge: `.workbench-sidebar__terminal-count` vor dem Namen.
- `▸`-Bullet entfernen oder durch Farbpunkt ersetzen.
- Unread-Badge: inline-style mit Workspace-Farbe statt festem Orange.
- Collapsed-Modus: Farbe am Icon-Rand oder als Hintergrund des Initialen-Kästchens.

### CSS — [`styles.css`](../../styles.css)

Neue/angepasste Klassen: `__color-dot`, `__terminal-count`, dynamisches `__badge--total`, optional `__row--active` mit `--workspace-color` für `border-left-color`.

### i18n — [`src/i18n/keys.rs`](../../src/i18n/keys.rs) + alle `locales/*.rs`

Neue Keys: `SbEditMenu`, `SbEditTitle`, `SbEditSubmit`, `SbColorLabel`, `SbTerminalCountAria`.

## Tests

- Neuer Workspace: Farbpunkt sichtbar, persistiert nach Neustart.
- Bestehende Workspaces ohne `color` in JSON: Backfill weist stabile Farbe zu.
- Kontextmenü → Bearbeiten → Farbe ändern → Neustart → Farbe bleibt.
- Terminal-Slots hinzufügen/entfernen: Zahl vor Name aktualisiert sich reaktiv.
- Unread-Badge rechts nutzt Workspace-Farbe.
- Collapsed-Sidebar: Farbe weiterhin erkennbar.

```bash
cargo check -p blxcode-ui --target wasm32-unknown-unknown
cargo test --workspace
```

## Tasks

- [ ] `model-color` - WorkspaceEntry.color, workspace_effective_color, Backfill und Setter in state.rs
- [ ] `shared-color-util` - normalize_hex_color aus memory_panel extrahieren und gemeinsam nutzen
- [ ] `edit-dialog` - Rename-Dialog in sidebar.rs zu Edit-Dialog mit Color-Picker und Presets erweitern
- [ ] `sidebar-ui` - Farbpunkt, Terminal-Slot-Badge und farbiges Unread-Badge rendern
- [ ] `css` - Neue Sidebar-Klassen in styles.css; Active-State mit Workspace-Farbe
- [ ] `i18n` - Neue I18nKeys in keys.rs und allen locales/*.rs
