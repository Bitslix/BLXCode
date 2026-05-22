# BLXCode Documentation

Welcome to the BLXCode docs. User guides explain how to run and use the app; developer guides explain how to build, extend, and contribute.

**Published copy:** [GitHub Wiki](https://github.com/Bitslix/BLXCode/wiki) (auto-synced from this folder on pushes to `main` that touch `docs/**`). Edit documentation here in the repository, not in the wiki web UI.

## Doc map

**Start here:** [Getting Started](user/getting-started.md) → [Workspaces](user/workspaces.md) → [Agent Harness](user/agent-harness.md)

| Topic | Guide |
|-------|--------|
| Settings (API Keys, BLXCode Agent, Workspace, **Appearance / themes**) | [Settings](user/settings.md) · [Appearance & Themes](user/appearance-themes.md) |
| Workbench, terminals, sidebar, handoff | [Workspaces](user/workspaces.md) |
| File preview (images, video, Markdown, Mermaid) | [File Preview](user/file-preview.md) |
| Memory, learnings, graph, categories | [Memory And Tasks](user/memory-and-tasks.md) |
| Markdown plans and plan-linked tasks | [Plans](user/plans.md) |
| Workspace rules and skills | [Rules And Skills](user/rules-and-skills.md) |
| Core skills, shell/git/web tools | [Agent Harness](user/agent-harness.md) |
| Coordinated subagents (scout/review/security) | [Subagents](user/subagents.md) |
| Tmux vs legacy shortcuts, notifications | [Keyboard Shortcuts](user/keyboard-shortcuts.md) |
| Image generation mode | [Image Mode](user/image.md) |
| Providers, API keys, context, hooks | [Agent Providers](user/agent-providers.md) |
| Voice STT/TTS | [Voice](user/voice.md) |
| UI language and EULA | [UI Language](user/language.md) |
| Build from source | [Building](user/building.md) |
| Common issues | [Troubleshooting](user/troubleshooting.md) |

## User docs

- [Getting Started](user/getting-started.md) — prerequisites, run BLXCode, first workspace, where data lives.
- [Settings](user/settings.md) — docked center-tab settings, API Keys, BLXCode Agent grid, Workspace, **Appearance / themes**.
- [Appearance & Themes](user/appearance-themes.md) — theme picker, presets, persistence, exceptions.
- [Workspaces](user/workspaces.md) — creation, terminal grids, sidebar explorer, Git graph, handoff, persistence.
- [File Preview](user/file-preview.md) — center-tab previews for images (incl. SVG), video, rendered Markdown, syntax-highlighted source code, and Mermaid diagrams. Repository policy docs (`LICENSE`, `CONTRIBUTING`, `SECURITY`, `CHANGELOG`, …) render as Markdown with a kind-specific hero banner — with or without a `.md` extension.
- [Memory And Tasks](user/memory-and-tasks.md) — Memory panel (Files, Graph, Search), dynamic categories, tasks, agent memory tools.
- [Plans](user/plans.md) — `.agents/plans/`, Kanban board, task syntax, Plans panel, agent tools.
- [Rules And Skills](user/rules-and-skills.md) — expandable rule/skill cards, core vs user skills, install dialog.
- [Agent Harness](user/agent-harness.md) — core skills, slim prompt, environment/shell/git/web, web API keys.
- [Subagents](user/subagents.md) — parallel runs, roles, timeline, tool groups, limits.
- [Keyboard Shortcuts](user/keyboard-shortcuts.md) — tmux prefix vs legacy chords, notification toasts.
- [Image Mode](user/image.md) — generate images from the agent panel, settings, limits, persistence.
- [Agent Providers](user/agent-providers.md) — OpenRouter, Anthropic, OpenAI-compatible, context, hooks.
- [Voice](user/voice.md) — STT, TTS, microphone, push-to-talk.
- [UI Language](user/language.md) — locales, language picker, EULA localization.
- [Building](user/building.md) — Linux, macOS, Windows release builds.
- [Troubleshooting](user/troubleshooting.md) — startup, build, browser, keyring, terminal issues.

## Developer docs

- [Setup](developer/setup.md) — local environment and verification commands.
- [Architecture](developer/architecture.md) — frontend/backend split, agent, memory, plans, handoff, diagrams.
- [Agent Harness](developer/agent-harness.md) — core skills, tool dispatch, web settings, extension guide.
- [Subagents](developer/subagents.md) — `subagents.run`, runner, protocol, tool groups, new roles.
- [Tauri IPC](developer/tauri-ipc.md) — command registration, wrappers, command groups.
- [Voice Architecture](developer/voice.md) — STT/TTS modules and flows.
- [Internationalization](developer/i18n.md) — locales, EULA content, translation workflow.
- [Themes](developer/themes.md) — tokens, `ThemeService`, adding themes, lint rules.
- [Contributing](developer/contributing.md) — code style, rules, testing, pull request checklist.

## Project principles

BLXCode is a local-first desktop workbench. Workspaces, terminals, memory, plans, tasks, and agent context stay close together without hiding where data lives on disk. When behavior changes, update the docs beside the code so users and contributors can move forward without reverse-engineering the app.
