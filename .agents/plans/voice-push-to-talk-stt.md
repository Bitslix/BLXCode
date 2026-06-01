# Push-to-Talk STT (lokal whisper.cpp + Cloud)

Push-to-Talk-Spracheingabe für BLXCode: lokaler, warm gehaltener
`whisper.cpp`-Backend als Default plus optionaler Cloud-Pfad. Taste gedrückt →
sofort aufnehmen → Taste los → finaler Transkript wird in das gemerkte Ziel
(Agent-Composer, Terminal-Slot, aktives Textfeld oder Clipboard) geschrieben.

> **Wichtig — das ist kein Greenfield.** BLXCode hat bereits ein vollständiges
> Voice-Subsystem. Dieser Plan **erweitert** es, dupliziert nichts. Alle
> Vorschläge der ursprünglichen Aufgabenstellung
> ([blxcode-push-to-talk-stt-agent-prompt.md](../../blxcode-push-to-talk-stt-agent-prompt.md))
> wurden gegen den realen Code gespiegelt.

## Bestandsaufnahme (was es schon gibt)

| Bereich | Datei | Status / Wiederverwendung |
|---|---|---|
| Mic-Capture (cpal → WAV, Resampling, Downmix) | [recorder.rs](../../src-tauri/src/voice/recorder.rs) | **Refactor**: PCM-f32-Pfad statt nur WAV (whisper braucht In-Memory-PCM) |
| Cloud-STT (OpenAI/OpenRouter multipart) | [stt.rs](../../src-tauri/src/voice/stt.rs) | **Reuse** als `cloud`-Provider; AWS wird hier bereits korrekt für STT abgelehnt |
| TTS (OpenAI speech) | [tts.rs](../../src-tauri/src/voice/tts.rs) | unverändert; nur Kollisions-Hook |
| Settings-Envelope (`voice` in `agent_provider_settings.json`) | [settings.rs](../../src-tauri/src/voice/settings.rs) | **Erweitern** um `ptt`-Sub-Objekt; `PttHotkey` existiert bereits |
| Tauri-Commands | [commands.rs](../../src-tauri/src/voice/commands.rs) | **Erweitern** um lokale-STT-/PTT-Commands |
| Provider-Key-Resolver (inkl. AWS-Polly nur TTS) | [settings.rs](../../src-tauri/src/voice/settings.rs) `provider_key` | **Reuse**, korrekt |
| Orb-State-Machine (Idle/RecordingHold/Toggle/Transcribing) | [voice_orb/state.rs](../../src/workbench/agent_panel/voice_orb/mod.rs) | **Erweitern** um Transcribing/Partial |
| Globaler Hotkey-Hook (window keydown/keyup) | [voice_orb/mod.rs](../../src/workbench/agent_panel/voice_orb/mod.rs) `install_ptt_hotkey` | **Reuse + Ziel-Routing**; aktuell fest an Composer (`on_transcript`) gebunden; Key kommt künftig aus `ShortcutConfig` |
| Data-driven Shortcuts (`ShortcutAction`/`KeyChord`/`Binding`, Capture-/Reset-/Konflikt-UI) | [shortcut_config.rs](../../src/workbench/shortcut_config.rs), [shortcuts_settings_pane/mod.rs](../../src/workbench/shortcuts_settings_pane/mod.rs) | **Reuse**: PTT-Key wird hier als neue `ShortcutAction::PushToTalk` definierbar |
| Settings-Pane (1082 Z.) | [harness_voice_pane/mod.rs](../../src/workbench/harness_voice_pane/mod.rs) | **Erweitern** um PTT-Section |
| i18n: 48 `Voice*`-Keys inkl. `VoicePttSection`/`VoicePttEnabled`/`VoicePttHotkey` | [keys.rs](../../src/i18n/keys.rs) | **Erweitern**, viele Keys vorhanden |
| Terminal-Write | [tauri_bridge.rs](../../src/tauri_bridge.rs) `pty_write` | **Reuse** für Terminal-Ziel |
| Clipboard | [clipboard.rs](../../src-tauri/src/clipboard.rs) | **Reuse** für Clipboard-Ziel |

