# Task: Implement local/cloud Push-to-Talk STT for BLXCode

You are working in the BLXCode Rust/Tauri codebase.

Implement a production-ready Push-to-Talk speech-to-text system with a local `whisper.cpp` backend and optional cloud mode using the already existing voice/provider infrastructure where applicable.

The goal is fast, local-first voice input for the BLXCode Agent composer, terminals, and optionally any active text input.

## Important

Before implementing, analyze the existing codebase carefully:

- Existing voice recorder / voice input implementation
- Existing STT/TTS provider settings
- Existing OpenAI/OpenRouter/AWS Polly integrations
- Existing BLXCode Agent composer input flow
- Existing terminal input/write flow
- Existing settings architecture
- Existing i18n structure
- Existing theme/style-token system
- Existing shortcut/hotkey handling
- Existing task/event architecture between frontend and Tauri backend

Do not make assumptions where the codebase already has a pattern. Follow existing BLXCode conventions.

---

## Feature Goal

Add a new Push-to-Talk section in BLXCode Settings.

The user should be able to configure:

- Push-to-Talk enabled/disabled
- Input hotkey
- STT mode:
  - `Local`
  - `Cloud`
- Local backend:
  - `whisper.cpp`
- Local model:
  - selectable model path
  - downloadable/selectable model presets if the project already supports such flows
  - quality presets: `Fast`, `Balanced`, `Best`
- Cloud provider:
  - reuse existing supported providers where possible
  - OpenAI transcription if already available or suitable
  - OpenRouter/OpenAI-compatible transcription only if the existing provider supports it
  - AWS Polly must be treated carefully: Polly is TTS, not STT. Do not incorrectly use Polly for transcription. Only integrate it where it already belongs in the existing TTS flow.
- Insert behavior:
  - insert transcript into BLXCode Agent composer
  - insert transcript into selected/active text input
  - insert transcript into active terminal
  - copy transcript to clipboard
- Target behavior:
  - `Current focus`
  - `Remember target at Push-to-Talk start`
  - `Always BLXCode Agent composer`
  - `Always active terminal`
  - `Clipboard only`
- Auto-submit behavior:
  - off by default
  - optional auto-submit for BLXCode Agent composer
  - optional auto-submit/send for terminal target
- Partial transcript display:
  - enabled/disabled
  - show live partial transcript while speaking
- Collision handling:
  - prevent conflicts between Push-to-Talk STT and existing BLXCode Agent STT/TTS playback
  - prevent microphone capture while TTS is speaking unless explicitly allowed
  - optionally pause/duck TTS when Push-to-Talk starts
  - avoid feedback loops where TTS output is captured by STT

---

## Required Runtime Flow

Implement this behavior:

```text
App start:
  STT engine is initialized if Push-to-Talk local mode is enabled.
  whisper.cpp model is loaded once.
  Model stays warm in memory.
  No model loading should happen on every Push-to-Talk use.

Push key down:
  Capture the current insertion target.
  Start recording immediately into a ring buffer.
  Include a small pre-roll buffer if possible.
  Optionally start streaming/partial decoding if supported.
  Show recording state in the UI.

While speaking:
  Continue writing audio into the ring buffer.
  Show partial transcript if enabled.
  Keep UI responsive.
  Do not block the main frontend thread or Tauri command handler.

Push key up:
  Stop recording.
  Finalize transcription.
  Commit the final transcript.
  Insert it into the configured target:
    - remembered Agent composer
    - remembered terminal
    - currently active text input
    - clipboard
  Clear partial transcript state.
```

---

## Local STT Backend

Use `whisper.cpp` as the local backend.

Implementation requirements:

- Use a Rust-compatible integration for `whisper.cpp`.
- Prefer existing crates/bindings only after checking project compatibility.
- Keep inference off the UI thread.
- Load the model once and keep it warm.
- Support CPU-only first.
- Design the backend so GPU/Metal/Vulkan/CUDA acceleration can be added later without rewriting the feature.
- Use 16 kHz mono audio for Whisper.
- Add resampling if the selected microphone format differs.
- Handle device changes and microphone errors gracefully.
- Provide useful errors in Settings/UI if:
  - model path is missing
  - model cannot be loaded
  - microphone cannot be opened
  - backend initialization fails
  - transcription fails

