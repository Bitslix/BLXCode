# Documentation Coverage Audit

Date: 2026-06-04

This audit tracks screenshot freshness, useful missing screenshots, and documentation coverage for the current changelog-driven documentation pass.

## Referenced Images That Still Look Old

These images are currently referenced from user docs and should be replaced with fresh captures because the surrounding feature changed materially.

| Priority | Image | Referenced from | Why replace |
|---|---|---|---|
| High | `settings-api-keys.png` | `docs/user/settings.md` | The API Keys pane now includes more text providers and media/search/voice rows. The current alt text still reflects older provider availability. |
| High | `settings-blxcode-agent.png` | `docs/user/settings.md` | The Agent settings pane now includes auto-compact, tool-loop limit, provider server URLs, agent orb mode, nickname/default role, and newer provider layout. |
| High | `create-workspace-step-2.png` | `docs/user/workspaces.md` | Step 2 now has per-terminal CLI-agent model and effort selection, plus OpenCode support. |
| High | `plan-manager.png` | `docs/user/plans.md` | Plans now have AI Plan/AI Tasks, status groups, Show in Kanban, per-plan folders, and diagram buttons. |
| High | `sidebar-explorer-git.png` | `docs/user/workspaces.md` | Sidebar now has VS Code-style Git commits, File Diff polish, status counts, and newer titlebar/statusline context. |
| High | `rules-panel.png` | `docs/user/rules-and-skills.md` | Rules now have category filters, live search, category chips, and updated descriptions. |
| High | `skills-panel.png` | `docs/user/rules-and-skills.md` | Skills now have category filters/search and are scoped to user/workspace skills. |
| High | `agent-image-preview.png` | `docs/user/image.md` | Image mode should show the modern Agent timeline/composer and reference-image flow. |
| Medium | `settings-terminal-hooks.png` | `docs/user/agent-providers.md` | Hooks are now also covered by the new App settings screenshot; this should become the hook install dialog or app log view, or be removed as duplicate. |
| Medium | `workspace-grid-2x2-claude.png` | `docs/user/workspaces.md` | The basic grid screenshot predates the new titlebar, status line, view-mode controls, and current Agent panel. |
| Medium | `workspace-grid-agent-extra-slots.png` | `docs/user/workspaces.md` | Named terminals and terminal-agent control changed the visual story; capture a named-terminal grid instead. |
| Medium | `terminal-handoff.png` | `docs/user/workspaces.md` | Handoff should show named terminal targets and Send to BLXCode Agent in the current titlebar/theme. |
| Medium | `workspace-home.png` | `docs/user/getting-started.md` | The default workbench now has the custom titlebar, pinned Kanban tab, status line, and newer right rail. |
| Medium | `create-workspace-connection-dropdown.png` | `docs/user/remote-ssh.md` | Remote settings are now master/detail cards; capture the newer remote picker and/or Remote settings pane. |
| Low | `workspace-center-tabs.png` | `docs/user/workspaces.md`, `docs/user/file-preview.md` | Still useful, but should eventually show Canvas/Swarm/Kanban-era center tabs. |
| Low | `memory-graph-handoff.png` | `docs/user/memory-and-tasks.md` | Still useful, but should be refreshed in the current centered Memory layout. |
| Low | `file-preview-*.png` | `docs/user/file-preview.md` | File preview now uses CodeMirror 6 in read-only mode; source/text preview screenshots should show the current editor surface and Vim status. |

## Unreferenced Old Assets

These are not currently referenced by docs and can be archived, deleted, or re-used after review:

- `agent-panel.png`
- `create-workspace-step-1.png`
- `memory-files.png`
- `memory-graph.png`
- `settings-agent-providers.png`
- `settings-app.png`
- `settings-appearance-themes.png`
- `settings-remote-ssh.png`
- `settings-voice-stt-tts.png`
- `welcome-screen.png`
- `workspace-resumed-agent-sessions.png`
- `hero-terminals.png`
- Timestamp-only May screenshots: `screenshot-2026-05-18_*.png`, `screenshot-2026-05-20_*.png`, `screenshot-2026-05-22_22-54-47.png`

## Useful Missing Screenshots

These features are documented in text but would benefit from fresh screenshots.

