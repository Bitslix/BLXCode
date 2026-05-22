# Voice Architecture

The voice subsystem adds speech-to-text, text-to-speech, microphone recording, and voice settings on top of the existing agent panel.

## Backend Modules

Voice backend code lives under `src-tauri/src/voice/`.

| File | Responsibility |
|---|---|
| `mod.rs` | Module exports and subsystem overview. |
| `commands.rs` | Tauri command surface for recording, transcription, settings, voices, and preview. |
| `recorder.rs` | `cpal` microphone capture into temporary WAV files. |
| `settings.rs` | Persistent voice settings stored inside `agent_provider_settings.json`. |
| `stt.rs` | OpenAI/OpenRouter audio transcription HTTP client. |
| `tts.rs` | OpenAI speech HTTP client returning MP3 bytes. |
| `voices.rs` | Static TTS voice catalog with gender hints. |

`src-tauri/src/lib.rs` registers `VoiceRecorderState` as managed state and registers every voice command in `generate_handler![]`.

## Frontend Modules

Voice frontend code lives in:

- `src/workbench/agent_panel/voice_orb/`: microphone orb, recording state machine, playback, and push-to-talk hotkey.
- `src/workbench/harness_voice_pane/mod.rs`: voice settings UI.
- `src/tauri_bridge.rs`: voice settings structs and typed command wrappers.
- `src/agent_wire.rs`: `voice_input` on `UserTurn` and `voice_ready` on `AgentEvent`.

## Settings Model

Voice settings are stored as a `voice` sub-object in the same JSON envelope as agent provider settings:

```text
<app-config>/agent_provider_settings.json
```

The voice settings module deserializes the file as `serde_json::Value`, updates only the `voice` object, and round-trips the other settings untouched.

Default settings:

- STT provider: OpenAI.
- STT model: `gpt-4o-mini-transcribe`.
- STT sample rate: `16000`.
- TTS provider: OpenAI.
- TTS model: `gpt-4o-mini-tts`.
- TTS voice: `nova`.
- TTS enabled: `true`.
- Post-STT flow: `AutoSend`.
- STT language: `FollowApp`.
- PTT hotkey: Space.

Voice provider keys piggyback on `agent_settings::provider_key_pub`, so OpenAI voice uses the OpenAI keyring entry and OpenRouter STT uses the OpenRouter keyring entry.

## Recording Flow

1. The user starts the voice orb or configured push-to-talk hotkey.
2. The frontend calls `voice_start_recording(sampleRateHz)`.
3. The backend creates a UUID turn ID and starts `cpal` capture from the default input device.
4. Audio is downmixed to mono and resampled to the configured target rate.
5. Samples are written as 16-bit PCM WAV under `<app-cache>/voice/`.
6. The frontend stops recording with `voice_stop_and_transcribe(turnId, localeHint)`.
7. The backend finalizes the WAV, sends it to STT, deletes the WAV, and returns transcript text.

Cancelling calls `voice_cancel_recording(turnId)`, stops the worker, and removes the temporary WAV.

## STT Flow

`stt::transcribe_wav` posts multipart form data to:

- OpenAI: `https://api.openai.com/v1/audio/transcriptions`
- OpenRouter: `https://openrouter.ai/api/v1/audio/transcriptions`

The request includes:

- `model`.
- `file` as `audio/wav`.
- `response_format=text`.
- Optional `language` reduced to a primary ISO-639-1 code.

Responses are parsed as JSON first for compatibility with providers that still return `{ "text": "..." }`, then as raw text.

## TTS Flow

TTS runs only for turns that originated from voice input. The agent panel marks the next `UserTurn` with `voice_input=true` after a successful STT transcript.

After the model turn finishes, `session_orchestrator::maybe_emit_tts`:

1. Loads voice settings.
2. Skips work if TTS is disabled.
3. Reads the final assistant text from conversation state.
4. Resolves the TTS provider key.
5. Calls `tts::synthesize`.
6. Pushes `AgentEvent::VoiceReady { audio_b64, mime }`.

The frontend converts the base64 MP3 into a Blob URL and plays it through an `<audio>` element.

TTS currently supports OpenAI only. If another provider is selected, `tts::synthesize` returns an unsupported-provider error.

## Voice Tauri Commands

- `voice_start_recording`
- `voice_stop_and_transcribe`
- `voice_cancel_recording`
- `voice_settings_get`
- `voice_settings_save`
- `voice_tts_preview`

Keep new command arguments owned and serializable. Validate provider/model assumptions on the backend, not only in the settings UI.

## Failure Behavior

- Missing microphone: `voice_start_recording` returns an error.
- Failed STT: the temporary WAV is still removed and the error is returned to the frontend.
- Failed TTS: the text answer remains available and an `AgentEvent::Error` is queued.
- Missing key: key resolution fails through the shared provider key lookup.

## Dependencies

Voice currently depends on:

- `cpal` for cross-platform audio input.
- `hound` for WAV writing.
- `uuid` for recording turn IDs.
- `reqwest` multipart support for STT uploads.

