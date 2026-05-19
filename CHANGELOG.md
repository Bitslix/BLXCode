# Changelog

All notable changes to BLXCode are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- One-time dialog after the EULA asking whether to append `.blxcode/` to the workspace `.gitignore` (answer stored in `blxcode_gitignore_prompt_v1`; skipped in non-Tauri builds).
- Tauri command `workspace_ensure_agents` to create `.agents/memory/`, `.agents/learnings/`, and `_templates/` on workspace open.
- Tauri command `gitignore_append_blxcode` to add a `.blxcode/` entry when the user accepts the post-EULA prompt.

### Changed

- Workspace memory and learnings now live under `.agents/memory/` and `.agents/learnings/` (unified Memory API with `learnings/…` paths). Legacy `.blxcode/memory/` is migrated automatically when the new memory folder is empty. Existing learnings Markdown index links are upgraded to `[[wikilinks]]` for the memory graph.
- `.agents/` layout is bootstrapped when a workspace path is set (wizard commit, workspace switch, or workbench restore), not only when opening the Memory tab.
- Agent system prompt and memory tool descriptions reference `.agents/memory/` and `.agents/learnings/`.
- Agent memory pointer blocks list both memory and learnings roots.
- Memory export/import uses `memory/` and `learnings/` subdirectories.
- `scripts/render_i18n_locales_from_en.py` default mode translates only **missing** keys (not a full locale rewrite); use `--full` for the previous behavior, `--patch-english-matches` or `--keys` for English placeholder rows.

### Fixed

- i18n render script no longer rewrites locale files when nothing changed; prints guidance when zero rows are translated.

## [0.1.5] - 2026-05-19

### Added

- Session resume for hooked agent terminals (`sessions.json`) with captured titles on the terminal grid.
- Workspace and per-terminal completion notification badges with optional sound when background agents finish.
- Voice input and replies: microphone STT, OpenAI/OpenRouter transcription, and OpenAI TTS playback with a dedicated settings pane.
- Embedded browser navigation history and `browser_run_js` for running scripts in browser tabs.
- Workspace terminal improvements: multi-slot agent assignment, active/visibility controls, `pty_drain_wait`, terminal-wait-ready events, and agent launch retry.
- Workspace session UUIDs and notification pruning for agent tasks.
- User documentation: building guide, UI language guide, extra screenshots, and expanded README highlights.

### Changed

- First-run terms updated for MIT open-source distribution (MIT license acknowledgment, third-party API disclaimer); acceptance storage key bumped to `blxcode_eula_v2` so existing installs see the new text.
- Terminal layout and workspace grid observation for more responsive resizing.
- Workspace ID allocation and reset logic for stable session handling.

### Fixed

- Localization keys for unread workspace notifications.
- Potential deadlock when resetting workspace ID in terminal update closures.

## [0.1.0] - 2026-05-18

### Added

- Initial public release under the [MIT License](LICENSE).
- Native desktop shell (Tauri 2 + Leptos CSR) with multi-terminal workspaces, split panes, and persisted layout.
- Agent panel with OpenRouter, Anthropic, and OpenAI-compatible providers; sandboxed workspace file tools.
- **14-language UI** with compile-time translations and localized first-run terms.
- Workspace memory (`.blxcode/memory`), tasks (`.blxcode/tasks`), and memory graph view.
- Embedded browser with native child webviews on supported platforms and iframe fallback on Linux.
- Agent hooks for Claude, Codex, Gemini, OpenCode, and Cursor session/title capture.
- Command palette, Quick Open, drag-and-drop workspace reordering, and harness settings.
- EULA acceptance gate and platform app-config persistence.
