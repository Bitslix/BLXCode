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

Voice API keys are set under **Settings → API Keys** (OpenAI, OpenRouter, AWS/Polly). The voice column in **Settings → BLXCode Agent** shows configured/missing status only.

- **OpenAI** — six OpenAI TTS voices selectable.
- **OpenRouter** — STT/TTS models; voice picks show OpenAI names disabled with a hint.
- **AWS** — six Polly voices when the AWS key is set.

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

Open **Settings** (center tab) → **BLXCode Agent** → **Voice** section.

**Settings → App** holds STT **language mode** and **push-to-talk** hotkey only.

You can configure:

- STT/TTS provider and models (shared provider dropdown).
- Recording quality: low `16000`, standard `24000`, or high `48000` Hz.
- TTS voice (fixed catalog per provider) and gender filter.
- TTS autoplay on or off.
- Whether STT should auto-send or only fill a draft.

See [Settings](settings.md).

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

## Push-to-Talk (PTT)

Push-to-Talk lets you hold a key, speak, and drop the transcript into a target
of your choice. It runs **local-first** with a warm `whisper.cpp` model, with an
optional cloud mode that reuses the existing STT providers.

### Enabling

1. Open **Settings → Voice** and turn on **Push-to-Talk**.
2. Choose **STT mode**:
   - **Local (whisper.cpp)** — on-device, private, no network. Pick a model in
     the model manager (see below) and a decode quality (Fast / Balanced / Best).
   - **Cloud** — uses OpenAI or OpenRouter transcription. (AWS Polly is **not**
     offered here — it is a text-to-speech service and cannot transcribe.)
3. Set the **insert target** and **target mode**, and optionally **auto-submit**
   and **live partial transcript**.

### Setting the hotkey

The PTT key is configured like every other shortcut, under
**Settings → Shortcuts → Push-to-Talk** (rebind / reset / conflict warning).
The default is **Ctrl+Shift+Space**. It is a *hold* key: recording starts on
key-down and finalizes on key-up. The hotkey is active while the BLXCode window
is focused.

### Targets

| Target | Behaviour |
|---|---|
| Agent composer | Inserts into the agent input; can auto-submit. |
| Active terminal | Writes into the focused terminal; auto-submit appends Enter. |
| Active text input | Inserts at the focused `<input>`/`<textarea>`. |
| Clipboard | Copies the transcript only. |

**Target mode** decides whether the destination follows the current focus, or is
**remembered at PTT start** (so a focus change while you speak is ignored).

### Live partial transcript

`whisper.cpp` has no native streaming, so live text is produced by periodically
re-decoding the audio captured so far. This is on by default and can be turned
off to save CPU. Partial transcript is only available in **local** mode.

### Collision with TTS

To avoid the microphone capturing the assistant's own voice, PTT checks whether
TTS is currently playing. The **While TTS is playing** setting chooses:
**Stop TTS**, **Pause TTS**, or **Block** push-to-talk (default).

### Local model manager

In local mode the model manager lists downloadable `whisper.cpp` models
(`ggerganov/whisper.cpp` on Hugging Face) with:

- filter tabs (All / Standard / Quantized / Turbo / Large),
- size, language (multilingual / EN-only), and Speed / Accuracy ratings,
- **Download** with a live progress bar, speed (MB/s) and **resumable** transfers
  (a paused/interrupted download shows **Resume**),
- **Installed** state with **Use** (select as the active model) and **Delete**.

Models are stored under `<app-data>/voice/models/<id>.bin`. Local mode requires a
downloaded Whisper-compatible model file.

> Local whisper is compiled behind the `local-whisper` build feature. Builds
> without that feature support cloud PTT only and report a clear error if a
> local model is used.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| "No local Whisper model selected." | Pick/Download a model in the model manager, then **Use** it. |
| "Could not load Whisper model." | The model file is missing or corrupt — delete and re-download. |
| "Microphone is already in use." | Another capture (agent voice orb) is active; release it first. |
| "Push-to-talk blocked while TTS is playing." | Change **While TTS is playing** to Stop or Pause, or wait for playback to finish. |
| Slow transcription | Use a smaller model (Tiny/Base/Q8) or a faster decode quality; large models need strong hardware. |
| "Cloud transcription provider is not configured." | Add the provider API key under Settings → API Keys. |
| TTS audio gets transcribed | Keep the default **Block** collision setting (prevents the feedback loop). |
| First word cut off | Speak a beat after pressing; the recorder includes a small lead-in but very fast starts can clip. |
