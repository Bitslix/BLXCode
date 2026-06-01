# PTT-Review: Nebenläufigkeit, Main-Thread & Fehler-Toasts

Folge-Review zum [Push-to-Talk-Plan](voice-push-to-talk-stt.md). Ziel: **belegen
und absichern**, dass die gesamte PTT-/Whisper-/Modell-Manager-Kette sauber
async/multithreaded läuft (der Main-Thread der App hängt **nie**) und dass jeder
benutzer­relevante Fehler sauber als **Toast/Sonner** erscheint statt verschluckt
zu werden.

> Dies ist ein **Audit-Plan mit gezielten Fixes**, kein Feature-Plan. Jede
> Sektion hat: *Befund (im Code) → Risiko → Fix → Verifikation*.

## Geltungsbereich

Neuer/erweiterter Code aus dem PTT-Feature:
- Backend: `src-tauri/src/voice/{ptt,models,stt,recorder}.rs(/…)`
- Frontend: `src/workbench/ptt_runtime/`, `src/workbench/harness_voice_pane/{ptt_section,model_manager}/`
- Bridge: `src/tauri_bridge.rs` (ptt_*/whisper_* + Event-Listener)

## A. Main-Thread- & Async-Audit (höchste Priorität)

**Grundregel Tauri 2:** `#[tauri::command] pub fn …` (synchron) läuft auf dem
**Main-Thread**; `pub async fn …` läuft auf dem async-Runtime-Pool. Alles
Blockierende (Datei-I/O, Audio-Device-Enumeration, Netzwerk, Inferenz) gehört
**nicht** in einen sync-Command und **nicht** ungekapselt in einen async-Task.

### A1 — `ptt_start` ist synchron und blockiert potenziell den Main-Thread ⚠️
**Befund:** [`ptt_start`](../../src-tauri/src/voice/ptt/commands.rs) ist `pub fn`
(sync) und ruft `settings::load(&app)` (JSON-Datei lesen) + `recorder::start_pcm`
auf. `start_pcm` macht **vor** dem Worker-Thread `host.default_input_device()` +
`device.default_input_config()` — cpal-Device-Enumeration kann je nach
Audio-Backend (ALSA/PulseAudio/CoreAudio) **mehrere 100 ms blockieren**.
**Risiko:** UI-Freeze beim Tastendruck — genau das, was vermieden werden soll.
**Fix:** `ptt_start` auf `async` umstellen und die Device-Öffnung in
`tauri::async_runtime::spawn_blocking` verlagern (oder `recorder::start_pcm`
intern blocking-kapseln). Settings-Load ebenfalls in den blocking-Teil.
Alternativ: Mic-Warmup beim Aktivieren von PTT (nicht erst bei key-down).

### A2 — Download nutzt blockierendes `std::fs`/`std::io` im async-Task ⚠️
**Befund:** [`download_inner`](../../src-tauri/src/voice/models/mod.rs) läuft via
`tauri::async_runtime::spawn`, schreibt aber mit `std::fs::OpenOptions` +
`file.write_all(&chunk)` pro Chunk und prüft die Prüfsumme mit `sha256_file`
(`std::io::copy`, ganze Datei). Das **blockiert einen async-Runtime-Worker**
über die gesamte Download-/Hash-Dauer (Modelle bis ~1,5 GB).
**Risiko:** Bei wenigen Runtime-Threads Stau anderer async-Commands (z. B.
`ptt_finalize`, Agent-Streaming).
**Fix:** Datei-Schreiben auf `tokio::fs` umstellen **oder** den Schreib-/Hash-Teil
in `spawn_blocking` kapseln. sha256 immer in `spawn_blocking`.

### A3 — `whisper_models_list` / `whisper_model_delete` sync mit Datei-I/O
**Befund:** sync-Commands mit Verzeichnis-Scan / Datei-Stat / `remove_file`.
**Risiko:** gering (schnelle Operationen), aber formal Main-Thread-I/O.
**Fix:** entweder als `async` + `spawn_blocking`, oder bewusst belassen mit
Begründung (Operationen < wenige ms). Entscheidung dokumentieren.

### A4 — Frontend: keydown-Handler bleibt nicht-blockierend
**Befund:** [`install_ptt_runtime`](../../src-tauri/../../src/workbench/ptt_runtime/mod.rs)
liest im keydown synchron nur den thread_local-Settings-Cache + ShortcutConfig
(beides nicht-blockierend) und delegiert den Rest an `spawn_local`. ✅
**To verify:** sicherstellen, dass **kein** `*_get`/`invoke` synchron im
keydown/keyup läuft und der Settings-Cache wirklich befüllt ist (sonst No-Op).
Prüfen, dass `refresh_ptt_settings_cache` nach jedem Save **und** beim Aktivieren
läuft.

