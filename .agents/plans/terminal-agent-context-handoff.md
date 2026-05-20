# Terminal Agent Context Handoff

**Status:** implemented

## Summary

Ensure the BLXCode Agent can pass its current context to terminal agent
sessions such as Claude, Codex, Gemini, OpenCode, or Cursor through a dedicated
toolcall. Context transfer is explicit, not automatic: the BLXCode Agent sends
context only when it chooses the new handoff tool for a selected terminal
session.

Images are handed off as metadata plus local exported file paths. BLXCode does
not depend on provider-specific image paste flows for terminal CLIs.

## Key Changes

- Add a semantic client-side tool `harness.send_agent_context`.
- Reuse the existing terminal target resolution from
  `harness.send_terminal_keys`: prefer `slotId`, otherwise allow a unique
  `agentSlug`.
- Render a terminal-safe Markdown context block containing workspace, attached
  Memory/Learnings/Rules context, image metadata, and optional user
  instruction.
- Send the rendered block through the existing PTY write path and submit it by
  default.
- Keep `harness.send_terminal_keys` as the low-level generic paste tool; use
  the new tool for context-aware delegation.

## Tool Interface

`harness.send_agent_context`

```json
{
  "slotId": "optional terminal slot id",
  "agentSlug": "optional agent slug when slotId is omitted",
  "instruction": "optional task or instruction appended after the context",
  "includeKinds": ["memory", "images"],
  "submit": true
}
```

- Exactly one target must resolve to a live terminal session.
- `includeKinds` defaults to both `memory` and `images`.
- `submit=false` writes the context block without pressing Enter.
- Ambiguous or missing `agentSlug` targets return a clear error and suggest
  `harness.list_terminals`.

## Context Rendering

- Use one shared renderer for the new tool so the prompt shape is stable and
  testable.
- Start the handoff with a clear header such as
  `BLXCode attached context for this terminal agent`.
- Include workspace root and current session target metadata.
- For Memory/Learnings/Rules, include titles, labels, and local paths; avoid
  unnecessarily duplicating large file bodies.
- For images, include label, MIME type, size, read/pending status, and exported
  local path.
- Keep the existing BLXCode image `Pending`/`Read` provider consumption state
  separate from terminal handoff state. Sending an image path to a terminal
  agent must not mark it as provider-consumed.

## Image Export

- On `harness.send_agent_context`, export selected image attachments to an
  app-managed workspace-local context directory, for example
  `.blxcode/agent-context/images/`.
- Use stable, sanitized filenames derived from image id and MIME extension.
- Write a small manifest next to the exports so sessions can inspect the
  handoff if needed.
- Do not write base64 image data into prompts, chat history, logs, or
  persisted conversation snapshots.

## Hooks And Environment

- Continue using existing hooks for title updates, session capture, resume, and
  notifications.
- Add terminal environment variables:
  - `BLX_AGENT_CONTEXT_DIR`
  - `BLX_AGENT_CONTEXT_MANIFEST`
- Hooks may read those paths, but hooks must not automatically inject context.
  The primary transport remains the explicit BLXCode toolcall through the PTY.

## Agent Behavior

- Update the BLXCode Agent system prompt/tool guidance:
  - Use `harness.list_terminals` before delegating context-aware work.
  - Prefer `harness.send_agent_context` over raw
    `harness.send_terminal_keys` when the terminal agent needs BLXCode context.
  - Use `slotId` when more than one session could match.
  - Do not broadcast large context blocks to multiple terminal agents unless
    the user explicitly asks for that.

## Test Plan

- Unit-test the context renderer for empty, memory-only, image-only, and mixed
  context.
- Unit-test image path export with MIME extension mapping, sanitized filenames,
  and paths containing spaces.
- Unit-test target resolution for `slotId`, unique `agentSlug`, ambiguous
  `agentSlug`, and missing terminals.
- Tool-test `submit=true` and `submit=false` behavior through the existing PTY
  write abstraction.
- Manual QA with Claude/Codex/Gemini/OpenCode terminals:
  context toolcall sends the block, image paths are readable, and existing
  hooks still capture sessions and notifications.

## Assumptions And Sources

- Context handoff is explicit via toolcall only, not automatic on terminal
  start or context changes.
- Images are handed off as metadata plus local paths.
- Existing PTY control is the most reliable cross-CLI transport for v1.
- Sources checked on 2026-05-20:
  Claude Code hooks
  <https://code.claude.com/docs/en/hooks>,
  Gemini CLI docs
  <https://google-gemini.github.io/gemini-cli/docs/cli/>,
  OpenCode plugins
  <https://opencode.ai/docs/plugins/>.
