# MCP-Server-Support (Registry + In-App-Agent + Terminal-CLI-Injektion)

> Status: **planned** (noch nicht umgesetzt; Recherche zu CLI-Formaten
> abgeschlossen, siehe Abschnitt „Terminal-CLI-Injektion").

## Summary

Der User soll **MCP-Server** (Model Context Protocol) in BLXCode zentral
**eintragen / registrieren / bearbeiten / entfernen** können. Die registrierten
Server werden an **zwei Konsumenten** weitergegeben:

1. **In-App-Agent** (HTTP-Provider-Pfad: `anthropic.rs` / `openrouter.rs`) —
   BLXCode startet pro registriertem MCP-Server einen MCP-Client, entdeckt
   dessen Tools und mappt sie in den bestehenden Tool-Loop, sodass der Agent
   sie wie Harness-Tools aufrufen kann.
2. **Terminal-CLI-Agents** (claude, codex, gemini, opencode, cursor) — BLXCode
   schreibt beim Start/Launch jedes CLI dessen **nativen MCP-Config in das
   Workspace-Root** (projekt-scoped), übersetzt aus der zentralen Registry, so
   dass die mitgelieferten CLIs die gleichen Server sehen.

Neuer Settings-Tab **„MCP"** in der Settings-Sidebar mit Listen-/CRUD-UI. Der
User wird deutlich darauf hingewiesen, dass die **Chat-Session zurückgesetzt
werden muss** (`agent_clear_conversation`), damit der In-App-Agent die neue
Server-Liste lädt. Alle neuen UI-Strings sind **i18n** (Eintrag in **jeder**
`locales/*.rs`).

## Decisions

- **Zentrale Registry als Single Source of Truth.** Eine JSON-Datei pro
  Installation unter `{app_data_dir}/mcp/servers.json` (analog zu
  `agent_settings`/`tasks`). Nicht workspace-committed, da Secrets (Env/Headers)
  enthalten sein können. Globaler Scope (gilt für alle Workspaces); Per-Workspace
  später als optionaler Override möglich (out of scope für v1).
- **Transport-Typen v1:** `stdio` (command + args + env) und `http`/`sse`
  (url + headers). Das deckt die gemeinsame Schnittmenge aller fünf
  mitgelieferten CLIs ab.
- **In-App-MCP-Client via offizielles Rust-SDK** (`rmcp`, das offizielle
  modelcontextprotocol/rust-sdk Crate) statt Eigenbau. Falls Abhängigkeit zu
  schwer: minimaler eigener stdio-JSON-RPC-Client (Tool `list` + `call`). Siehe
  „Offene Entscheidungen".
- **Terminal-CLIs bekommen projekt-scoped Config-Dateien ins Workspace-Root
  geschrieben** (nicht globale User-Configs überschreiben), damit BLXCode keine
  fremden globalen Configs zerstört. Geschrieben **beim Launch** des jeweiligen
  CLI über die bestehende Launch-Pipeline (`terminal_agent_profiles.rs` /
  Terminal-Glue). Dateien werden klar als BLXCode-managed markiert (Kommentar /
  `_generated_by`), und ein Merge-Modus bewahrt existierende User-Einträge.
- **Session-Reset-Hinweis** ist Pflicht-UI: Banner im MCP-Tab + Toast nach
  Save, weil der In-App-Agent die Server beim Turn-Start (nicht live) lädt.
- **Secrets** (Env-Werte, Auth-Header) werden in der Registry gespeichert; für
  v1 im Klartext in `servers.json` (wie bereits andere lokale Configs).
  Optionale Verlagerung in den OS-Keyring ist ein Folge-Task.

## Ist-Zustand (verifiziert)

**Backend (`src-tauri/src/`)**

- In-App-Agent-Tool-Loop: `anthropic.rs` baut `tools_json` via
  `tools::render_for_anthropic()` ([anthropic.rs:139](../../src-tauri/src/agent/anthropic.rs#L139))
  und dispatcht über `dispatch_tool` aus
  `agent/tool_dispatch.rs` ([anthropic.rs:16](../../src-tauri/src/agent/anthropic.rs#L16)).
  `openrouter.rs` spiegelt das. **Das ist der zentrale Integrationspunkt** für
  In-App-MCP-Tools: Tool-Definitionen ergänzen + Dispatch-Routing für
  `mcp.<server>.<tool>`-Namen.
- Settings-Persistenz-Muster: `agent_settings.rs` lädt/speichert atomar
  (`load_settings`/`save_settings`, [agent_settings.rs:438](../../src-tauri/src/agent_settings.rs#L438)).
  Neues `mcp_registry.rs` folgt demselben Muster.
- Pfade: `app_paths.rs` (`app_data_dir()`, `workspace_hash`,
  [app_paths.rs:96](../../src-tauri/src/app_paths.rs#L96)) — Vorlage für
  `mcp_servers_path()`.
- Command-Registrierung in `lib.rs` (alle Tauri-Commands).
- Terminal-CLI-Profile: `TERMINAL_AGENT_PROFILES`
  ([src/workbench/terminal_agent_profiles.rs](../../src/workbench/terminal_agent_profiles.rs))
  — fünf Agents: `claude`, `codex`, `gemini`, `opencode`, `cursor`. Launch-/
  Resume-Metadaten leben hier; die Profil-Struct ist der Ort, um pro CLI das
  MCP-Config-Format zu hinterlegen.

**Frontend (`src/`)**

- Settings-Sidebar/Dock: `harness_ui.rs` `SettingsDock`
  ([harness_ui.rs:900](../../src/workbench/harness_ui.rs#L900)) rendert die
  Kategorie-Buttons (`HarnessCatBtn`) und das `match` auf
  `HarnessSettingsCategory` ([state.rs:790](../../src/workbench/state.rs#L790)).
  Icon-Mapping in `harness_settings_cat_icon`
  ([harness_ui.rs:886](../../src/workbench/harness_ui.rs#L886)).
- Pane-Vorlage: `api_keys_pane/` (Draft-Modell, Footer Save/Discard,
  `beforeunload`-Dirty-Guard) — **ideale Blaupause** für die MCP-CRUD-Pane.
- IPC-Wrapper: `tauri_bridge.rs`. Shared Serde-Typen: `agent_wire.rs`.
- i18n: `I18nKey` in `i18n/keys.rs`; je String ein Arm in **jeder**
  `i18n/locales/*.rs` (Compile-Zeit-Exhaustiveness). Nicht-Englisch über
  `scripts/render_i18n_locales_from_en.py` nachziehen.

## Terminal-CLI-Injektion — recherchierte Config-Formate

Ziel: Für jeden mitgelieferten CLI-Agent die registrierten Server in dessen
**nativem, projekt-scoped** Format ins Workspace-Root schreiben. Belege:

| CLI | Projekt-Config (geschrieben ins Workspace-Root) | Format | Schlüssel |
|---|---|---|---|
| **claude** | `.mcp.json` | JSON | `mcpServers` (command/args/env \| `type:"http"` url/headers). Alternativ CLI `claude mcp add-json <name> '<json>'`. |
| **codex** | `.codex/config.toml` | **TOML** | `[mcp_servers.<name>]` command/args/env |
| **gemini** | `.gemini/settings.json` | JSON | `mcpServers` |
| **opencode** | `opencode.json` | JSON | `mcp.<name>` mit `type:"local"` (command-array) bzw. `type:"remote"` (url), `enabled` |
| **cursor** | `.cursor/mcp.json` | JSON | `mcpServers` |

Quellen:
[Claude Code MCP Docs](https://code.claude.com/docs/en/mcp),
[Codex MCP](https://developers.openai.com/codex/mcp),
[Gemini CLI MCP](https://geminicli.com/docs/tools/mcp-server/),
[OpenCode MCP](https://opencode.ai/docs/mcp-servers/),
[Cursor MCP](https://www.truefoundry.com/blog/mcp-servers-in-cursor-setup-configuration-and-security-guide).

**Strategie:** Pro CLI eine kleine Render-Funktion `render_mcp_config_for(slug,
&[McpServer]) -> (relativer_pfad, dateiinhalt)`. Beim Launch eines Terminal-
Agents (bestehende Launch-Pipeline) schreibt BLXCode die Datei ins
Workspace-Root, **merge-sicher**: existierende, nicht von BLXCode stammende
Einträge bleiben erhalten; BLXCode-managed Einträge werden ersetzt (Markierung
über einen reservierten Kommentar bzw. `x-blxcode-managed: true`-Feld, wo das
Format Zusatzfelder erlaubt). Beim Format ohne Kommentare (reines JSON) wird ein
sidecar-Manifest `.blxcode/mcp-managed.json` zur Nachverfolgung der von uns
verwalteten Keys geführt.

## Implementation Notes

### Backend

1. **Datenmodell** (`src-tauri/src/mcp/registry.rs`, neu — kein Monolith,
   eigenes Modul-Verzeichnis `mcp/`):
   - `McpTransport { Stdio { command, args, env: Map }, Http { url, headers: Map } }`
   - `McpServer { id, name, enabled, transport, description }`
   - `McpRegistry { servers: Vec<McpServer> }`
   - `load_registry()/save_registry()` atomar; Pfad `mcp_servers_path()` in
     `app_paths.rs` = `{app_data_dir}/mcp/servers.json`.
2. **Tauri-Commands** (in `lib.rs` registrieren):
   - `mcp_list() -> Vec<McpServer>`
   - `mcp_upsert(server) -> McpServer` (validiert name/transport, vergibt id)
   - `mcp_remove(id)`
   - `mcp_test(id) -> McpTestResult` (Verbindungstest: stdio spawnen bzw.
     HTTP-Handshake, `initialize` + `tools/list`, Tool-Anzahl zurück).
3. **In-App-MCP-Client** (`src-tauri/src/mcp/client.rs`):
   - Pro aktivem Server beim Turn-Start (in `session_orchestrator.rs`
     `dispatch_user_turn`) einen MCP-Client-Handle aufbauen, `tools/list`
     cachen. Lifecycle: Clients leben für die Dauer der Session; bei
     `agent_clear_conversation` neu aufbauen (deshalb der Reset-Hinweis).
   - Tool-Namespacing: `mcp.<server-id>.<tool-name>` → kollidiert nicht mit
     Harness-Tools. Anthropic-Name-Sanitizing (`to_anthropic_name`) greift schon
     für Punkte ([anthropic.rs:33](../../src-tauri/src/agent/anthropic.rs#L33)).
   - **Tool-Definitionen einspeisen:** `tools_json` in `anthropic.rs`/
     `openrouter.rs` um die MCP-Tools erweitern (JSON-Schema kommt aus
     `tools/list`).
   - **Dispatch-Routing:** in `tool_dispatch.rs` MCP-Namen erkennen und an den
     passenden Client `tools/call` weiterreichen, Ergebnis als `ToolResult`
     zurück in den Loop.
4. **Terminal-CLI-Renderer** (`src-tauri/src/mcp/cli_export.rs`):
   - `render_mcp_config_for(slug, &[McpServer]) -> Vec<(PathBuf, String)>`
     (claude/codex/gemini/opencode/cursor).
   - Merge-Logik + `.blxcode/mcp-managed.json`-Manifest.
   - Aufruf aus der Terminal-Launch-Pipeline (dort, wo `terminal_agent_profile`
     den Launch-Command baut), gated über ein Setting „Inject MCP into terminal
     CLIs" (default an).

### Frontend

5. **Neue Settings-Kategorie** `HarnessSettingsCategory::Mcp`
   ([state.rs:790](../../src/workbench/state.rs#L790)); Button in `SettingsDock`
   ([harness_ui.rs:918](../../src/workbench/harness_ui.rs#L918)); Icon
   (`LuPlug`/`LuServerCog`) in `harness_settings_cat_icon`; `match`-Arm rendert
   `<McpSettingsPane/>`.
6. **`src/workbench/mcp_settings_pane/`** (eigenes Verzeichnis, Reusable-
   Components-Regel): Liste registrierter Server (Name, Transport-Badge,
   enabled-Toggle, Tool-Count nach Test), Buttons **Hinzufügen / Bearbeiten /
   Entfernen**, Add/Edit-Dialog (Name, Transport-Typ-Switch, command/args/env
   bzw. url/headers Key-Value-Editoren), **Test-Button** pro Server. Draft-/
   Save-Modell + `beforeunload`-Guard analog `api_keys_pane`.
   - **Pflicht-Reset-Banner**: „Änderungen wirken im Chat erst nach Session-
     Reset" + direkter „Reset session"-Button (ruft `agent_clear_conversation`,
     disabled während `busy`).
7. **IPC-Wrapper** in `tauri_bridge.rs` (`mcp_list/upsert/remove/test`); Shared-
   Typen in `agent_wire.rs` spiegeln das Backend-Modell.
8. **i18n**: alle neuen Keys in `i18n/keys.rs` + `en_us.rs`, dann
   `scripts/render_i18n_locales_from_en.py` für die übrigen Sprachen.

## Tests

- **Backend Unit:** `registry.rs` Roundtrip (load/save), id-Vergabe, Validierung;
  `cli_export.rs` Snapshot-Tests pro CLI (korrektes JSON/TOML, Merge bewahrt
  Fremd-Einträge); Namespacing/Sanitizing der Tool-Namen.
- **Backend Integration:** `mcp_test` gegen einen Mock-stdio-MCP-Server
  (Echo-Server mit `initialize` + `tools/list` + `tools/call`).
- **In-App-Loop:** Dispatch eines `mcp.*`-Tool-Calls routet zum richtigen Client
  und liefert `ToolResult` (mit Mock-Client).
- **Frontend:** `cargo check -p blxcode-ui --target wasm32-unknown-unknown` grün
  (i18n-Exhaustiveness erzwingt alle Locales).
- `cargo test --workspace` grün.
- **Manuell (`cargo tauri dev`):** Server anlegen/bearbeiten/löschen; Test-Button;
  Reset-Banner; In-App-Agent ruft ein MCP-Tool auf; je ein Terminal-CLI starten
  und verifizieren, dass die Projekt-Config korrekt geschrieben wurde und das CLI
  den Server sieht.

## Offene Entscheidungen

- **Rust-MCP-SDK (`rmcp`) vs. minimaler Eigenbau** für den In-App-Client —
  Abwägung Dependency-Gewicht vs. Wartung. Empfehlung: `rmcp`, Fallback Eigenbau
  (stdio JSON-RPC: `initialize`/`tools/list`/`tools/call`).
- **Secret-Handling**: v1 Klartext in `servers.json`; OS-Keyring als Folge-Task.
- **Per-Workspace-Override** der globalen Registry — v2.
- **CLIs ohne sauberen projekt-scoped Merge** (reines JSON ohne Kommentare):
  Bestätigung, dass das Sidecar-Manifest `.blxcode/mcp-managed.json` der
  akzeptierte Weg ist.

## Tasks

- [ ] `mcp-registry-model` - Backend `mcp/registry.rs`: Modell + atomar load/save + `mcp_servers_path()`
- [ ] `mcp-crud-commands` - Tauri-Commands `mcp_list/upsert/remove` + Registrierung in `lib.rs`
- [ ] `mcp-client-core` - In-App-MCP-Client (`mcp/client.rs`): initialize + tools/list + tools/call (stdio + http)
- [ ] `mcp-test-command` - `mcp_test` Verbindungstest + Tool-Count
- [ ] `mcp-inapp-tool-injection` - MCP-Tools in `tools_json` (anthropic+openrouter) einspeisen, Namespacing `mcp.<srv>.<tool>`
- [ ] `mcp-inapp-dispatch` - `tool_dispatch.rs` routet `mcp.*`-Calls an den Client und liefert ToolResult
- [ ] `mcp-client-lifecycle` - Client-Aufbau im `session_orchestrator` beim Turn-Start, Neuaufbau bei `agent_clear_conversation`
- [ ] `mcp-cli-export-renderers` - `mcp/cli_export.rs`: Renderer für claude/codex/gemini/opencode/cursor (JSON/TOML)
- [ ] `mcp-cli-export-merge` - Merge-sicheres Schreiben + `.blxcode/mcp-managed.json`-Manifest
- [ ] `mcp-cli-launch-hook` - Terminal-Launch-Pipeline schreibt Projekt-Configs vor CLI-Start (gated per Setting)
- [ ] `mcp-settings-category` - `HarnessSettingsCategory::Mcp` + SettingsDock-Button + Icon + match-Arm
- [ ] `mcp-settings-pane` - `mcp_settings_pane/`: Liste + Add/Edit/Remove-Dialog + Test-Button (Draft/Save wie api_keys_pane)
- [ ] `mcp-reset-banner` - Pflicht-Hinweis-Banner „Session-Reset nötig" + Reset-Button (disabled während busy)
- [ ] `mcp-ipc-bridge` - `tauri_bridge.rs` Wrapper + `agent_wire.rs` Shared-Typen
- [ ] `mcp-i18n` - Neue I18nKeys in `keys.rs` + `en_us.rs`, übrige Locales via Script
- [ ] `mcp-tests` - Unit/Integration/Snapshot-Tests (registry, cli_export, dispatch, mock-server)