| Priority | Feature | Suggested capture |
|---|---|---|
| High | Settings -> MCP | Server list with stdio/HTTP transport, enabled switch, test result, and reset-session hint. |
| High | Settings -> HeartBeat | Interval, service toggles, Memory Indexer status, and Run now. |
| High | Settings -> Memory | Right-panel toggle, split view, memory pointers, architecture controls, and Memory Indexer stats. |
| High | Settings -> Code Editor | Vim toggle, disabled shortcut section while Vim owns the keymap, and editor shortcut rows. |
| High | Settings -> Voice / PTT | Push-to-Talk target/mode controls and local/cloud STT mode. |
| High | Whisper model manager | Model filters, download/resume progress, installed Use/Delete actions. |
| High | Settings -> API Keys | Current provider rows: OpenRouter, Anthropic, OpenAI, Hugging Face, Cloudflare, Together AI, Portkey, Tavily/Brave, fal.ai, AWS voice. |
| High | Settings -> BLXCode Agent | Current provider/model grid, auto-compact, tool-loop limit, Agent orb mode, nickname/default role. |
| High | Remote settings | Master/detail connection cards and editor view. |
| High | Notification feed | Titlebar bell with unread badge and deep-link notification items. |
| High | Update dialog | Structured release notes inside the update flow with Stable/Beta context. |
| High | App log panel | Recent log entries with level chips and metadata. |
| High | Hook install dialog | Missing/installed hooks modal separate from the App settings grid. |
| High | AI Plan / AI Tasks dialog | Prompt, generate-tasks toggle, loading shimmer, Markdown preview, Save/Regenerate. |
| High | Changed files card | Agent timeline card with additions/deletions and collapsible file tree. |
| High | Compact tasks bar | Collapsed and expanded task bar in the Agent panel. |
| Medium | Cache metrics popover | Cached-vs-fresh token breakdown on a message. |
| Medium | Context drag-and-drop | Drag a Project File, File Diff row, Git Commit, or terminal cell onto the Agent composer. |
| Medium | File Diff center viewer | Inline diff opened from the sidebar. |
| Medium | Git commit hover card | VS Code-style lanes plus expanded files and Open on GitHub hover/focus card. |
| Medium | Terminal drag-and-drop reorder | Source slot, target outline, and ghost preview. |
| Medium | Named terminals | Friendly terminal names plus rename/reset context menu. |
| Medium | Onboarding dialog | Display name and default session role on first launch. |
| Medium | 3D Drobo orb state | Recording/running/idle orb states, plus 2D fallback setting. |
| Medium | Fuzzy file finder | Open dialog and matching behavior. |
| Medium | Remote workspace | SSH badge in sidebar plus remote file/git behavior. |
| Medium | Image mode with references | Reference images attached and generated image row in modern timeline. |

## Changelog Coverage

### Git Log Coverage Check (`v0.3.3..HEAD`)

The git-log pass grouped user-facing commits into feature clusters and compared them against `docs/user/*`, `docs/developer/*`, `docs/releases/v0.5.0.md`, and `CHANGELOG.md`.

Result: every user-facing app feature introduced since `v0.3.3` has text documentation in at least one appropriate user/developer/release doc. The remaining issues are screenshot freshness and missing screenshots, not missing core topic documentation.

Covered git-log clusters:

- Agent providers, provider registry, Agent settings sections, API key catalog, tool-loop limit, auto-compact, context meter, 3D/2D orb, nickname, onboarding, session roles, Codewright/Branch Steward roles, and `git_conflicts`.
- MCP server management, MCP lifecycle/app-log integration, terminal-CLI MCP export, session-reset/reload semantics, and the dedicated MCP core skill.
- Terminal-agent model/effort selection, terminal-agent control tools, OpenCode support, prompt enhancement, named terminals, session capture, Claude usage capture, Grid/Canvas/Swarm terminal view modes.
- Agent timeline work: session stats, grouped tools, structured tool output, changed-files card, compact tasks bar, cached token metrics, Thinking preview, question card, and composer controls.
- Plans and Kanban: per-plan folders, automatic `PLANS.md`, AI Plan/AI Tasks, quick actions, collapsible status groups, Multi-Kanban, drag/drop, Kanban-to-agent context, and plan-linked Mermaid diagrams.
- Mermaid: `mermaid_create`, persisted/ad-hoc diagrams, inline timeline cards, gallery, provenance, stats, zoom, export, deletion, and plan-side cleanup.
- Settings and shell UI: App log, hook dialog/status, update channels, background update checks, post-update release notes, Help/About menu, notifications, status line, custom titlebar, Navigate menu.
- Appearance/theme work: redesigned BLXCode theme, BLXCode Legacy, 32 themes, roundings, font, global font size, and terminal font propagation.
- Memory: centered Memory panel, settings pane, right-panel default, architecture map, async architecture/indexer work, HeartBeat runtime, Memory Indexer, root note/category actions, category grouping, graph clustering, and split/tree behavior.
- Workspaces/sidebar/files/git: Project Files explorer, new file/folder inline creation, File Diff, center diff viewer, Git commit graph, fetch/pull/push, manual/AI commit, sidebar context drag/drop, fuzzy file finder, file preview/editor, CodeMirror 6, Vim mode, editor shortcuts, and Remote SSH parity.
- Voice/image: Push-to-Talk, local/cloud STT modes, Whisper model manager, TTS collision handling, voice provider layout, image mode/reference context, and modern Agent timeline interaction.
- Release/build/developer-only changes: release CI, updater signing/build flow, Remotion output cleanup, locale generation fixes, async/blocking-pool refactors, provider/tool-dispatch hardening, and module refactors are covered in release/developer docs or intentionally do not require a user guide section.

