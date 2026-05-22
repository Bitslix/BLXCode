# API Keys zentralisieren

## Summary

Zentrale API-Schlüssel unter **Settings → API Keys**. UI folgt **App**- und **Workspace**-Harness-Stil; **ein** Speichern-Button für alle Keys. Backend: Katalog, Batch-`api_keys_apply`, zentraler `resolve`.

## Decisions

- Design-Vorlage: `harness-pane`, `harness-subpane`, `harness-stack`, `workbench-plain-input`, Footer wie Workspace (`workbench-mini-btn--primary` + `LuSave` + `BtnSave`).
- **Kein Save pro Zeile**; optional „Key entfernen“ pro Zeile → Draft, Ausführung mit globalem Save.
- Batch-IPC: `api_keys_apply`.
- (Siehe Cursor-Plan für Keyring, Env, Coming soon, Pfade.)

## Tasks

- [ ] `api-keys-backend` - Katalog, resolve, `api_keys_apply`
- [ ] `api-keys-bridge` - tauri_bridge, agent_wire
- [ ] `settings-scaffold` - `settings/mod.rs`
- [ ] `api-keys-ui` - Harness-Stil, ein Save-Footer, Draft-State
- [ ] `agent-pane-trim` - Agent ohne Key-UI
- [ ] `i18n-docs` - Locales + docs
