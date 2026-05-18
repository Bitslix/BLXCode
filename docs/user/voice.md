# Voice: STT And TTS

BLXCode supports voice input and voice replies in the agent panel.

- **STT** means speech-to-text: BLXCode records your microphone, transcribes the audio, and inserts the transcript into the agent composer.
- **TTS** means text-to-speech: when a turn started from voice input finishes, BLXCode can synthesize the assistant's final answer and play it back.

Voice features are available in the Tauri desktop app. They are not available in frontend-only `trunk serve` mode because microphone capture, provider keys, and native cache paths are handled by the Tauri backend.

## Requirements

You need:

- A working system microphone.
- Microphone permission granted to BLXCode or the development shell.
- An API key for the configured voice provider.
- Network access to the configured STT/TTS provider.

Voice API keys reuse the existing agent provider key storage:

- OpenAI voice uses the saved OpenAI key.
- OpenRouter STT uses the saved OpenRouter key.

Keys are stored through the OS keyring when available, with the same private file fallback used by agent provider settings.

## Default Settings

The default voice settings are conservative:

| Setting | Default |
|---|---|
| STT provider | OpenAI |
| STT model | `gpt-4o-mini-transcribe` |
| Recording sample rate | `16000` Hz |
| TTS provider | OpenAI |
| TTS model | `gpt-4o-mini-tts` |
| TTS voice | `nova` |
| TTS autoplay | enabled |
| Post-STT behavior | auto-send |
| STT language | follow app locale |
| Push-to-talk hotkey | Space |

## Configure Voice

Open the voice settings tab in the right-side settings area.

<p align="center">
  <img src="../images/screenshot-2026-05-18_17-58-12.png" alt="Voice settings showing STT provider, model, recording quality, TTS model, and voice selection" />
</p>

You can configure:

- STT provider and model.
- Recording quality: low `16000`, standard `24000`, or high `48000` Hz.
- TTS model. The backend currently supports OpenAI for TTS.
- TTS voice.
- TTS voice gender filter.
- TTS autoplay on or off.
- Whether STT should auto-send or only fill a draft.
- STT language mode.
- Push-to-talk hotkey.

## STT Language Modes

BLXCode can send an optional language hint with transcription requests:

- **Follow app**: uses the current UI locale and reduces it to a primary ISO-639-1 language code, such as `de` from `de-DE`.
- **Auto detect**: sends no language hint and lets the provider detect speech language.
- **Manual**: sends the custom language code you enter.

## Recording From The Agent Panel

Use the voice orb in the agent panel:

- Hold the orb longer than a short threshold to record push-to-talk style; release to transcribe.
- Click quickly to toggle recording; click again to stop and transcribe.
- Press Space or Enter while the orb is focused to start/stop recording.
- Press Escape while recording to cancel.

The global push-to-talk hotkey also starts recording when enabled. A plain key such as Space is ignored while typing in editable fields, so normal text input remains safe.

## Auto-Send Versus Draft

When post-STT behavior is **auto-send**, BLXCode submits the transcript to the agent immediately.

When post-STT behavior is **draft**, BLXCode inserts the transcript into the compose field so you can edit it before sending.

## Voice Replies

When a prompt came from voice input and TTS is enabled, BLXCode synthesizes the final assistant answer after the model turn completes. The generated MP3 is sent back to the frontend as a `voice_ready` event and played in the agent panel.

Text answers still appear normally. If TTS fails, the text answer remains available and BLXCode reports the TTS error separately.

## Supported Providers

### STT

- OpenAI: `https://api.openai.com/v1/audio/transcriptions`
- OpenRouter: `https://openrouter.ai/api/v1/audio/transcriptions`

BLXCode sends WAV audio as multipart form data with `response_format=text`.

### TTS

TTS currently uses OpenAI's speech endpoint:

- OpenAI: `https://api.openai.com/v1/audio/speech`

OpenRouter TTS is not currently supported by the backend, even though OpenRouter can be used for STT.

## Voice Catalog

The OpenAI voice catalog currently exposed in BLXCode is:

| Voice | Gender Hint |
|---|---|
| `alloy` | neutral |
| `ash` | male |
| `ballad` | female |
| `coral` | female |
| `echo` | male |
| `fable` | neutral |
| `nova` | female |
| `onyx` | male |
| `sage` | female |
| `shimmer` | female |

The gender label is only a UI filtering hint.

## Privacy Notes

During recording, BLXCode writes a temporary WAV file under the app cache directory:

```text
<app-cache>/voice/<turn-id>.wav
```

After transcription finishes, BLXCode deletes the WAV file. Cancelled recordings are also removed. The audio is still sent to the selected remote STT provider for transcription, so use a provider and model whose data policy fits your workflow.