### Covered In Text And With A Current Screenshot

- Workspace Multi-Kanban and interactive Kanban drag/drop.
- Mermaid diagram gallery, inline diagram card, export buttons, zoom controls, and provenance stats.
- Agent session stats, context-window meter, grouped tool rows, tool-list output formatting, modern composer.
- Terminal CLI-agent question card.
- Welcome-screen Create Workspace action and recent workspace rows.
- Settings -> App hooks, app logging, Stable/Beta channel, and Help menu.
- Appearance roundings, font, font size, 32 themes, BLXCode redesign.
- Memory center tab architecture view and 3D graph clustering.
- Terminal view modes: Grid, Canvas, and Swarm.
- Claude usage status-line capture.

### Covered In Text, But Screenshot Is Missing Or Old

- New text providers and provider-specific setup: covered in `docs/user/agent-providers.md`; needs updated API Keys and Agent settings screenshots.
- MCP server support: covered in Settings, Agent Providers, developer harness, release notes; needs Settings -> MCP screenshot.
- HeartBeat and Memory Indexer: covered in Settings, Memory, developer architecture, release notes; needs Settings -> HeartBeat and Settings -> Memory screenshots.
- Push-to-Talk and Whisper model manager: covered in Voice and developer voice docs; needs PTT and model manager screenshots.
- AI Plan / AI Tasks: covered in Plans; needs dialog screenshot.
- Plan card quick actions, per-plan folders, collapsible groups: covered in Plans; old `plan-manager.png` should be replaced.
- Named terminals: covered in Workspaces; needs current named-terminal screenshot.
- Custom titlebar, Navigate menu, notification bell: covered in Workspaces/Settings; needs Navigate and notification-feed screenshots.
- VS Code-style Git commit graph and File Diff: covered in Workspaces/developer architecture; needs current sidebar/hover/diff screenshots.
- Rules/Skills/Plans filtering and category model: covered in Rules and Skills/Plans; old Rules/Skills screenshots should be replaced.
- Remote SSH master/detail: covered in Remote SSH user/developer docs; needs Remote settings screenshot.
- Code Editor Vim mode and shortcuts: covered in Settings/File Preview; needs Code Editor screenshot.
- File preview CodeMirror 6 replacement and highlight.js removal: covered in File Preview/release notes; needs fresh source-preview screenshot.
- Sidebar -> Agent context drag-and-drop: covered in Workspaces/Agent Providers; needs drag-overlay screenshot.
- In-app log, hook dialog, update dialog: covered in Settings/release notes; needs dedicated screenshots.
- Agent onboarding, nickname, default role: covered in Agent Providers/Workspaces/Settings; needs onboarding/settings screenshot.
- Cached token metrics, compact tasks bar, changed-files card: covered in Agent Providers/release notes; needs screenshots.

### Covered Mostly By Release Notes Or Developer Docs

These are backend, build, or internal-polish changes where user-facing docs are not always necessary:

- Architecture/memory indexing moved off the main thread.
- Backend Git/fs commands are async and blocking-pool backed.
- Tool dispatch, provider registry, subagent prompt/tool hardening, Azure tool-name sanitization.
- CI, release scripts, setup/build automation, dependency upgrades.
- Internal component/module splits, terminology refactors, and compile/test fixes.
- `src-remotion` tracking fix.

### Documentation Gaps To Close

- Canvas/Swarm are now documented in `docs/user/workspaces.md`, `docs/releases/v0.5.0.md`, and `CHANGELOG.md`.
- `docs/user/settings.md` documents the newer categories, but the API Keys and BLXCode Agent screenshots are stale.
- `docs/user/agent-providers.md` documents Changed files, compact tasks, cache metrics, and Thinking preview, but only grouped tools/tool-list output currently have screenshots.
- `docs/user/rules-and-skills.md` text covers filters/categories, but screenshots predate that UI.
- `docs/user/remote-ssh.md` text covers the master/detail redesign, but the screenshot still shows only the older Create Workspace connection dropdown.
- `docs/user/file-finder.md` has no screenshot.
- `docs/user/subagents.md` has no screenshot of the current subagent card/tool inventory UI.
- `docs/user/image.md` has one older image-mode screenshot and should be refreshed against the modern Agent timeline.

## Image Path Check

Current result: no missing `<img src="../images/...">` targets were found.
