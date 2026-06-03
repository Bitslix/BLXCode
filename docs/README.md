# BLXCode Documentation

Welcome to the BLXCode docs. User guides explain how to run and use the app; developer guides explain how to build, extend, and contribute.

**Published copy:** [GitHub Wiki](https://github.com/Bitslix/BLXCode/wiki) (auto-synced from this folder on pushes to `main` that touch `docs/**`). Edit documentation here in the repository, not in the wiki web UI.

## Doc map

**Start here:** [Getting Started](user/getting-started.md) → [Workspaces](user/workspaces.md) → [Agent Harness](user/agent-harness.md)

| Topic | Guide |
|-------|--------|
| Settings (API Keys, **BLXCode Agent (text/image/voice, MCP, HeartBeat, Memory, Code Editor)**, Workspace, **Appearance / themes**, Help/About, App log) | [Settings](user/settings.md) · [Appearance & Themes](user/appearance-themes.md) |
| Workbench, terminals, sidebar, Git diff/sync, handoff, **app status line**, **sidebar context drag-and-drop** | [Workspaces](user/workspaces.md) |
| Remote workspaces over SSH (connections, terminals, file/git, resume) | [Remote (SSH)](user/remote-ssh.md) |
| File preview (images, video, Markdown, Mermaid, **Vim mode**) | [File Preview](user/file-preview.md) |
| Memory, learnings, architecture map, graph, **Memory Indexer (HeartBeat)**, **Memory settings pane** | [Memory And Tasks](user/memory-and-tasks.md) |
| Markdown plans, plan-linked tasks, **Multi-Kanban**, **Mermaid diagram gallery** | [Plans](user/plans.md) |
| Workspace rules and skills | [Rules And Skills](user/rules-and-skills.md) |
| Core skills (incl. **MCP**, **prompt-generating**), shell/git/web tools | [Agent Harness](user/agent-harness.md) |
| Coordinated subagents (scout/review/security) | [Subagents](user/subagents.md) |
| Tmux vs legacy shortcuts, **Create Workspace**, **Push-to-Talk** | [Keyboard Shortcuts](user/keyboard-shortcuts.md) |
| Image generation mode | [Image Mode](user/image.md) |
| Providers, API keys, context, hooks, **MCP servers**, **agent nickname**, **onboarding** | [Agent Providers](user/agent-providers.md) |
| Voice STT/TTS, **Push-to-Talk (local Whisper / cloud)** | [Voice](user/voice.md) |
| UI language and EULA | [UI Language](user/language.md) |
| Build from source | [Building](user/building.md) |
| Common issues | [Troubleshooting](user/troubleshooting.md) |

## User docs

- [Getting Started](user/getting-started.md) — prerequisites, run BLXCode, first workspace, where data lives, **welcome-screen Create Workspace**, per-agent model/effort fleet.
- [Settings](user/settings.md) — docked center-tab settings, **API Keys, BLXCode Agent (text/image/voice), MCP, HeartBeat, Memory, Code Editor (Vim)**, Workspace, **Appearance / themes**, Help/About, App log, Notifications.
- [Appearance & Themes](user/appearance-themes.md) — theme picker, presets, persistence, exceptions, **font size**.
- [Workspaces](user/workspaces.md) — creation, terminal grids, sidebar explorer, File Diff (stage/commit/push), Git graph (fetch/pull), handoff, persistence, **app status line**, **sidebar context drag-and-drop (Files / Folders / Diffs / Commits)**, **hook install dialog**, **named terminals**.
- [Remote (SSH)](user/remote-ssh.md) — Settings → Remote connection presets (password / key / agent, encrypted secrets), creating remote workspaces, remote terminals + file/git + session resume (tmux vs keepalive).
- [File Preview](user/file-preview.md) — center-tab previews for images (incl. SVG), video, rendered Markdown, syntax-highlighted source code, and Mermaid diagrams. **Vim mode** via `@replit/codemirror-vim`. Repository policy docs (`LICENSE`, `CONTRIBUTING`, `SECURITY`, `CHANGELOG`, …) render as Markdown with a kind-specific hero banner — with or without a `.md` extension.
- [Memory And Tasks](user/memory-and-tasks.md) — Memory panel (Files, Graph, Search), architecture map, dynamic categories, tasks, agent memory tools, **Memory Indexer (HeartBeat)**, **Memory settings pane**.
- [Plans](user/plans.md) — `.agents/plans/<slug>/plan.md`, auto-maintained `PLANS.md` index, **workspace Multi-Kanban**, **Mermaid diagram gallery**, task syntax, Plans panel, agent tools.
- [Rules And Skills](user/rules-and-skills.md) — expandable rule/skill cards, core vs user skills, install dialog.
- [Agent Harness](user/agent-harness.md) — core skills (incl. **MCP**, **prompt-generating**), slim prompt, environment/shell/git/web, web API keys.
- [Subagents](user/subagents.md) — parallel runs, roles, timeline, tool groups, limits.
- [Keyboard Shortcuts](user/keyboard-shortcuts.md) — tmux prefix vs legacy chords, notification toasts, **Create Workspace**, **Push-to-Talk**.
- [Image Mode](user/image.md) — generate images from the agent panel, settings, limits, persistence.
- [Agent Providers](user/agent-providers.md) — OpenRouter, Anthropic, OpenAI-compatible + Ollama / LM Studio / Hugging Face / Cloudflare / Together / Portkey / custom, **MCP servers**, **agent nickname**, **onboarding**, **sidebar context kinds**.
- [Voice](user/voice.md) — STT, TTS, microphone, **Push-to-Talk (local Whisper / cloud)**.
- [UI Language](user/language.md) — locales, language picker, EULA localization.
- [Building](user/building.md) — Linux, macOS, Windows release builds.
- [Troubleshooting](user/troubleshooting.md) — startup, build, browser, keyring, terminal issues.

## Developer docs

- [Setup](developer/setup.md) — local environment and verification commands.
- [Architecture](developer/architecture.md) — frontend/backend split, agent, memory, plans, handoff, diagrams, **HeartBeat**, **Memory Indexer**, **MCP**, **Kanban**, **Mermaid**, **Notifications**, **App log**, **App status line**, **sidebar context drag-and-drop**.
- [Agent Harness](developer/agent-harness.md) — core skills (incl. **MCP**, **prompt-generating**), tool dispatch, web settings, text-provider registry, **MCP module**, extension guide.
- [Subagents](developer/subagents.md) — `subagents.run`, runner, protocol, tool groups, new roles.
- [Tauri IPC](developer/tauri-ipc.md) — command registration, wrappers, command groups, **HeartBeat / Memory Indexer / MCP / Kanban / Mermaid / Notifications / App log** commands.
- [SSH Remote Transport](developer/ssh-remote.md) — wrapped-`ssh` terminals, persistent exec channel, fs/git remote routing, secrets, resume, teardown, russh follow-up.
- [Voice Architecture](developer/voice.md) — STT/TTS modules and flows.
- [Internationalization](developer/i18n.md) — locales, EULA content, translation workflow.
- [Themes](developer/themes.md) — tokens, `ThemeService`, adding themes, lint rules.
- [Contributing](developer/contributing.md) — code style, rules, testing, pull request checklist.

## Project principles

BLXCode is a local-first desktop workbench. Workspaces, terminals, memory, plans, tasks, and agent context stay close together without hiding where data lives on disk. When behavior changes, update the docs beside the code so users and contributors can move forward without reverse-engineering the app.

## Release notes

User-facing notes for each version live in [`docs/releases/`](releases/) (for example [`v0.5.0.md`](releases/v0.5.0.md)). Prereleases use their exact tag filename as well, such as `v0.5.1-pre.ed4dc.md`. They power the in-app **What's new** dialog after updates and should stay non-technical. The technical changelog remains in [`CHANGELOG.md`](../CHANGELOG.md) at the repository root.
