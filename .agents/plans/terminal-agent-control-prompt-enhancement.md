# Terminal Agent Control & Prompt Enhancement

## Summary

Make BLXCode's built-in Agent able to reliably control terminal CLI agents end to end: create terminal slots, target running terminals by slot/name/agent slug, hand off prompts and attached context, read terminal output, wait for new responses, and interrupt stuck sessions. Add a core `prompt-generating` skill plus a composer toolbar toggle that enhances the user's draft through a separate one-shot AI request before sending it as the actual chat turn.

## Decisions

- Keep terminal control PTY-based for interactive CLI agents; headless CLI modes are documented as guidance, not the default control path.
- Supported terminal agent slugs remain `claude`, `codex`, `gemini`, `opencode`, and `cursor`.
- Add a reliable wait/observe tool instead of requiring agents to repeatedly peek at the rolling tail.
- `Enhance prompt before send` defaults off, is persisted per workspace, and auto-sends the enhanced prompt after generation succeeds.
- Prompt enhancement must run on a separate one-shot channel (`oneshot::complete_text` style), without normal chat history, tools, timeline events, or session mutation.
- If enhancement fails, restore the original draft and show an error; do not silently send the original.

## Implementation Notes

- Extend terminal harness tools with `harness.wait_terminal_output { slotId? | name? | agentSlug?, afterSeq?, timeoutMs?, idleMs?, maxBytes?, contains? }`, returning `{ sessionId, seq, bytes, text, timedOut }`.
- Add `harness.terminal_interrupt { slotId? | name? | agentSlug? }` for Ctrl+C against a targeted PTY session.
- Extend PTY backend state with a monotonic output sequence and last-output timestamp so wait/observe can detect new output and output-idle reliably.
- Keep `harness.read_terminal_output` as a non-destructive tail peek for quick inspection.
- Centralize terminal agent launch/resume profiles for `claude`, `codex`, `gemini`, `opencode`, and `cursor` so UI launch commands, docs, and skills do not drift.
- Update permission classification: submitted terminal input, submitted context handoff, wait/observe, and interrupt are terminal-control operations; `Ask Edits` prompts before submitted input/interrupt, `Plan` blocks submitted input/interrupt, and `Allow all` runs directly.
- Add embedded core skill `prompt-generating` and reference it from `CORE_SKILLS`, `system_prompt.rs`, `rules-skills.md`, and user/developer docs.
- Add composer toolbar toggle `Enhance prompt before send`; when enabled, `on_submit` calls new `agent_enhance_prompt` first, then submits the enhanced text as the `UserTurn.prompt`.
- `agent_enhance_prompt` should preserve intent, constraints, language, explicit commands, and security boundaries, while improving structure for BLXCode chat, terminal CLI agents, subagents, or user-facing responses.

## Tests

- Unit-test PTY output sequencing and wait behavior: new output, timeout, idle wait, `contains`, max-byte cap, and lossy UTF-8 output.
- Unit-test terminal target resolution for `slotId`, friendly `name`, and `agentSlug`, including missing/running states.
- Unit-test permission classification for `harness.wait_terminal_output`, `harness.terminal_interrupt`, submitted `harness.send_terminal_keys`, and submitted `harness.send_agent_context` across all Agent Chat modes.
- Unit-test `agent_enhance_prompt`: empty prompt rejection, language preservation, no scope broadening, fence stripping, and provider failure behavior.
- Manual Tauri smoke test: open shell plus `claude`/`codex`/`gemini`/`opencode`/`cursor`, list terminals, target by slot/name/slug, send prompt, wait for response, read tail, send context, interrupt a long-running command, and verify enhanced prompt is the submitted user turn.

## Tasks

- [ ] `terminal-profile-registry` - Centralize supported terminal agent launch and resume profiles
- [ ] `pty-output-sequencing` - Add PTY output sequence and last-output timestamp tracking
- [ ] `wait-terminal-output-tool` - Add `harness.wait_terminal_output` client/backend support
- [ ] `terminal-interrupt-tool` - Add `harness.terminal_interrupt` and permission handling
- [ ] `terminal-permission-tests` - Cover terminal-control permission classification across chat modes
- [ ] `prompt-generating-skill` - Add the core `prompt-generating` skill and system prompt references
- [ ] `prompt-enhance-command` - Add isolated `agent_enhance_prompt` one-shot generation command
- [ ] `prompt-enhance-toolbar` - Add per-workspace composer toggle and auto-send behavior
- [ ] `docs-terminal-agents` - Update harness/user/developer docs for CLI agent control patterns
- [ ] `tests-smoke` - Add automated tests and run manual Tauri terminal-agent smoke checks
