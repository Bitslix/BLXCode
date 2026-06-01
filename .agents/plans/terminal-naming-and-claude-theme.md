# Terminal-Benennung + Claude-Code-Theme

> Status: **done** (implementiert & verifiziert — `cargo check` beider Crates grün, 206 UI-Unit-Tests kompilieren, Theme-Lint ohne neue Verstöße).

## Summary

Terminals zeigen in der Titelbar wahlweise die native **Slot-Nummer** (`#3`)
oder einen **Agent-Namen** (z. B. `Devon`) aus einem editierbaren Pool. Die
`slot_id` bleibt überall die technische Identität (terminal_key, PTY,
sessions.json); Namen sind eine reine Anzeige-/Adressierungs-Schicht, komplett
WASM-seitig aufgelöst — das Tauri-Backend bleibt unverändert. Der BLXCode
Agent kennt die Namen (`harness.list_terminals` liefert `name`) und kann
Terminals per `slotId | name | agentSlug` ansteuern.

Zusätzlich wurde das Dark-Theme `github-dark` zu **„Claude Code"** umgefärbt
(warmes Anthrazit `#1f1e1d`/`#262624` + Coral-Akzent `#d97757`); die
BLXCode-Default-Themes blieben unangetastet.

## Decisions

- Naming-Modus ist **global** (App-weit, localStorage), nicht pro Workspace.
- Drei Auflösungs-Ebenen: Per-Slot-Override > deterministischer Pool-Pick >
  Fallback `#slot_id`. Auto-Zuweisung kollisionsfrei via `slot_id`-Hash +
  Linear-Probe — stabil, ohne Persistenz.
- Per-Slot-Override als `HashMap<u64,String>` (keyed by `slot_id`), kein
  paralleler Vektor → keine Index-Pflege bei Slot-Insert/-Remove.
- Rename per **Kontextmenü** (Header-Rechtsklick) *und* **Doppelklick** inline.
- Theme: ein vorhandenes Nicht-Default-Dark-Theme umgefärbt statt neuem Slot.

## Tasks

- [x] `config-keys` - localStorage-Keys + Default-Namens-Pool in `app.config.rs`.
- [x] `naming-module` - `terminal_naming`-Modul (Mode-Enum, `resolve_slot_name`, `display_label`, Pool-Parsing).
- [x] `app-prefs` - `AppPrefsService` um `naming_mode` + `name_pool` (Signale + Persistenz) erweitert.
- [x] `state-overrides` - `slot_name_overrides` in `WorkspaceEntry` + WorkbenchService-API (get/set/clear, Cleanup bei Close).
- [x] `header-label-rename` - Header zeigt reaktives Label; Doppelklick-Inline-Rename.
- [x] `context-menu` - Header-Kontextmenü „Umbenennen / Name zurücksetzen".
- [x] `settings-pane` - Workspace-Settings-Sektion: Modus-Toggle + editierbarer Pool (Add/Remove/Reset).
- [x] `agent-tools` - `list_terminals` liefert `name`/`namingMode`; Adressierung per `name` (case-insensitive) in `resolve_target_session`.
- [x] `harness-docs` - `harness.md` Tool-Doku aktualisiert.
- [x] `i18n` - 12 neue Keys in `keys.rs` + allen Locales (en/de übersetzt, Rest engl. Fallback).
- [x] `claude-theme` - `github-dark` → „Claude Code" (tokens.css, catalog.rs, i18n-Keys umbenannt).
- [x] `verification` - `cargo check` (UI + Tauri), Theme-Token-Lint (0 neue Verstöße).

## Follow-ups (offen, optional)

- i18n-Beschreibungen/Strings für ~15 nicht-deutsche Locales sind englische
  Fallbacks — via `scripts/render_i18n_locales_from_en.py` nachziehbar.
- Manueller Live-Test im Tauri-Fenster (Theme-Wechsel + Rename-Flow) steht aus.
