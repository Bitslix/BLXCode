# Image Mode

BLXCode's agent panel can generate images directly from a prompt. Toggle **Image mode** in the chat header, type what you want, and the agent produces an image instead of a chat reply.

Image mode is available in the Tauri desktop app. It is not available in `trunk serve` mode because API keys and file writes are handled by the Tauri backend.

## Requirements

You need:

- An API key for the chosen image provider (OpenAI or OpenRouter).
- Network access to the provider.
- A selected workspace **if** you want generated images saved to disk. Without one the image lives in chat memory only.

Image keys piggyback on the existing agent provider keyring:

- **OpenAI** image generation uses the saved OpenAI key.
- **OpenRouter** image generation uses the saved OpenRouter key.

## Settings (Settings → Image)

| Setting | Default |
|---|---|
| Provider | OpenAI |
| Model | `gpt-image-1` |

Switch the provider with the OpenAI/OpenRouter buttons; pick a model from the suggestion list or type a custom id. The refresh button pulls the provider's catalog (filtered to image-shaped models).

OpenRouter uses chat-completions with `modalities: ["image"]`. OpenAI uses `/v1/images/generations` (text-only) or `/v1/images/edits` when one or more reference images are attached.

## Generating

1. Click the **image** icon in the agent chat header. The icon turns blue and a hint appears.
2. Optionally drop images into the panel to use as references (img2img).
3. Type a prompt and hit Enter. A non-empty prompt is required even when reference images are attached.
4. The result appears in the timeline as an inline image with a **Download** button.

When a workspace is set, the file is saved under:

```text
<workspace>/.blxcode/generated/<unix-ms>-<slug>.<ext>
```

Filenames collide-protect with a numeric suffix. The relative path is shown under the image so you can find it in your file manager.

## Voice + Image

If you submit an image-mode turn from voice (PTT or hotkey) **and** TTS is enabled in the Voice tab, BLXCode plays a short confirmation phrase (in your locale's voice) after the image arrives. The image content itself is not narrated.

## Limits

- Up to **4** reference images per turn, **8 MiB** each.
- Supported reference MIME types: PNG, JPEG, GIF, WebP.
- Generated previews larger than **20 MiB** are not rehydrated after a workspace reload — the original file on disk is still valid.

## Persistence

The chat timeline persists generated-image entries by their saved path, not by their base64 bytes, to keep `sessions.json` small. On reload, BLXCode lazily reads the file from disk for preview. If the file has been moved or deleted, the row remains in chat but the preview area stays empty.

## Troubleshooting

- **"No API key set for the image provider."** — Open Settings → Agent Provider and store a key for OpenAI or OpenRouter; image mode reuses those keys.
- **OpenRouter returns no image.** — Pick a model whose output modality includes `image` (Settings → Image → refresh, then choose one with "image" in the id).
- **Image saved but not visible after reload.** — Confirm the file at the path shown under the image still exists.
