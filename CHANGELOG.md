# Changelog

All notable changes to BLXCode are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- One-time dialog after the EULA asking whether to append `.blxcode/` to the workspace `.gitignore` (answer stored in `blxcode_gitignore_prompt_v1`; skipped in non-Tauri builds).
- Tauri command `workspace_ensure_agents` to create `.agents/memory/`, `.agents/learnings/`, and `_templates/` on workspace open.
- Tauri command `gitignore_append_blxcode` to add a `.blxcode/` entry when the user accepts the post-EULA prompt.
- Release CI workflow: `authorize` job — only the repository owner may run release builds (`workflow_dispatch` or `v*` tag push); org repos may set Actions variable `RELEASE_OWNER`.
- Release CI **manual run**: choose build targets — **Alle**, **Linux (deb, rpm, AppImage)**, **Mac Universal**, or **Windows** (`plan` job sets the matrix). Tag pushes still build all platforms.
- Memory **Files** sidebar: collapsible **Memory** / **Learnings** groups (chevron toggle, collapsed by default). Group headers open the category index note (`README.md` / `MEMORY.md` or `learnings/LEARNINGS.md`) and list other notes when expanded.
- Memory **Search** view: dynamic category filter badges (pill style) between the query field and results — **All**, **Memory**, and **Learnings** with hit counts when both roots match; filters reset on a new query.
- Memory **Graph** view: lazy-loaded offline 3D graph powered by a local `3d-force-graph` / Three.js bundle, with the existing SVG graph retained as the 2D fallback.
- Memory **Graph** toolbar: icon-only controls for reset, zoom in/out, and 2D/3D mode switching; the selected graph mode is persisted locally.
- Memory **Graph** preview flow: clicking a node stays in the Graph tab, flies/focuses to the node, and opens a markdown preview popover with close, “Open in Files”, and in-preview wikilink navigation.
- Memory category context menu: right-click category headers to edit display name, category color, sidebar visibility, graph visibility, or send the category to the BLXCode Agent.
- Memory note context menu: right-click individual Memory/Learnings entries to open them or send that single note to the BLXCode Agent.
- BLXCode Settings → Memory: app-wide color preset management for Memory category colors, with add/edit/delete/reset controls.
- Agent Panel **Context** section: collapsed-by-default list of attached Memory/Learnings categories and notes, with per-item remove controls.

### Changed

- Memory file list no longer repeats the `learnings/` folder label on every row; notes are grouped under their category instead.
- Memory graph labels are cleaned up for display (`tanstack-start-api-routes` → `Tanstack Start API Routes`) in 3D, 2D fallback, and preview titles.
- Memory graph node colors now respect workspace category settings and graph visibility toggles.
- Memory graph spacing and interaction feel more physical: 3D nodes spread farther apart, links curve subtly, and connected lines wobble when nodes or links are dragged/released.
- BLXCode Agent turns now include attached Memory context as compact path metadata before the user prompt, leaving file contents to existing workspace read tools.
- Workspace memory and learnings now live under `.agents/memory/` and `.agents/learnings/` (unified Memory API with `learnings/…` paths). Legacy `.blxcode/memory/` is migrated automatically when the new memory folder is empty. Existing learnings Markdown index links are upgraded to `[[wikilinks]]` for the memory graph.
- `.agents/` layout is bootstrapped when a workspace path is set (wizard commit, workspace switch, or workbench restore), not only when opening the Memory tab.
- Agent system prompt and memory tool descriptions reference `.agents/memory/` and `.agents/learnings/`.
- Agent memory pointer blocks list both memory and learnings roots.
- Memory export/import uses `memory/` and `learnings/` subdirectories.
- Developer docs (`architecture`, `tauri-ipc`, `getting-started`, `memory-and-tasks`) updated for the `.agents/` memory layout, `workspace_ensure_agents`, and memory-flow diagrams.
- `scripts/render_i18n_locales_from_en.py` default mode translates only **missing** keys (not a full locale rewrite); use `--full` for the previous behavior, `--patch-english-matches` or `--keys` for English placeholder rows.

### Fixed

- Release CI `authorize` job reads optional `RELEASE_OWNER` via the GitHub API (`actions/github-script`) instead of `${{ vars.RELEASE_OWNER }}` in workflow expressions (avoids invalid context warnings when the variable is unset).
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