## B. Lock-Hold- & Backpressure-Audit

### B1 — `WhisperEngine`-Mutex wird über die ganze Inferenz gehalten ⚠️
**Befund:** [`transcribe_pcm`](../../src-tauri/src/voice/stt/local_whisper.rs)
lockt `inner` (Mutex) und ruft darin `state.full(...)` — der Lock wird also über
die **komplette** Inferenz gehalten. Damit serialisiert ein laufendes
Partial-Decode jedes Finalize und umgekehrt; `ensure_loaded` blockiert bis der
Decode fertig ist.
**Risiko:** Partial-Polls (alle ~400 ms) und Finalize konkurrieren um den Lock;
bei großen Modellen spürbare Latenz/Stau. Verstößt gegen „keine Locks über
Inferenz halten" aus dem Ursprungsplan.
**Fix:** Lock nur zum Klonen/Leihen des `WhisperContext`-Handles halten (der ist
`Arc`-intern teilbar), dann **außerhalb** des Locks `create_state()` + `full()`
laufen lassen. Ziel: ein laufender Decode hält den Engine-Lock nicht.

### B2 — Überlappende Partial-Decodes (fehlende Backpressure) ⚠️
**Befund:** [`start_partial_poll`](../../src/workbench/ptt_runtime/mod.rs) pollt
`ptt_partial` alle 400 ms. Dauert ein Decode länger als 400 ms (lange Aufnahme,
großes Modell), kann der nächste Poll starten, bevor der vorige fertig ist →
mehrere parallele `spawn_blocking`-Inferenzen, die sich am Engine-Lock (B1)
stauen.
**Fix:** „in-flight"-Guard (ein `Cell<bool>`/AtomicBool): nächsten Poll erst nach
Abschluss des vorigen starten; sonst Tick überspringen. Optional adaptives
Intervall.

### B3 — `snapshot_pcm` klont den wachsenden Buffer pro Poll
**Befund:** [`snapshot_pcm`](../../src-tauri/src/voice/recorder.rs) klont den
gesamten `Vec<f32>` unter Lock bei jedem Partial-Poll; der Buffer wächst linear
mit der Sprechdauer.
**Risiko:** gering–mittel (O(n) Kopie alle 400 ms), aber unnötige Allokationen.
**Fix:** akzeptabel für MVP; optional nur den Zuwachs liefern oder Decode-Cap
(letzte N Sekunden). Entscheidung dokumentieren.

### B4 — Recorder-Worker-Thread-Lebenszyklus
**Befund:** `start_pcm` spawnt pro Aufnahme einen OS-Thread; `stop_pcm`/
`cancel_pcm` joinen ihn. on_cleanup im Frontend ruft `ptt_cancel`.
**To verify:** kein Thread-/Stream-Leak bei (a) App-Fokusverlust mitten in der
Aufnahme, (b) Komponenten-Unmount, (c) Reject-Pfad (Recording wurde nie
gestartet, turn_id None). Prüfen, dass `VoiceRuntimeState` in **jedem** Pfad
wieder auf `Idle` zurückfällt (auch bei `ptt_finalize`-Fehler — aktuell wird
`Idle` gesetzt, gut; Reject-Pfad setzt nie RecordingPtt, gut).

## C. Fehler → Toast/Sonner-Audit (benutzer­relevant)

**Befund:** Der `ToastService` ([toast.rs](../../src/workbench/toast.rs)) bietet
`.success(msg)` / `.error(msg)` und ist im Workbench-Context verfügbar. Im
PTT-Code werden Fehler aber überwiegend **verschluckt**:
- [`ptt_runtime`](../../src/workbench/ptt_runtime/mod.rs): `ptt_finalize(...).unwrap_or_default()`
  → Transkriptions-/Modellfehler verschwinden lautlos; `ptt_start` Err → nur
  `recording=false`, **kein** Toast; Routing-Fehler (`pty_write`, clipboard,
  insert) via `let _ = …` ignoriert.
- Reject-Fälle setzen `bus.hint` (Indicator), aber **kein** Toast.

