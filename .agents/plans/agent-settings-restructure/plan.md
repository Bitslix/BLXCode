# Agent Settings Tab Restructure

## Summary

The Settings → **Agent** tab (`HarnessSettingsCategory::AgentProvider`) is today a single
1188-line monolith (`src/workbench/agent_provider_pane/mod.rs`) that crams provider, model,
thinking, web tools, image and voice settings into one ad-hoc 2-column grid plus a trailing
web-tools section. It is unsorted and hard to scan.

Rebuild the tab from scratch following the provided mockup, keeping **all existing settings**,
using global theme tokens (incl. `--radius-*` roundings), icons (`icondata` Lucide), and the
visual language of the other settings panes (`SettingsPaneHeader`, `.harness-subpane`,
`.harness-pane-subhead`, `.harness-field-label`). Split the monolith into focused modules
(no god-file) and extract the duplicated provider/level pickers into reusable components.

Target layout (top → bottom), each a bordered card (`--radius-xl`) with an icon subhead:

1. **Personality** — Name, 2D|3D orb toggle, Role, Intelligence, Gender, Voice grid
2. **Provider** — Chat, Image, STT, TTS (4 independent provider pickers)
3. **Models** — Chat, Image, STT, TTS (4 independent model pickers)
4. **Configuration** — sub-groups: Images, Audio, Misc, WebSearch

## Decisions

- **Audio Mood**: out of scope. No backend exists; do **not** add a placeholder. May become a
  separate future plan.
- **STT/TTS split**: STT and TTS get **independent** provider + model selectors (Provider/Models
  cards). The backend `VoiceSettings` already carries separate `stt.provider`/`stt.model_id` and
  `tts.provider`/`tts.model_id`; today's UI couples them — that coupling is removed.
- **Hard constraint — not PTT**: These STT/TTS settings are the *agent's own* speech config
  (dictation transcription + reply speech) and the reply **Voice/Gender** under Personality.
  They must stay fully decoupled from the Push-to-Talk config (`VoiceSettings.ptt`), which is
  owned by the separate Voice settings pane (`HarnessSettingsCategory::Voice`). Only ever read/
  write the `stt`/`tts` sub-objects here; never touch `ptt`.
- **WebSearch**: lay out per mockup (Brave + Tavily rows with toggles), but key **management
  stays in the API-Keys tab** — here we only show toggles + masked key status and a link/hint to
  the API-Keys tab. Provider selection stays mutually exclusive (existing
  `WebProviderKind` None/Tavily/Brave); no backend model change.
- **Save behavior**: **auto-save per section** (debounced), matching how Image/Voice already
  behave. Remove the global Save button and the `dirty`/`baseline` machinery. Nickname still
  validates (`agent_validate_nickname`) before its debounced save commits.
- **After Translation** (mockup, Misc) = `post_stt_flow` (`AutoSend` / `Draft`).
- **Audio record quality** (mockup, Audio) = `stt.sample_rate_hz` (16k/24k/48k).
- Existing settings not drawn in the mockup are **kept** and placed sensibly:
  - Provider-conditional fields (Ollama/LM-Studio local URL, Portkey base URL, Cloudflare
    account ID) → render conditionally inside the **Provider** card under the Chat provider row.
  - Per-provider API-key status + "manage keys" hint + "Refresh models" buttons → keep, attached
    to the relevant Provider/Models rows.
  - TTS autoplay-enabled toggle (`tts.enabled`) → **Configuration → Audio**.

## Implementation Notes

### Module layout (replaces the monolith)

New dir `src/workbench/agent_settings_pane/` (retire `agent_provider_pane/`):

- `mod.rs` — `AgentSettingsPane` orchestrator. Loads all four stores
  (`agent_settings_get`, `image_settings_get`, `voice_settings_get`, `agent_web_settings_get`)
  + `agent_session_roles_list` + `api_keys_status`, owns the shared `RwSignal`s, renders the four
  section components in order, wires debounced auto-save per store, dispatches
  `blxcode-agent-settings-changed` after agent-core saves.
- `personality.rs` — `PersonalitySection`: Name (`nickname` + validation/hint), 2D|3D
  (`orb_mode`, `blx-switch`), Role (`SessionRolePicker`), Intelligence (`ThinkingLevelPicker`),
  Gender (`GenderFilter` row), Voice (voice catalog grid w/ preview play).
- `providers.rs` — `ProvidersSection`: Chat (`ProviderPicker`) + conditional endpoint fields,
  Image (`ImageProviderPicker`), STT + TTS (`VoiceProviderPicker` ×2, independent), each with
  key-status row.
- `models.rs` — `ModelsSection`: Chat/Image/STT/TTS `AgentModelPicker`s + per-row Refresh +
  source hint.