Suggested architecture:

```text
voice/
  ptt/
    mod.rs
    settings.rs
    audio_capture.rs
    ring_buffer.rs
    target.rs
    transcript.rs
    collision.rs

  stt/
    mod.rs
    provider.rs
    local_whisper.rs
    cloud.rs
```

This structure is only a suggestion. Follow the existing project layout if it already has a better pattern.

---

## Target Routing

Implement target routing carefully.

When Push-to-Talk starts, capture the current target context:

```rust
enum PttTarget {
    AgentComposer,
    TerminalSlot { workspace_id: String, slot_id: String },
    ActiveTextInput { element_id: Option<String> },
    Clipboard,
    Unknown,
}
```

The exact shape should follow existing BLXCode types.

If the user selects `Remember target at Push-to-Talk start`, the transcript must still go to the original target even if focus changes while speaking.

Examples:

- User focuses BLXCode Agent composer
- Presses Push-to-Talk
- Clicks somewhere else while still holding the key
- Releases key
- Transcript still goes into the original Agent composer

Same for terminal:

- User focuses terminal slot 2
- Presses Push-to-Talk
- Focus changes
- Releases key
- Transcript still goes into terminal slot 2

Analyze how BLXCode currently tracks active terminal slots, Agent composer state, and focused inputs. Use the existing mechanisms instead of inventing a parallel state system.

---

## Collision Handling With Existing STT/TTS

Before implementing, inspect the existing BLXCode voice features.

Add a clear state machine so Push-to-Talk does not collide with existing voice input or TTS playback.

Possible states:

```rust
enum VoiceRuntimeState {
    Idle,
    RecordingPtt,
    TranscribingPtt,
    PlayingTts,
    AgentVoiceInputActive,
}
```

Required behavior:

- Starting Push-to-Talk while TTS is playing should either:
  - stop TTS
  - pause TTS
  - duck TTS
  - or reject recording with a visible hint
- Use a setting for this behavior if appropriate.
- Default should avoid feedback loops.
- Do not allow two microphone capture sessions at the same time.
- Do not let Agent TTS trigger Agent STT accidentally.
- Existing Agent STT/TTS behavior must continue working.

---

## Settings UI

Add a new Settings section:

```text
Settings → Voice → Push-to-Talk
```

or, if the existing settings layout has a better place:

```text
Settings → Push-to-Talk
```

The section should include:

- Enable Push-to-Talk
- Hotkey
- Mode: Local / Cloud
- Local backend: whisper.cpp
- Local model path
- Local quality preset:
  - Fast
  - Balanced
  - Best
- Cloud provider/model selection using existing provider patterns
- Insert target:
  - BLXCode Agent composer
  - Active terminal
  - Active text input
  - Clipboard
- Target mode:
  - Current focus
  - Remember target at PTT start
- Auto-submit toggle
- Partial transcript toggle
- TTS collision behavior:
  - Stop TTS on PTT
  - Pause TTS on PTT
  - Block PTT while TTS is speaking
- Microphone device selector if already supported
- Test button:
  - records a short sample
  - transcribes it
  - shows result without sending it anywhere

Use the existing Settings styling and components.

---

## i18n

Add all new UI strings to the existing i18n system.

Required locales:

- Add keys for all shipped locales.
- German should be properly translated.
- Other locales may follow the existing fallback pattern if that is how the project currently handles incomplete translations.
- Do not hardcode English text in UI components.

Suggested i18n keys:

```text
VoicePttTitle
VoicePttDescription
VoicePttEnable
VoicePttHotkey
VoicePttMode
VoicePttModeLocal
VoicePttModeCloud
VoicePttLocalBackend
VoicePttLocalModel
VoicePttQuality
VoicePttQualityFast
VoicePttQualityBalanced
VoicePttQualityBest
VoicePttCloudProvider
VoicePttCloudModel
VoicePttInsertTarget
VoicePttTargetAgent
VoicePttTargetTerminal
VoicePttTargetActiveInput
VoicePttTargetClipboard
VoicePttTargetMode
VoicePttTargetModeCurrentFocus
VoicePttTargetModeRememberStart
VoicePttAutoSubmit
VoicePttPartialTranscript
VoicePttTtsCollision
VoicePttTtsStop
VoicePttTtsPause
VoicePttTtsBlock
VoicePttTest
VoicePttRecording
VoicePttTranscribing
VoicePttTranscriptPreview
VoicePttModelMissing
VoicePttModelLoadFailed
VoicePttMicUnavailable
VoicePttBackendFailed
VoicePttInsertFailed
```