**Ziel:** Jeder benutzer­relevante Fehler erscheint als **Toast** (`error`),
lokalisiert über i18n. Nicht-benutzerrelevantes (z. B. „cancelled") bleibt still.

### C1 — Fehlerquellen → Meldung (Mapping festlegen)
| Quelle | Auslöser | UX |
|---|---|---|
| `ptt_start` Err / kein Mic | Mikrofon nicht öffenbar | Toast `VoiceErrNoMic` |
| Reject „busy" | zweite Mic-Session | Toast/Hint `VoicePttMicBusy` |
| Reject „TTS playing" (Block) | Kollision | Hint **+** optional Toast `VoicePttBlockedTts` |
| `ptt_finalize` Err „kein Modell" | `local_model_path` leer | Toast `VoicePttNoModel` |
| `ptt_finalize` Err Laden/Inferenz | Modell kaputt/Backend | Toast `VoicePttModelLoadFailed` / generisch |
| Cloud-Key fehlt | Provider nicht konfiguriert | Toast (bestehende Key-Meldung) |
| Insert fehlgeschlagen | `pty_write`/Clipboard/DOM Err | Toast `VoicePttInsertFailed` |
| Download-Fehler | Netz/sha/Disk | **bereits** inline in der Karte ✅ + optional Toast |

### C2 — Umsetzung
- `ToastService` im `ptt_runtime` via `use_context` holen und an die
  finalize/route/start-Pfade durchreichen (Toasts laufen im Leptos-Owner,
  daher Aufruf aus dem `spawn_local`-Ergebnis, nicht im Backend).
- Fehlende i18n-Keys ergänzen (`VoicePttModelLoadFailed`, `VoicePttInsertFailed`
  — die übrigen existieren bereits) in **allen** Locales (Render-Skript).
- Backend-Fehlerstrings, die heute Deutsch hartcodiert sind (z. B. in
  `recorder.rs` „Kein Default-Audio-Eingang…"), entweder als stabile Codes
  zurückgeben und im Frontend lokalisieren, **oder** dokumentieren, dass sie
  nur als Detail in einen lokalisierten Toast eingebettet werden.

### C3 — Konsistenz mit bestehendem Voice-Flow
Prüfen, dass der bestehende Voice-Orb-Pfad (`VoiceErrNoMic`/`VoiceErrStt`)
dieselbe Toast-Konvention nutzt, damit PTT sich nahtlos einfügt (kein zweiter
Fehlerstil).

## D. Verifikation

1. **Statisch:** `cargo check` beide Crates warnungsfrei; gezielt nach
   `unwrap_or_default`/`let _ =` in den PTT-Modulen grep’en und jeden Treffer
   bewerten.
2. **Async-Beleg:** kurzer Lasttest — Download eines großen Modells **während**
   eine PTT-Aufnahme + Agent-Streaming läuft; UI muss flüssig bleiben
   (kein Frame-Hang). Manuell über die App (`cargo tauri dev`).
3. **Lock-Beleg (B1/B2):** mit `Best`-Quality + langer Aufnahme prüfen, dass
   Partial-Polls nicht stauen und Finalize zügig kommt.
4. **Fehler-Beleg (C):** je Fehlerquelle gezielt provozieren (kein Modell,
   Modellpfad kaputt, Mic entzogen, Cloud-Key fehlt, Terminal-Ziel ohne aktives
   Terminal) → Toast erscheint, Wortlaut lokalisiert, App stabil.
5. **Tests:** wo praktikabel Unit-Tests (z. B. Fehler-Mapping-Funktion, in-flight-
   Guard-Logik). Reine UI-Pfade via manueller Verifikation (kein neues
   Test-Framework einführen).

## Reihenfolge / Aufwand

1. **A1 + A2** (Main-Thread/async) — höchste Priorität, klar abgegrenzt.
2. **B1 + B2** (Lock/Backpressure) — mittel.
3. **C1–C3** (Toasts + i18n) — breit, aber mechanisch.
4. **B3/A3/B4** — Bewertung + ggf. dokumentierte Akzeptanz.
5. **D** — Verifikation, dann Plan auf „done".

## Akzeptanzkriterien

- Kein PTT-Pfad führt blockierende Arbeit (Datei-I/O, Device-Open, Inferenz,
  Hashing) auf dem Main-Thread oder ungekapselt auf der async-Runtime aus.
- Engine-Lock wird nicht über die Inferenz gehalten; Partial-Polls überlappen
  nicht.
- Jeder benutzer­relevante Fehler erscheint als lokalisierter Toast; „cancelled"
  und Routine-Abbrüche bleiben still.
- Beide Crates bauen warnungsfrei; manuelle Verifikation (D2–D4) bestanden.