- `configuration.rs` — `ConfigurationSection` with four labelled sub-blocks:
  - Images: Image Quality (`ImageQualityLevelPicker`).
  - Audio: record quality (sample-rate choices), TTS autoplay enabled toggle.
  - Misc: Tool Loop Maximum (`tool_loop_limit`), Compaction Level % (`auto_compact_enabled` +
    `auto_compact_threshold`), After Translation (`post_stt_flow`).
  - WebSearch: Brave/Tavily toggles (mutually exclusive `WebProviderKind`) + masked key status +
    link to API-Keys tab.
- `pickers/` (reusable, rule-reusable-components) — move `ProviderPicker`, `ThinkingLevelPicker`,
  `ImageProviderPicker`, `ImageQualityLevelPicker`, `VoiceProviderPicker` here, de-duplicated.
  The three provider pickers are near-identical; collapse to one generic picker if cleanly
  possible, otherwise keep three thin wrappers over a shared listbox primitive.
- `agent_settings_pane.css` — card sections styled like `.harness-subpane`
  (`border-radius: var(--radius-xl)`), inputs `--radius-md`, pills `--radius-pill`. **No
  hardcoded colors** — only semantic tokens (`--bg-raised`, `--border`, `--accent`, …) per
  rule-theme-tokens.

### Wiring

- `harness_ui.rs`: `HarnessSettingsCategory::AgentProvider => <AgentSettingsPane/>` (and the
  legacy `Image` arm → same). Keep the enum variant name to avoid churn; optionally relabel the
  nav button key `HsCatProvider` → an "Agent" string. Update `src/workbench/mod.rs` exports.
- Keep `ImagePane` (standalone image settings) working — it reuses `AgentImageColumn` today; after
  the split it should reuse the shared image picker components, not the old column.
- Reuse, don't duplicate: `SettingsPaneHeader`, `AgentModelPicker`, `SessionRolePicker`.

### i18n

- New `I18nKey`s for section headings (Personality, Provider, Models, Configuration) and
  sub-group labels (Images, Audio, Misc, WebSearch) + field labels not already present
  (Gender, Voice; Intelligence reuses `AgThinkingField`). Each new key needs a string in **every**
  `src/i18n/locales/*.rs` (compile-time exhaustiveness); seed non-English with
  `scripts/render_i18n_locales_from_en.py`.
- Replace the two hardcoded English strings in the current pane ("Default role for newly-created
  workspaces.", "Agent orb", "3D Drobo"/"2D logo", orb hint) with i18n keys.

## Tests

- `cargo check -p blxcode-ui --target wasm32-unknown-unknown` clean.
- `cargo test --workspace` (locale exhaustiveness + voice/image settings serde) green.
- Manual: each section auto-saves on change and survives a tab reopen (reload from store).
- Manual: changing STT provider/model does **not** alter TTS values and vice-versa; neither
  touches PTT settings in the Voice pane.
- Manual: provider-conditional fields (Ollama/LM-Studio URL, Portkey base URL, Cloudflare
  account) appear only for their provider and persist.
- Manual: nickname validation still blocks bad input; WebSearch toggle reflects/saves
  `web_provider` and key status mirrors the API-Keys tab.
- Manual: theme token roundings respond to the global `--radius-scale` appearance knob; verify in
  light + dark themes; no hardcoded colors (grep the new CSS).
- Manual: layout matches the mockup ordering and grouping; icons present on every subhead/field.

## Tasks

- [ ] `scaffold-module` - Create `agent_settings_pane/` module skeleton + CSS, wire into `harness_ui` and `mod.rs`, retire `agent_provider_pane`
- [ ] `extract-pickers` - Move provider/level pickers into reusable `pickers/`, de-duplicate the three provider pickers
- [ ] `personality-section` - Build Personality card (Name, 2D|3D, Role, Intelligence, Gender, Voice grid)
- [ ] `providers-section` - Build Provider card with independent Chat/Image/STT/TTS pickers + conditional endpoint fields + key status
- [ ] `models-section` - Build Models card with Chat/Image/STT/TTS model pickers + refresh/source
- [ ] `configuration-section` - Build Configuration card: Images, Audio (record quality + TTS enabled), Misc (loop/compaction/after-translation), WebSearch (toggles + key status)
- [ ] `autosave-wiring` - Replace global Save/dirty machinery with debounced per-section auto-save across the four stores; keep nickname validation
- [ ] `i18n-keys` - Add new I18nKeys to en_us + all locales; replace hardcoded English strings; run locale render script
- [ ] `css-tokens` - Style all cards/inputs with theme tokens + roundings, verify no hardcoded colors, light/dark check
- [ ] `verify-decouple` - Manual + test pass: STT/TTS independent, PTT untouched, auto-save round-trips, layout matches mockup
