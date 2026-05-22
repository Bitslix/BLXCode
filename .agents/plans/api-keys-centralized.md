# API Keys zentralisieren

## Summary

Zentrale Verwaltung aller API-Schlüssel unter **Settings → API Keys**. Agent-Tab behält nur Provider/Model/Thinking und Web-Backend-Auswahl. Backend-Modul `api_keys` mit Katalog, einheitlichem `resolve`, Keyring-Kompatibilität.

Vollständiger Plan inkl. User-Entscheidungen: siehe Cursor-Plan `api_keys_zentralisieren` oder Abschnitte unten.

## Decisions

- Web-Keys (Tavily/Brave) in API Keys; Provider-Wahl bleibt im Agent-Tab.
- UI: `src/workbench/settings/api_keys/` (Ordner `settings/` jetzt anlegen).
- Katalog und Resolver im Backend; `api_keys_catalog` ist Quelle der Wahrheit.
- `key_statuses` aus `agent_settings_get` entfernen.
- Optionaler Env-Fallback für aktive Keys — **kein `.env` Pflicht**; Keyring/UI bleibt Standard. Reihenfolge: Keyring → (Agent: Fallback-Datei) → Env.
- Icons: brand-icons wenn vorhanden, sonst LuIcon-Fallback.
- Keyring-Konten unverändert; Coming-soon-Provider mit vollem CRUD.

## Tasks

- [ ] `api-keys-backend` - Modul `api_keys/`, Katalog, resolve, Commands, Delegation; `key_statuses` entfernen
- [ ] `api-keys-bridge` - tauri_bridge, agent_wire, lib.rs
- [ ] `settings-scaffold` - `src/workbench/settings/mod.rs` + wiring
- [ ] `api-keys-ui` - `settings/api_keys/` mit Backend-Katalog + ApiKeyRow
- [ ] `agent-pane-trim` - AgentProviderPane ohne Key-UI
- [ ] `i18n-docs` - Locales + user/developer docs
