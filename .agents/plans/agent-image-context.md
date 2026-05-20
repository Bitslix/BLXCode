# Agent Image Context Via Drag, Drop, And Paste

**Status:** implemented

## Summary

Add image intake to the BLXCode Agent panel so users can attach images by
global drag/drop or paste. Pending images are sent with the next user turn,
then marked as read so later text or voice turns do not process them again
unless the user manually reactivates them.

## Key Changes

- Extend the shared agent wire protocol with `image_context_items` on
  `UserTurn` and a mirrored `AgentImageContextItem` type carrying `id`,
  `label`, `mime`, `bytes_b64`, `size_bytes`, and `added_at`.
- Keep image context as frontend session state per workspace, not in the
  persisted workbench snapshot. Track frontend-only status as `Pending` or
  `Read`.
- Update the Agent Context section to show Memory context plus image context,
  with image status badges, remove controls, and a `Use again` action for
  read images.
- Submit only `Pending` images with the next Agent turn. After the backend
  confirms consumption, mark those images `Read`.

## Intake And UI

- Add an Agent Panel intake module for Tauri native file drops, DOM drops, and
  paste events.
- Native OS drops use Tauri `onDragDropEvent`; DOM drops and paste use
  `DataTransferItem.getAsFile()` / `ClipboardEvent.clipboardData`.
- Accept `image/png`, `image/jpeg`, `image/webp`, and `image/gif`.
- Enforce limits: 8 MiB per image, 4 pending images, and 16 MiB raw image data
  per turn.
- Make the whole Agent panel/chat area a visible drop zone:
  dashed highlighted border, subtle overlay, copy cursor, and centered text
  such as `Drop images to attach`.
- For unsupported drops, keep the highlight but show a rejection message such
  as `Only image files can be attached`.
- Clear the drop highlight on `drop`, `dragleave`, Tauri cancel, or Escape.

## Preview Dialog

- Clicking an image context row opens a modal dialog with a larger preview.
- The dialog shows label, MIME type, size, status, and actions: `Close` and
  `Remove`; read images also show `Use again`.
- `Remove` deletes the image from session context and closes the dialog if it
  was previewing that image.
- Escape, backdrop click, and the close button dismiss the dialog without
  changing context.

## Provider Rendering

- OpenAI/OpenRouter: render the user message as a content array with text
  first, then image Data URLs in `image_url` blocks.
- Anthropic: render image content blocks first, then the text block.
- Add `AgentEvent::ImageContextConsumed { ids }` so the frontend can mark
  images read after the provider request starts successfully.
- Sanitize persisted conversation history by replacing image blocks with a
  compact text marker after the turn; do not store or resend base64 image data
  in later turns.
- Keep images pending when a provider request fails before the stream starts,
  so the user can retry after fixing settings or switching model.

## Tools And Prompting

- Add client-side tools `image_context_list` and `image_context_detach` for
  visibility and cleanup.
- Do not add an agent tool to reactivate read images in v1; reuse is an
  explicit UI action.
- Update the system prompt to explain that pending images are automatically
  included with the next user turn and read images are not sent again.

## Test Plan

- Unit-test provider payload builders:
  OpenAI/OpenRouter produce text plus `image_url`; Anthropic produces image
  blocks plus text.
- Unit-test the conversation sanitizer so no base64 image data survives in
  persisted history.
- Unit-test validation for unsupported MIME types, oversized images, too many
  pending images, and per-turn size budget.
- Manually test drag over Agent Panel, unsupported drag, drop cancel, paste,
  preview dialog close/remove/use-again, and second text/voice turn not
  resending read images.
- Run `cargo check`, `cargo test -p blxcode`, and `trunk build`.

## Assumptions And Sources

- Session-only means no image bytes in snapshot, disk, timeline, or logs.
- Existing Memory context behavior stays path-only and is not converted to
  image context.
- Sources checked on 2026-05-20:
  Tauri `onDragDropEvent` file-drop API
  <https://tauri.app/reference/javascript/api/namespacewebviewwindow/>,
  MDN file drag/drop
  <https://developer.mozilla.org/en-US/docs/Web/API/HTML_Drag_and_Drop_API/File_drag_and_drop>,
  MDN `ClipboardEvent.clipboardData`
  <https://developer.mozilla.org/en-US/docs/Web/API/ClipboardEvent/clipboardData>,
  OpenAI Images/Vision
  <https://developers.openai.com/api/docs/guides/images-vision>,
  Anthropic Vision
  <https://platform.claude.com/docs/en/build-with-claude/vision>,
  OpenRouter image inputs
  <https://openrouter.ai/docs/guides/overview/multimodal/image-understanding>.