**Architektur des bestehenden Voice-Flows** ist *request/response*, nicht
poll-based: `voice_start_recording` → `turn_id`, `voice_stop_and_transcribe` →
`text`. Der poll-basierte `AgentEvent`-Kanal gehört dem Agent-Stream. Partial-
Transkripte brauchen daher einen **eigenen, kleinen Event-Kanal** (siehe P3).

## Designprinzipien

- **Erweitern statt parallel bauen.** Keine zweite Recorder-/Settings-/Hotkey-
  Welt. Local- vs. Cloud-STT wird hinter einem Trait abstrahiert, der Rest des
  Flows bleibt identisch.
- **Regelkonform**: neue UI-Bausteine als eigener Subfolder + eigene CSS
  (`rule-reusable-components`); keine Monolith-Erweiterung von `commands.rs`/
  `harness_voice_pane` über die PTT-Section hinaus (`rule-no-monolith-structure`);
  ausschließlich `var(--token)` (`rule-theme-tokens`).
- **whisper.cpp warm halten**: Modell wird einmal in einen `tauri::State`
  geladen und bleibt im Speicher; Inferenz läuft in `spawn_blocking`, niemals
  auf dem IPC-Thread.
- **Privacy-Default**: lokal first; kein WAV-File-Write im PTT-Pfad (In-Memory-
  PCM); kein Feedback-Loop (Mic blockt während TTS per Default).

## Offene Entscheidungen (als TODO im Code dokumentieren, sicheren Subset bauen)

1. **whisper-Crate**: `whisper-rs` (sichere Bindings auf whisper.cpp). Bringt
   eine native C/C++-Build-Abhängigkeit + Modell-Download mit → das ist die
   einzige *große* neue Dependency und muss laut `Implementation Rules`
   begründet werden. **Begründung**: einzige ausgereifte Rust-Anbindung an
   whisper.cpp; CPU-only baubar ohne CUDA/Metal; Feature-Flags erlauben spätere
   GPU-Backends ohne Rewrite. → Hinter Cargo-Feature `local-whisper` kapseln,
   damit Builds ohne Toolchain weiter durchlaufen (Cloud-only Fallback).
