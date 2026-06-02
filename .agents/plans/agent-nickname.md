# Persönlicher Agent-Name ("Nickname") mit Badword-Filter

> Status: **planned**

## Summary

Der User soll dem Agenten in **Settings → Agent** einen persönlichen Namen
geben können. Ist keiner gesetzt, gilt der Default **`BLXCodey`**. Der Name wird
gegen eine **fest eingebaute, nicht änderbare Badword-Liste** geprüft (aus einer
aktuellen Netz-Quelle generiert). Der gewählte Name fließt in den
**Systemprompt**, damit der Agent ihn kennt — und zwar identisch für **Text-
und Voice-Pfad** (beide laufen über denselben Dispatch → denselben Prompt).
Alle neuen UI-Strings sind **i18n**.

## Kontext / Ist-Zustand (verifiziert)

**Backend (`src-tauri/`)**

- Settings-Modell: `AgentProviderSettings`
  ([agent_settings.rs:130](../../src-tauri/src/agent_settings.rs#L130)),
  persistiert atomar in `SETTINGS_FILE` im `app_config_dir`. Es gibt parallel:
  - `AgentProviderSettingsPatch`
    ([agent_settings.rs:201](../../src-tauri/src/agent_settings.rs#L201)) — Eingabe für Save.
  - `AgentProviderSettingsView`
    ([agent_settings.rs:182](../../src-tauri/src/agent_settings.rs#L182)) — `#[serde(flatten)]`
    der Settings + `key_statuses`.
  - Save-Command `agent_settings_save`
    ([agent_settings.rs:889](../../src-tauri/src/agent_settings.rs#L889)) kopiert die
    Patch-Felder ins geladene Settings-Objekt und schreibt.
- **Systemprompt:** `system_prompt(workspace_root: Option<&str>) -> String`
  ([system_prompt.rs:8](../../src-tauri/src/agent/system_prompt.rs#L8)) — *einzige*
  Quelle für alle HTTP-Provider. Aufrufer:
  - [openrouter.rs:210](../../src-tauri/src/agent/openrouter.rs#L210) `system_prompt(workspace_string.as_deref())`
  - [anthropic.rs:115](../../src-tauri/src/agent/anthropic.rs#L115) `system_prompt(workspace_string.as_deref())`

  Beide haben das geladene `settings`-Objekt bereits im Scope (via
  `DispatchContext`/`dispatch_user_turn`,
  [session_orchestrator.rs:37](../../src-tauri/src/agent/session_orchestrator.rs#L37)).
- **Voice == Text:** Der Voice-Pfad ist STT → Text → `submit_turn` →
  `dispatch_user_turn` → derselbe Provider-Call → **derselbe `system_prompt`**.
  Es gibt **keinen separaten Voice-Systemprompt**. ⇒ Ein einziger Name deckt
  beide ab (Anforderung „beide sollten denselben haben" = ja, automatisch).
- Subagents nutzen einen eigenen Prompt
  ([subagent_prompts.rs:141](../../src-tauri/src/agent/subagent_prompts.rs#L141)) —
  sie behalten ihre Rollen-Identität; der Nickname betrifft den **Hauptagenten**
  (Subagent-Erwähnung optional, siehe Out-of-Scope).

**Frontend (`src/`)**

- Settings-„Agent"-Pane: `AgentProviderPane`
  ([agent_provider_pane/mod.rs:436](../../src/workbench/agent_provider_pane/mod.rs#L436)).
  Felder als `RwSignal` (provider, custom_model, thinking_level,
  tool_loop_limit, auto_compact_*, orb_mode), Dirty-Tracking via
  `AgentSettingsBaseline`, Save über `agent_settings_save(...)`
  ([mod.rs:594](../../src/workbench/agent_provider_pane/mod.rs#L594)), danach
  `dispatch_agent_settings_changed()`.
- IPC-Mirror: `AgentProviderSettingsView`
  ([tauri_bridge.rs:430](../../src/tauri_bridge.rs#L430)) und der `agent_settings_save`-
  Wrapper ([tauri_bridge.rs:553](../../src/tauri_bridge.rs#L553)) müssen Feld-parallel
  zum Backend bleiben.
- i18n: Keys in [src/i18n/keys.rs](../../src/i18n/keys.rs), EN-Quelle
  [en_us.rs](../../src/i18n/locales/en_us.rs), übrige 12 Locales via
  `scripts/tools/render_i18n_locales_from_en.py` (Compile-Time-Exhaustiveness:
  neuer Key ⇒ Eintrag in **jeder** `locales/*.rs`). Pane nutzt bereits
  `AgProvider*`, `AgThinking*`, `AgSaveProviderDone`.

## Design-Entscheidungen

1. **Speicherung:** Neues Feld `agent_nickname: String` in
   `AgentProviderSettings` (`#[serde(default)]`, leer = „nicht gesetzt"). Ein
   Helfer `resolve_agent_name(&settings) -> String` liefert `BLXCodey`, wenn
   leer/whitespace. Konstante `DEFAULT_AGENT_NICKNAME = "BLXCodey"`. Leer
   speichern statt „BLXCodey" hartzucodieren hält den Default an *einer* Stelle.
2. **Badword-Liste (fest, nicht user-änderbar):** Generiert aus der gepflegten
   Quelle **LDNOOBW** („List of Dirty, Naughty, Obscene and Otherwise Bad
   Words", `github.com/LDNOOBW/List-of-Dirty-Naughty-Obscene-and-Otherwise-Bad-Words`,
   Sprachlisten `en` + `de`). Ein Script
   `scripts/tools/gen_badwords.py` lädt beide Listen, dedupliziert, lowercased
   und rendert eine Rust-Quelle `src-tauri/src/agent/badwords.rs` mit
   `pub const BADWORDS: &[&str] = &[ ... ];` (eingecheckt; Liste damit
   reproduzierbar, offline und **nicht zur Laufzeit änderbar**). Lizenz: LDNOOBW
   ist CC-BY-4.0 → Quelle/Attribution als Datei-Header eintragen.
3. **Validierung (Backend = Source of Truth):**
   `validate_nickname(name) -> Result<String, NicknameError>` mit
   `NicknameError { Empty, TooLong, InvalidChars, BadWord }`:
   - trim; leer ist **erlaubt** (= Default `BLXCodey`) → kein Fehler, Save
     speichert leer.
   - Länge ≤ 32 Zeichen (`TooLong`).
   - erlaubte Zeichen: Buchstaben (inkl. Unicode), Ziffern, Space, `-`, `_`
     (`InvalidChars`).
   - normalisierte Form (lowercase, simple Leetspeak-Map `4→a 3→e 1→i 0→o 5→s
     @→a $→s`, Nicht-Alphanum entfernt) gegen `BADWORDS` per **Whole-Word- und
     Substring-Match** prüfen (`BadWord`).
   - Aufgerufen **in `agent_settings_save`** (harte Ablehnung) **und** über ein
     neues, leichtgewichtiges Command `agent_validate_nickname(name)` für
     Live-Feedback im UI ohne Speichern.
4. **i18n der Fehler:** Command/Backend gibt einen **stabilen Reason-Code**
   zurück (z. B. `"badWord"`), das Frontend mappt Code → `I18nKey`. So bleiben
   übersetzbare Texte im Frontend.
5. **Systemprompt-Threading:** Signatur →
   `system_prompt(workspace_root: Option<&str>, agent_name: &str)`. Ganz oben
   ergänzen, z. B.: *"Your name is {agent_name}. The user may address you by
   this name; acknowledge it naturally when they do. It does not change your
   role, scope, or these rules."* Aufrufer in `openrouter.rs`/`anthropic.rs`
   übergeben `resolve_agent_name(&settings)`.
6. **Name-Badge am Orb:** Der aufgelöste Name wird als dezente, „glasige"
   Badge **über/am oberen Rand des Orbs** angezeigt (siehe Skizze: orange
   markierter Bereich oben am Orb). Optik ausschließlich aus Theme-Tokens +
   `backdrop-filter: blur(...)`. Reaktiver Name-Signal im Agent-Panel, der bei
   `agent-settings-changed` neu lädt, sodass die Badge nach dem Speichern sofort
   den neuen Namen zeigt.

## Umsetzung — Tasks

- **NICK-01 — Badword-Generator + eingecheckte Liste**
  `scripts/tools/gen_badwords.py` (Download en+de von LDNOOBW, dedup/lowercase,
  Rust-Render) → `src-tauri/src/agent/badwords.rs` (`pub const BADWORDS`,
  Header mit Quelle + CC-BY-4.0-Attribution). `mod badwords;` registrieren.

- **NICK-02 — Validierung**
  `agent/nickname.rs` (oder in `agent_settings.rs`): `DEFAULT_AGENT_NICKNAME`,
  `resolve_agent_name`, `NicknameError` + `reason_code()`, `validate_nickname`
  (Normalisierung inkl. Leetspeak, Whole-Word/Substring). Unit-Tests:
  Default-leer ok, gängiges Badword (en+de) blockt, Leetspeak-Variante blockt,
  Längen-/Zeichen-Grenzen, harmloser Name ok.

- **NICK-03 — Settings-Modell + Persistenz**
  `agent_nickname: String` in `AgentProviderSettings` (+ `Default`),
  `AgentProviderSettingsPatch`, `AgentProviderSettingsView`. In
  `agent_settings_save`: `validate_nickname` → bei Fehler `Err(reason_code)`,
  sonst `settings.agent_nickname` setzen. Neues Command
  `agent_validate_nickname` + Registrierung in
  [lib.rs](../../src-tauri/src/lib.rs) (`invoke_handler`).

- **NICK-04 — Systemprompt**
  `system_prompt`-Signatur um `agent_name: &str` erweitern, Namens-Absatz
  ergänzen. Aufrufe in [openrouter.rs:210](../../src-tauri/src/agent/openrouter.rs#L210)
  und [anthropic.rs:115](../../src-tauri/src/agent/anthropic.rs#L115) auf
  `resolve_agent_name(&settings)` umstellen. Bestehende `system_prompt`-Tests
  anpassen + neuer Test: Prompt enthält gesetzten Namen / Default.

- **NICK-05 — IPC-Mirror (Frontend-Bridge)**
  `agent_nickname` in `tauri_bridge::AgentProviderSettingsView`
  ([tauri_bridge.rs:430](../../src/tauri_bridge.rs#L430)) ergänzen; `agent_settings_save`-
  Wrapper ([tauri_bridge.rs:553](../../src/tauri_bridge.rs#L553)) um den Parameter
  erweitern; Wrapper `agent_validate_nickname(name) -> Result<(), String>`
  (liefert Reason-Code als Err).

- **NICK-06 — UI im Agent-Pane**
  In `AgentProviderPane`: `nickname` `RwSignal<String>`, Textfeld mit Label,
  Placeholder (`BLXCodey`) und Hilfetext. In `AgentSettingsBaseline`,
  Dirty-Check, `snapshot_baseline`, `apply_settings`, `save` einhängen
  ([mod.rs:572](../../src/workbench/agent_provider_pane/mod.rs#L572)). Live-Validierung
  (debounced `agent_validate_nickname` on:input) → Inline-Fehler über
  Reason-Code→`I18nKey`; Save bei Fehler blockieren.

- **NICK-07 — i18n**
  Neue Keys in [keys.rs](../../src/i18n/keys.rs) + EN in
  [en_us.rs](../../src/i18n/locales/en_us.rs): `AgNicknameLabel`,
  `AgNicknamePlaceholder`, `AgNicknameHelp`, `AgNicknameErrTooLong`,
  `AgNicknameErrInvalidChars`, `AgNicknameErrBadWord`, `AgNameBadgeAria`
  (Badge-`aria-label`, z. B. „Agent name: {name}"). Übrige Locales via
  `python3 scripts/tools/render_i18n_locales_from_en.py`.

- **NICK-08 — Name-Badge am Orb (UI + CSS)**
  Reaktiver `agent_name`-Signal im Agent-Panel
  ([agent_panel/mod.rs](../../src/workbench/agent_panel/mod.rs)) — initial via
  `agent_settings_get()` (analog `model_label`,
  [mod.rs:133](../../src/workbench/agent_panel/mod.rs#L133)) **und** Re-Load über
  einen `agent-settings-changed`-Listener, damit die Badge nach Save sofort
  aktualisiert (aktuell wird `model_label` nur einmalig gesetzt — Listener neu
  hinzufügen). Badge-Markup über dem Orb (Hero ist Positionierungs-Kontext;
  Element als Sibling der `VoiceOrb`/`.agent-hero__orb` oder im `VoiceOrb`-
  Fragment, [voice_orb/mod.rs:287](../../src/workbench/agent_panel/voice_orb/mod.rs#L287)).
  Neue CSS-Klasse `.agent-name-badge` in [styles.css](../../styles.css):
  `position: absolute` am oberen Orb-Rand, zentriert; Hintergrund
  `color-mix(... var(--accent) ...)`/`var(--overlay-*)`,
  `border: 1px solid var(--border)`, `backdrop-filter: blur(...)` (analog
  bestehender Blur-Regeln), `border-radius: var(--radius-pill)`,
  Theme-Token-Text, Ellipsis bei langen Namen. Im `--compact`-Hero-Zustand
  ([styles.css:3063](../../styles.css#L3063)) skaliert/positioniert sich die Badge
  passend. **Live-State:** Bei Thinking/Running bekommt die Badge einen
  dezenten Akzent-Puls/Glow (gleiche Logik wie `agent-session-stats__state--live`
  / die `busy`/`active_thinking`-Signale) — Modifier-Klasse
  `.agent-name-badge--live` mit sanfter `@keyframes`-Animation aus Theme-Tokens.

- **NICK-09 — Verifikation**
  `cargo test --workspace` (Validierungs- + Prompt-Tests),
  `cargo check -p blxcode-ui --target wasm32-unknown-unknown`,
  `bash scripts/lint_theme_tokens.sh` (Badge nutzt nur Tokens). Manuell in
  `cargo tauri dev`: Name setzen/leeren, Badword wird abgelehnt (UI-Fehler),
  Default `BLXCodey` greift bei leer, Badge zeigt den Namen am Orb und
  aktualisiert nach Save live, Agent reagiert im Text **und** Voice auf den
  Namen.

## Out of Scope

- Pro-Workspace-Namen (Nickname ist global wie die übrigen Agent-Settings).
- Subagent-Identität: Subagents behalten ihre Rolle; optional könnte
  `subagent_system_prompt` den Hauptagenten-Namen erwähnen — **nicht** in v1.
- Umbenennung des **Chat-Author-Labels** (`AgAssistant`) bzw. der Hero-Brand
  von „Agent" auf den Namen: hier nur die **Orb-Badge** (NICK-08), nicht jeder
  Vorkommen im Chat-Verlauf.
- TTS-Begrüßung mit Namen.

## Betroffene Dateien

- `src-tauri/src/agent/badwords.rs` *(neu, generiert)*,
  `scripts/tools/gen_badwords.py` *(neu)*.
- `src-tauri/src/agent/nickname.rs` *(neu)* bzw. Erweiterung
  `src-tauri/src/agent_settings.rs`.
- `src-tauri/src/agent/system_prompt.rs`, `.../openrouter.rs`, `.../anthropic.rs`.
- `src-tauri/src/lib.rs` (Command-Registrierung + `mod`).
- `src/tauri_bridge.rs`, `src/workbench/agent_provider_pane/mod.rs`.
- `src/workbench/agent_panel/mod.rs` (Name-Signal + `agent-settings-changed`-
  Listener), `src/workbench/agent_panel/voice_orb/mod.rs` (Badge-Markup),
  `styles.css` (`.agent-name-badge`).
- `src/i18n/keys.rs`, `src/i18n/locales/*.rs`.