Adapt naming to existing conventions.

---

## Styling / Themes

Use existing BLXCode theme tokens.

Do not hardcode colors.

All new UI must support:

- light theme
- dark theme
- existing accent colors
- compact layouts
- responsive Settings panel width

Use existing design patterns:

- cards
- labels
- segmented controls
- select/dropdown components
- switches
- inline error states
- muted helper text
- mono/status text where appropriate

---

## Backend / Frontend Events

Use the existing Tauri command/event architecture.

Expected event types may include:

```text
PttRecordingStarted
PttPartialTranscript
PttFinalTranscript
PttRecordingStopped
PttError
PttStateChanged
```

Do not spam the frontend with too many partial events. Throttle/debounce partial transcript updates.

Keep the frontend responsive during recording and transcription.

---

## Performance Requirements

- Model loading must not happen on every recording.
- Recording start should feel instant.
- Use a ring buffer.
- Add optional pre-roll to avoid cutting off the first word.
- Transcription must run in a worker/background task.
- Avoid unnecessary WAV file writes.
- Prefer in-memory PCM buffers unless existing code requires temporary files.
- Avoid blocking Tauri IPC.
- Avoid holding locks during inference longer than necessary.

---

## Error Handling

Add clear, user-friendly error handling.

Examples:

- “No local Whisper model selected.”
- “Could not load Whisper model.”
- “Microphone is already in use.”
- “Push-to-Talk blocked while TTS is playing.”
- “Could not insert transcript into the original target.”
- “Cloud transcription provider is not configured.”

Errors should appear in the Push-to-Talk UI and, where appropriate, as existing BLXCode toasts/status messages.

---

## Tests

Add tests where practical.

Minimum expected coverage:

- settings serialization/deserialization
- default settings
- target routing behavior
- remembered target behavior
- collision state machine
- model path validation
- insert behavior selection
- i18n key presence if the project has such tests
- no regression to existing voice settings envelope

If the project has existing Rust unit tests for settings/events, extend them.

If frontend component tests are not used in the project, do not introduce a new test framework unnecessarily.

---

## Documentation

Update relevant documentation:

- user docs for Voice / Push-to-Talk
- settings documentation
- troubleshooting section:
  - model not found
  - microphone unavailable
  - slow transcription
  - cloud provider not configured
  - TTS/STT feedback loop prevention

Mention that local mode requires a Whisper-compatible model file.

---

## Acceptance Criteria

The implementation is complete when:

1. BLXCode has a Push-to-Talk settings section.
2. The user can enable local Push-to-Talk with `whisper.cpp`.
3. The local model loads once and stays warm.
4. Pressing the configured hotkey starts recording immediately.
5. Releasing the hotkey finalizes transcription.
6. Partial transcript can be shown while speaking if enabled.
7. Final transcript can be inserted into:
   - BLXCode Agent composer
   - active/remembered terminal
   - active text input
   - clipboard
8. Remembered target mode works even after focus changes.
9. Cloud mode is available through existing supported provider infrastructure where technically valid.
10. AWS Polly is not incorrectly used as an STT provider.
11. Existing Agent STT/TTS does not conflict with Push-to-Talk.
12. i18n keys are added.
13. Theme tokens are used, no hardcoded colors.
14. Existing voice features still work.
15. Rust tests are added/updated.
16. Documentation is updated.

---

## Implementation Rules

- Analyze before editing.
- Follow the existing BLXCode architecture.
- Keep changes modular.
- Avoid breaking existing provider settings.
- Avoid blocking the UI thread.
- Avoid hardcoded strings.
- Avoid hardcoded theme colors.
- Prefer safe Rust patterns.
- Keep platform-specific code isolated.
- Do not introduce large dependencies without justification.
- If a crate is added, explain why it is needed.
- If some part cannot be implemented cleanly because existing architecture is missing information, create a clear TODO with reasoning and implement the safe subset.