2. **Hotkey-Definition durch den User → Settings → Shortcuts.** Der PTT-Key
   wird als neue `ShortcutAction::PushToTalk` in den bestehenden data-driven
   Shortcuts geführt. Damit erbt PTT die komplette Capture-/Rebind-/Reset-/
   Konflikt-UI von [shortcuts_settings_pane](../../src/workbench/shortcuts_settings_pane/mod.rs)
   gratis — keine zweite Keyfeld-Insellösung in der Voice-Pane. Details unten
   („PTT-Key in Settings → Shortcuts"). **Einschränkung**: bleibt window-level
   (nur bei fokussierter App); echter OS-globaler Hotkey bräuchte
   `tauri-plugin-global-shortcut` → späteres Add-on, klares TODO.
3. **Streaming/Partials**: whisper.cpp streamt nicht nativ — es transkribiert
   immer einen kompletten Audio-Block, es gibt kein „gib mir den Zwischenstand".
   Live-Text wird daher per **Re-Decode-Worker** erzeugt: in festen Intervallen
   wird der bisher aufgenommene Ring-Buffer *komplett neu* durch whisper gejagt;
   das jeweils letzte Ergebnis ist der Partial. Das ist mit zunehmender Dauer
   CPU-intensiv (jeder Tick dekodiert das ganze, länger werdende Audio neu),
   deshalb **throttled** (≥300 ms zwischen Updates). Per Default **on**
   (gewünschtes Verhalten: Live-Text out of the box); der User kann es per
   Toggle abschalten, um CPU/Akku zu schonen.
4. **Modell-Manager mit Download**: vollwertige Modell-Liste mit Download-
   Progressbar (Bytes/Total, Speed in MB/s, ETA), Installed-/Delete-Status,
   Filtern und statischen Bewertungen (Speed/Accuracy). Quelle sind die
   offiziellen GGML/GGUF-Dateien des whisper.cpp-Repos (Hugging Face
   `ggerganov/whisper.cpp`). Details siehe Abschnitt „Modell-Manager". Ein
   freier „eigener Pfad"-Eintrag bleibt zusätzlich möglich (Power-User).

## Architektur

```
Frontend (blxcode-ui)                       Backend (blxcode)
─────────────────────                       ──────────────────
voice_orb/  (erweitert)                      voice/
  - VoiceOrbState +Partial                     settings.rs   (+ PttSettings)
  - install_ptt_hotkey → Ziel-Routing          recorder.rs   (+ PCM-Buffer-API)
                                               stt/
ptt_target/ (neu)                                mod.rs       (SttBackend-Trait)
  - PttTarget capture/restore                    cloud.rs     (= heutiges stt.rs)
                                                 local_whisper.rs (whisper-rs)
harness_voice_pane/                            ptt/
  ptt_section/ (neu, eigene CSS)                 mod.rs       (VoiceRuntimeState)
                                                 collision.rs (State-Machine)
                                               commands.rs   (+ ptt_* Commands)
```

### Ziel-Routing (Frontend)

```rust
// src/workbench/agent_panel/voice_orb/  (an bestehende BLXCode-Typen anlehnen)
enum PttTarget {
    AgentComposer,
    TerminalSlot { session_id: u64 },   // session_id wie bei pty_write
    ActiveTextInput,                    // document.activeElement merken
    Clipboard,
}
```

Ziel wird bei **key-down** erfasst (`capture_target()` liest aktives Element /
aktiven Terminal-Slot über bestehende Workbench-State-Signale). Bei
`Remember target at PTT start` wird `PttTarget` in einem `RwSignal` festgehalten
und bei key-up unabhängig vom aktuellen Fokus bedient. Insert:
- AgentComposer → bestehender `on_transcript`-Callback
- TerminalSlot → `pty_write(session_id, base64(text))`
- ActiveTextInput → `execCommand`/value-set auf gemerktem Element
- Clipboard → bestehende Clipboard-Bridge

### Kollisions-State-Machine (Backend)

```rust
enum VoiceRuntimeState { Idle, RecordingPtt, TranscribingPtt, PlayingTts, AgentVoiceInputActive }
```
Single `Mutex<VoiceRuntimeState>` in `tauri::State`. Übergänge:
- PTT-Start während `PlayingTts` → per Setting: Stop / Pause / Block(+Hint).
  Default **Block** (kein Feedback-Loop).
- Keine zwei Mic-Sessions: PTT-Start während `RecordingPtt`/
  `AgentVoiceInputActive` wird abgelehnt.

## Settings-Erweiterung (`voice.ptt`)

```rust
struct PttSettings {
    enabled: bool,                 // default false
    mode: PttMode,                 // Local | Cloud   (default Local)
    local_model_path: Option<String>,
    local_quality: WhisperQuality, // Fast | Balanced | Best
    cloud_provider: VoiceProviderKind, // reuse; AWS bleibt für STT gesperrt
    cloud_model_id: String,
    insert_target: PttInsertTarget,    // Agent | Terminal | ActiveInput | Clipboard
    target_mode: PttTargetMode,        // CurrentFocus | RememberStart
    auto_submit: bool,             // default false
    partial_transcript: bool,      // default true (Live-Text an; abschaltbar)
    tts_collision: TtsCollision,   // Stop | Pause | Block (default Block)
}
```
Serialisierung folgt dem bestehenden `#[serde(default)]`-Muster → **keine
Regression** am Envelope (alte Configs ohne `ptt` laden weiter).

> **Hinweis zur `PttHotkey`-Dublette:** Das vorhandene `PttHotkey`-Feld in
> [settings.rs](../../src-tauri/src/voice/settings.rs) (code + ctrl/shift/alt/meta)
> wird **nicht** weiter als Quelle für den Key genutzt. Single source of truth
> für den Tastendruck ist `ShortcutConfig` (siehe nächster Abschnitt).
> `PttHotkey.enabled` darf als reines Aktiv-Flag bestehen bleiben (Migration:
> in `PttSettings.enabled` überführen); das Key-Feld wird deprecaten, damit es
> keine zwei abweichenden Tastenbelegungen gibt.

## PTT-Key in Settings → Shortcuts

Der User definiert die Taste dort, wo alle anderen Shortcuts liegen — kein
Extra-Keyfeld in der Voice-Pane.

- **Neue Aktion**: `ShortcutAction::PushToTalk` zu `ShortcutAction::ALL`,
  `label_key()`, `action_icon()` (z. B. `LuMic`) ergänzen. Dadurch rendert
  [shortcuts_settings_pane](../../src/workbench/shortcuts_settings_pane/mod.rs)
  **automatisch** eine Row mit Rebind-Capture, Reset und Konflikt-Warnung —
  ohne neue UI-Komponente.
- **Default-Binding**: immer `Binding::Combo` (kein tmux-Prefix-Chord, da PTT
  gehalten wird). Vorschlag `Ctrl+Shift+Space` (kollidiert nicht mit den
  bestehenden Combos). `apply_shortcut_preset` muss `PushToTalk` in **beiden**
  Presets als Combo seeden (nicht als Chord).
- **Hold- statt Press-Semantik — wichtig**: Die übrigen Shortcuts feuern bei
  *keydown* in [harness_chords](../../src/workbench/harness_chords.rs). PTT
  braucht keydown→start / keyup→stop. Deshalb wird `PushToTalk` **nicht** in den
  press-fire-Dispatcher von `harness_chords` eingehängt. Die Shortcuts-Pane
  liefert nur Anzeige + Capture + Reset; das Matching bleibt in
  `install_ptt_hotkey`, das den Chord künftig aus
  `prefs.shortcut_config().binding(PushToTalk)` liest und `KeyChord::matches`
  auf keydown/keyup anwendet (Hold).
- **Semantik-Abgleich `KeyChord` ↔ heutiger PTT-Code**: `KeyChord` matcht über
  `ev.key()` und faltet Ctrl/Meta zusammen; das heutige `install_ptt_hotkey`
  nutzt `ev.code()` und erlaubt bare-Space. Umstellung auf `KeyChord::matches`
  ist gewollt (eine Matching-Logik im ganzen Projekt). Die `focus_in_editable()`-
  Ausnahme (bare Taste ohne Modifier in Eingabefeldern nicht abfangen) bleibt
  erhalten — relevant, falls der User PTT bewusst auf eine modifierlose Taste legt.
- **Bonus**: Die vorhandene `conflicts()`-Prüfung warnt automatisch, wenn der
  User PTT auf eine bereits belegte Aktion legt.

In der Voice-Pane verbleibt nur ein **read-only Hinweis** „Taste in Settings →
Shortcuts ändern" (verlinkt), plus der Enable-Toggle.

## Modell-Manager (lokale whisper-Modelle)

Eigene Komponente `harness_voice_pane/model_manager/` (Subfolder + CSS, nur
Tokens — `rule-reusable-components`). Liste auswählbarer/herunterladbarer
whisper.cpp-Modelle mit Download-Fortschritt, Verwaltung und Bewertungen.

### Katalog (statisch, im Backend)

Ein fest gepflegter `WhisperModelCatalog` (keine Remote-Discovery im MVP →
kein API-Call, kein Vertrauen in Drittquellen zur Laufzeit). Pro Eintrag:

```rust
struct WhisperModel {
    id: String,             // "tiny", "tiny-q8", "tiny.en", "base", "base-q8", "small", ...
    label: String,          // "Tiny", "Tiny Q8", "Base", ...
    family: ModelFamily,    // Standard | Quantized | Turbo | Large   (Filter-Tabs)
    multilingual: bool,     // true | false (EN only)
    size_bytes: u64,        // für Anzeige + Progress-Total-Fallback
    url: String,            // Hugging Face ggerganov/whisper.cpp Resolve-URL
    sha256: String,         // Integritätsprüfung nach Download
    speed_rating: u8,       // 1..=5  (statische Bewertung → Punkte-UI)
    accuracy_rating: u8,    // 1..=5
    best_for_key: I18nKey,  // "Best for: …" Hinweistext (lokalisiert)
}

enum ModelFamily { Standard, Quantized, Turbo, Large }
```

Installiert-Status wird **nicht** im Katalog gespeichert, sondern zur Laufzeit
ermittelt: Datei existiert in `app_data_dir/voice/models/<id>.bin` (+ optional
sha-Verify). So bleibt der Katalog rein deklarativ.

### Backend-Commands + Download-Events

- `whisper_models_list() -> Vec<WhisperModelView>` — Katalog + `installed: bool`
  + `installed_path` + `partial_bytes: Option<u64>` (Größe eines vorhandenen
  `.part` → Resume-Anzeige). Treibt die Liste.
- `whisper_model_download(id)` — streamt die Datei nach
  `app_data_dir/voice/models/<id>.bin.part`, atomar umbenennen nach Erfolg +
  sha256-Verify. Läuft in einem Background-Task (`spawn`), **nicht** blockierend.
  **Resume**: existiert bereits ein `.part`, wird mit `Range: bytes=<len>-`
  fortgesetzt und an die Datei angehängt (`OpenOptions::append`); der Server
  muss `206 Partial Content` liefern — bei `200` (kein Range-Support) wird das
  `.part` verworfen und von vorn geladen. `received` startet beim bestehenden
  `.part`-Offset, `total` = Offset + `Content-Length` bzw. Katalog-`size_bytes`.
  Fortschritt über eigenen Event-Kanal (derselbe wie PTT-Events, P3):
  `WhisperDownloadProgress { id, received, total, speed_bps }` (throttled
  ~200 ms) und `WhisperDownloadDone { id }` / `WhisperDownloadError { id, msg }`.
- `whisper_model_cancel(id)` — pausiert/bricht laufenden Download ab; das `.part`
  **bleibt erhalten**, damit ein erneuter Download per Range fortsetzt
  (explizites „Verwerfen" löscht das `.part`).
- `whisper_model_delete(id)` — entfernt installierte Datei.

`speed_bps`/ETA werden im Backend aus einem gleitenden Fenster der empfangenen
Bytes berechnet (kein Verlass auf Server-`Content-Length` allein).

### UI (an den Screenshot angelehnt)

- **Filter-Tabs**: All · Standard · Quantized · Turbo · Large (= `ModelFamily`),
  plus Sortier-Dropdown (Default / Size / Accuracy / Speed).
- **Karte pro Modell**: Name + Family-Badge + Multilingual/EN-Only-Badge,
  Beschreibung, Größe, Speed-/Accuracy-Punkte (gefüllte/leere Dots aus
  `speed_rating`/`accuracy_rating`), „Best for"-Zeile (muted), und rechts der
  Aktionsbereich:
  - nicht installiert, kein `.part` → **Download**-Button
  - läuft → **Progressbar** (received/total %), Speed (MB/s), ETA, Pause/Cancel
  - pausiert/unvollständig (`.part` vorhanden) → **Resume**-Button (zeigt
    bereits geladenen Anteil) + „Verwerfen"
  - installiert → „Installed"-Badge + **Delete**-Button; aktives Modell
    zusätzlich markiert (das in `PttSettings.local_model_path` gewählte)
- Auswahl eines installierten Modells setzt `PttSettings.local_model_path` →
  ist das warm geladene Modell des `WhisperEngine`-State (Reload bei Wechsel).
- Nur Theme-Tokens; Punkte/Badges/Progress über CSS-Vars, keine Literalfarben.

### Verhältnis zu den Quality-Presets

`WhisperQuality` (Fast/Balanced/Best) bleibt als **Inferenz-Parameter**
(threads/beam/strategy), unabhängig vom Modell. Modellwahl (Genauigkeit/Größe)
und Decode-Qualität sind getrennte Achsen; UI erklärt das knapp per Hint.

## Umsetzung in Phasen

### P0 — Fundament (Backend, kein UI)
- `PttSettings` + Defaults in [settings.rs](../../src-tauri/src/voice/settings.rs); Serde-Roundtrip-Tests.
- `recorder.rs`: neue API `start_pcm`/`stop_pcm` → liefert `Vec<f32>` 16 kHz mono In-Memory (Ring-Buffer, optional Pre-Roll). WAV-Pfad bleibt für Cloud-Reuse.
- `stt/mod.rs`: Trait `SttBackend { async fn transcribe_pcm(&self, pcm: &[f32]) -> Result<String> }`. `cloud.rs` = heutiges `stt.rs` (PCM→WAV-In-Memory→multipart).

### P1 — Lokales whisper.cpp + Modell-Manager-Backend
- Cargo-Feature `local-whisper` + `whisper-rs` (begründet, gekapselt).
- `local_whisper.rs`: Modell einmal laden (`tauri::State<WhisperEngine>`), Inferenz in `spawn_blocking`. Quality-Presets → whisper-Parameter (threads/beam/strategy).
- `voice/models/`: `WhisperModelCatalog` (statisch), `whisper_models_list/download/cancel/delete` Commands, streamender Download nach `app_data_dir/voice/models/` mit sha256-Verify + Progress/Speed-Events. In [lib.rs](../../src-tauri/src/lib.rs) registrieren.
- Fehlerpfade: Modellpfad fehlt / Laden fehlgeschlagen / Mic nicht öffenbar / Backend-Init / Transkription / Download (Netz, sha-Mismatch, Disk) → klare Strings (i18n).

### P2 — Commands + Kollision + Routing
- `ptt/collision.rs`: `VoiceRuntimeState`-Machine + Tests.
- `commands.rs`: `ptt_start`, `ptt_stop_finalize`, `ptt_cancel`, `ptt_test` (kurze Aufnahme→Transkript, ohne Insert). In [lib.rs](../../src-tauri/src/lib.rs) registrieren.
- Frontend Ziel-Routing (`PttTarget` capture/restore) + Insert-Bridges.

### P3 — Partial-Transkript (default on, abschaltbar)
- Eigener Event-Kanal (kleiner `VecDeque`+poll **oder** `app.emit`), getrennt vom Agent-Stream. Events: `PttRecordingStarted/PartialTranscript/FinalTranscript/RecordingStopped/Error/StateChanged` + Modell-Download: `WhisperDownloadProgress{received,total,speed_bps}/WhisperDownloadDone/WhisperDownloadError` (throttled ~200 ms).
- **Re-Decode-Worker**: periodisches Komplett-Dekodieren des Ring-Buffers, Updates throttlen (≥300 ms). Laufzeit-Gate: Worker startet nur bei `partial_transcript == true` (Default an); bei `false` nur ein finales Decode beim Loslassen.
- **Cloud-Mode**: kein Re-Decode (zu teuer/langsam pro Request) — Partials bleiben dort aus, unabhängig vom Toggle; UI-Hinweis bzw. Toggle nur im Local-Mode aktiv.

### P4 — Settings-UI + Shortcuts-Integration
- **Shortcuts**: `ShortcutAction::PushToTalk` in [shortcut_config.rs](../../src/workbench/shortcut_config.rs) (`ALL`, `label_key`, Default-Combo, Preset-Seeding als Combo) + `action_icon`; `install_ptt_hotkey` liest Chord aus `ShortcutConfig` statt `PttHotkey` und behält Hold-Semantik. PTT **nicht** in `harness_chords` press-fire einhängen.
- **Voice-Pane**: `harness_voice_pane/ptt_section/` (eigener Subfolder + CSS, nur Tokens). Alle Felder aus der Aufgabenstellung außer Key (Enable, Mode, Quality, Cloud-Provider/Model, Insert-Target, Target-Mode, Auto-Submit, Partial, TTS-Kollision) + Test-Button + Inline-Error-States + read-only Hinweis „Taste in Settings → Shortcuts". Bestehende Card-/Segmented-/Select-/Switch-Muster wiederverwenden.
- **Modell-Manager**: `harness_voice_pane/model_manager/` (eigener Subfolder + CSS). Filter-Tabs, Sortierung, Karten mit Speed-/Accuracy-Dots, Download/Progressbar/Speed/ETA/Cancel, Installed/Delete, aktives Modell markiert. Konsumiert die Download-Progress-Events aus P3.

### P5 — i18n, Doku, Tests
- Fehlende `VoicePtt*`-Keys in **allen** `locales/*.rs` (Exhaustiveness-Pflicht); Deutsch sauber, Rest via `scripts/render_i18n_locales_from_en.py`.
- Doku: User-Doku Voice/PTT + Troubleshooting (Modell fehlt, Mic, langsame Transkription, Cloud nicht konfiguriert, Feedback-Loop).
- Tests: Settings-Serde/Defaults (inkl. `partial_transcript=true`), Ziel-Routing, Remember-Target, Kollisions-Machine, Modellpfad-Validierung, Katalog-Integrität (URLs/sha vorhanden, IDs eindeutig), Installed-Detection, Resume-Offset-Logik (`.part`-Größe → Range-Header, 206 vs. 200-Fallback), Insert-Auswahl, i18n-Key-Presence (falls vorhanden), Voice-Envelope-Non-Regression.

## Akzeptanzkriterien (gegen Prompt gespiegelt)

PTT-Settings-Section ✓ · lokal whisper.cpp aktivierbar ✓ · Modell-Manager mit
Download-Progress/Speed/Ratings, Installed/Delete ✓ · Modell einmal warm ✓ ·
Hotkey über Settings → Shortcuts frei definierbar (Rebind/Reset/Konflikt) ✓ ·
Hotkey startet sofort ✓ · Loslassen finalisiert ✓ · Partials optional ✓ ·
Insert in Composer/Terminal/Active-Input/Clipboard ✓ · Remember-Target trotz
Fokuswechsel ✓ · Cloud via bestehender Provider ✓ · AWS-Polly **nicht** als STT ✓ ·
keine Kollision mit Agent-STT/TTS ✓ · i18n-Keys ✓ · nur Theme-Tokens ✓ ·
bestehende Voice-Features intakt ✓ · Rust-Tests ✓ · Doku ✓.

## Risiken

- **whisper-rs Build**: native Toolchain nötig → Feature-Flag schützt Cloud-only-Builds.
- **Modellgröße/RAM**: Best-Preset (large) ist schwer → Default Balanced, Hint in UI.
- **Window-level Hotkey** deckt nur fokussierte App ab — als bekannte Einschränkung dokumentieren.
- **Hold vs. Press**: PTT darf nicht in den press-fire-Dispatcher von `harness_chords`; Tests/Review sicherstellen, dass eine als PTT belegte Taste nicht zusätzlich eine reguläre Aktion auslöst (`conflicts()` warnt, blockt aber nicht).
- **`KeyChord`-Migration**: Umstieg von `ev.code()`/bare-Space auf `KeyChord` (`ev.key()`, Ctrl/Meta gefaltet) — Default-Combo bewusst kollisionsfrei wählen; `PttHotkey`-Key-Feld deprecaten ohne alte Configs zu brechen.
- **Modell-Download**: große Dateien (74 MB–1,5 GB) — atomarer `.part`→Rename, sha256-Verify gegen korrupte/abgebrochene Downloads, **Resume via HTTP-Range** (Fallback auf Full-Download bei `200` statt `206`); Katalog-URLs/Hashes müssen gepflegt werden (brechen, wenn HF-Pfade sich ändern → Test prüft Form, nicht Erreichbarkeit).
