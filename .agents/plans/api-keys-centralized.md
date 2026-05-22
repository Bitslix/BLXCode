# API Keys zentralisieren

## Summary

Zentrale API-Schlüssel unter **Settings → API Keys**. UI folgt **App**- und **Workspace**-Harness-Stil; **ein** Speichern-Button für alle Keys. Backend: Katalog, Batch-`api_keys_apply`, zentraler `api_keys::resolve` als einzige Lookup-Quelle für Agent, Subagents, Image, Voice, Web und Model-Refresh.

## Decisions

### UI
- Design-Vorlage: `harness-pane`, `harness-subpane`, `harness-stack`, `workbench-plain-input`, Footer wie Workspace (`workbench-mini-btn--primary` + `LuSave` + `BtnSave`).
- **Kein Save pro Zeile**; optional „Key entfernen" pro Zeile → Draft, Ausführung mit globalem Save.
- **Draft-UX**: Discard-Button neben Save; Verlassen-Warnung bei Dirty-State (Pane-Wechsel / Tab-Schließen).

### Backend
- Batch-IPC: `api_keys_apply` (setzt/löscht Keys in einem Aufruf).
- Zentrale Resolve-Funktion: `api_keys::resolve(provider) -> Option<String>` (intern genutzt) plus `provider_key_pub` für IPC-Konsumenten.
- **Env-Precedence**: Store gewinnt. Env (z. B. `BLX_ANTHROPIC_API_KEY`) ist nur Fallback, wenn Store leer ist. UI zeigt „via env" als Hinweis, wenn Fallback aktiv.
- **Migration (one-shot, beim Start)**: Bestehende Pro-Provider-Eintr. aus dem alten `agent_api_key_set`-Store werden in den zentralen Katalog übernommen; alter Store wird danach geleert. Idempotent — mehrfacher Start überschreibt nichts.
- **Alte IPCs entfernen (selber PR)**: `agent_api_key_set`, `agent_api_key_delete` + Bridge-Wrapper raus. Kein Deprecate-Shim.

### Image / Voice / Web
- Key-Felder vollständig aus `image_settings`, Voice-Settings und Web-Settings entfernen (UI **und** Backend-Struct).
- Runtime liest ausschließlich über `api_keys::resolve` mit den **reused** Provider-IDs (OpenAI / OpenRouter / …).
- Fehlermeldungen verweisen auf **Settings → API Keys**, nicht mehr auf „Image-Einstellungen" o. ä.

### Cursor-Plan (inline)
> Folgende Abschnitte aus dem ursprünglichen Cursor-Plan müssen hier eingetragen werden, damit dieser Plan eigenständig review-/umsetzbar ist:
- **Keyring-Strategie**: OS-Keyring (Linux: secret-service, macOS: Keychain, Windows: Credential Manager) vs. Plaintext-Datei — Fallback-Policy, Speicherort, Verschlüsselung. _(TODO: Inhalte aus Cursor-Plan einfügen)_
- **Pfade**: Konkreter Pfad des zentralen Katalogs (z. B. `~/.config/blxcode/api_keys.json` oder Keyring-Eintragsname). _(TODO)_
- **Env-Vars**: Vollständige Liste der respektierten Env-Vars pro Provider. _(TODO)_
- **Coming-soon-Provider**: Welche Provider erscheinen als deaktivierte Zeilen im UI? _(TODO)_

## Runtime (Review)

**Heute**: Agent / Subagent / Image / Voice / Web haben jeweils eigenen Key-Pfad. Image-Fehler zeigt irreführend „Image-Einstellungen". Subagent macht separaten Lookup.

**Ziel**: Agent, Subagents (gleicher Turn-Key), Image / Voice (Reuse OpenAI/OpenRouter-IDs), Web, Model-Refresh → **alle** über `api_keys::resolve` / `provider_key_pub`. Subagents ohne separaten Lookup. Image-Fehler auf API Keys umgestellt.

## Tasks (in Ausführungsreihenfolge)

1. [x] **`api-keys-backend`** — Katalog-Struct, Storage (Keyring/Datei), `api_keys_apply`, `api_keys::resolve`, One-shot-Migration aus altem `agent_api_key_set`-Store, Env-Fallback-Logik.
2. [x] **`api-keys-bridge`** — `tauri_bridge.rs`: `api_keys_status` / `api_keys_apply`; alte per-provider key commands entfernt.
3. [x] **`settings-scaffold`** — Docked center settings tab + sidebar categories (`harness_ui.rs` / `SettingsDock`).
4. [x] **`api-keys-ui`** — `api_keys_pane/`: Save/Discard footer, draft state, per-row remove, „via env" hint, brand icons.
5. [x] **`runtime-wiring`** — Agent/Subagent/Image/Voice/Web/Model-Refresh über zentralen resolve; Image/Voice ohne eigene Key-Felder.
6. [x] **`agent-pane-trim`** — BLXCode Agent: nur Status-Hinweis → API Keys (Text/Image/Voice).
7. [x] **`i18n-docs`** — Locales + user/developer docs + CHANGELOG (PR #13 branch).

## Acceptance Criteria

- [x] Subagent läuft mit zentral gesetztem Key (kein separater Lookup-Pfad).
- [x] Image-Fehlermeldung verweist auf **Settings → API Keys** (nicht mehr „Image-Einstellungen").
- [x] Agent-Pane enthält kein Key-Eingabefeld mehr.
- [x] Migration-Smoke: Vorhandene Pro-Provider-Keys (alter Store) sind nach erstem Start im zentralen Katalog lesbar; alter Store geleert.
- [x] `agent_api_key_set` / `agent_api_key_delete` (Backend-Command + Bridge-Wrapper) sind aus dem Repo entfernt.
- [x] Env-Fallback: Bei leerem Store-Eintrag wird `BLX_*` env gelesen; UI zeigt „via env".
- [x] Discard-Button verwirft Draft; Dirty-State im Footer sichtbar.
