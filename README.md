# BLXCode

[![License: MIT](https://img.shields.io/badge/license-MIT-f4a261?style=for-the-badge)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-2021-b7410e?style=for-the-badge&logo=rust&logoColor=white)
![Tauri](https://img.shields.io/badge/Tauri-2-24c8db?style=for-the-badge&logo=tauri&logoColor=white)
![Leptos](https://img.shields.io/badge/Leptos-0.8-ef3939?style=for-the-badge)
[![Languages](https://img.shields.io/badge/languages-14%20locales-5c6bc0?style=for-the-badge&logo=googletranslate&logoColor=white)](docs/user/language.md)
![Platforms](https://img.shields.io/badge/Linux%20%7C%20macOS%20%7C%20Windows-desktop-2f334d?style=for-the-badge)
![Status](https://img.shields.io/badge/status-ready-8a7cff?style=for-the-badge)

**BLXCode** is an open-source desktop workbench for running AI coding agents beside real terminals, project memory, Markdown plans, tasks, and an embedded browser. It is built with **Tauri 2**, **Rust**, **Leptos**, and **Trunk**.

The project is designed for people who want one focused local cockpit for agent-assisted development: create a workspace, assign terminal slots to tools such as Claude, Codex, Gemini, OpenCode, or Cursor, keep durable notes under `.agents/`, track work with plans and tasks, and talk to model providers from the same interface.

## Highlights

### Workbench

- **Native desktop shell** — Tauri 2 + Leptos 0.8 / WASM (Trunk).
- **Multi-terminal workspaces** — preset grids, split panes, session resume, hooks for Claude/Codex/Gemini/OpenCode/Cursor, persisted layout.
- **Sidebar** — resizable width; Project Files tree (hidden-dot toggle); **Git Commits** swim-lane graph; explorer/graph share a resizable bottom panel.
- **Keyboard shortcuts** — tmux-style `Ctrl+b` chords (default) or legacy direct chords; handoff success toasts and optional sounds.

### Plans, memory, and tasks

- **Plan Manager** — Markdown plans under `.agents/plans/`, `## Tasks` syntax, load-into-agent, status write-back to plan files.
- **Kanban board** (Plans) — per-workspace board across plan tasks, drag-and-drop columns and cards, Markdown writeback.
- **Memory** — dynamic categories under `.agents/memory/`, learnings, 2D/3D graph with category clustering, search filters, per-category colors, create category/note from the toolbar.
- **Tasks** — `.blxcode/tasks/` with plan-linked grouping in the agent panel.

### BLXCode Agent

- **Providers** — OpenRouter, Anthropic, OpenAI-compatible; thinking levels; OS keyring for API keys.
- **Better Harness** — slim system prompt + **11 core skills** (embedded docs via `skills_read`: memory, plans, shell, git, web, subagents, …); Skills panel **Core / User** tabs.
- **Coordinated subagents** — parallel `scout` / `review` / `security_analyst` runs on explicit request; inline timeline cards; shared provider settings (OpenRouter, OpenAI, Anthropic).
- **Server tools** — `environment_detect`, `shell_exec`, Git and workspace search/diff, optional **web search/fetch** (Tavily or Brave keys in settings).
- **Rules and skills** — `.agents/rules/` and `.agents/skills/` with install dialog (git/npm/local); expandable rule/skill cards and rule creation.
- **Image generation mode** — inline chat images, provider settings tab, workspace save under `.blxcode/generated/`.
- **Context** — memory/plans/tasks/images; drag-and-drop vision images; terminal **handoff** via `harness.send_agent_context`.
- **Voice** — STT, TTS, push-to-talk.

### Platform

- **14-language UI** — compile-time translations and localized EULA.
- **Setup scripts** — `scripts/setup/` for Linux, macOS, and Windows (Rust, WASM, Tauri deps, optional verify/build).
- **CI** — PR workflow runs `cargo check` for backend and `wasm32` frontend.

## What's new since 0.1.11

[0.1.11](CHANGELOG.md#0111---2026-05-20) added the sidebar Project Files tree and Git graph, tmux-style shortcuts, and handoff toasts.

**Shipped in [0.1.12](CHANGELOG.md#012---2026-05-20)–[0.1.14](CHANGELOG.md#0114---2026-05-21)**

- [Plan Manager](docs/user/plans.md), plan-linked tasks, resume checklist, handoff includes plans/tasks
- Sidebar resize, hidden files in explorer, Git commit swim-lanes
- [Image generation mode](docs/user/image.md) and Settings → Image tab
- Dynamic memory categories, new-note/category dialogs, graph clustering by category

**Agent harness and subagents** ([CHANGELOG Unreleased](CHANGELOG.md#unreleased) — merge pending)

- [Better Harness](docs/user/agent-harness.md): 11 core skills, slim system prompt, `environment_detect`, `shell_exec`, Git/workspace tools, Tavily/Brave web tools
- [Coordinated subagents](docs/user/subagents.md): `scout` / `review` / `security_analyst`, parallel timeline cards, tool-group sandboxing

**Also on current `main` / develop**

- Kanban board view for plan tasks; expandable Rules/Skills cards and rule creation
- Leptos **0.8**; `scripts/setup/` for Linux/macOS/Windows; PR [`cargo check`](.github/workflows/pr-check.yml) workflow

Details: [CHANGELOG.md](CHANGELOG.md) · [Documentation](docs/README.md)

## Internationalization

BLXCode ships **14 locales**; strings are checked at compile time. Change the language via **Ctrl+Shift+P** → **BLXCode settings** → **App** → **UI language** (or tmux: `Ctrl+b` then `:` → settings).

- User guide: [UI Language](docs/user/language.md)
- Contributor guide: [Internationalization](docs/developer/i18n.md)

## Status

BLXCode is early-stage open source. The workbench, plan/memory/task tooling, provider settings, Better Harness agent stack, and coordinated subagents are in active use on `main` / release branches; APIs and on-disk formats may still evolve. Current crate version: **0.1.14** (see [CHANGELOG.md](CHANGELOG.md) for unreleased work on feature branches).

## Screenshots

<p align="center">
  <img src="docs/images/hero-terminals.png" alt="BLXCode workbench with multi-agent terminal grid and right panel" width="920" />
</p>

| Workbench | Sidebar explorer and Git |
|---|---|
| <img src="docs/images/workspace-home.png" alt="Workspace sidebar and terminal grid" /> | <img src="docs/images/sidebar-explorer-git.png" alt="Project Files tree and Git Commits graph" /> |

| Plan Manager | Agent panel |
|---|---|
| <img src="docs/images/plan-manager.png" alt="Plans panel with task chips and Markdown editor" /> | <img src="docs/images/agent-panel.png" alt="BLXCode Agent with context and tasks" /> |

| Memory files and graph | Skills panel |
|---|---|
| <img src="docs/images/memory-files.png" alt="Memory Files with categories" /> | <img src="docs/images/skills-panel.png" alt="Skills panel and install dialog" /> |

<details>
<summary>More screenshots (setup, providers, voice)</summary>

<p align="center">
  <img src="docs/images/screenshot-2026-05-18_17-45-25.png" alt="Welcome screen with recent workspaces" width="720" />
</p>

| Workspace Setup | Agent Fleet |
|---|---|
| <img src="docs/images/screenshot-2026-05-18_17-45-40.png" alt="Create workspace layout" /> | <img src="docs/images/screenshot-2026-05-18_17-45-53.png" alt="Assign agents to terminals" /> |

| Provider Settings | Voice Settings |
|---|---|
| <img src="docs/images/screenshot-2026-05-18_17-58-05.png" alt="Agent provider settings" /> | <img src="docs/images/screenshot-2026-05-18_17-58-12.png" alt="Voice STT and TTS settings" /> |

</details>

## Quick Start

After cloning, run the setup script for your platform:

```bash
./scripts/setup/setup-linux.sh
./scripts/setup/setup-macos.sh
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts/setup/setup-windows.ps1
```

Use `--check-only` to inspect missing prerequisites without installing anything, or `--with-bundle` to run `cargo tauri build` after the default checks.

### Prerequisites

- Rust stable and Cargo.
- `wasm32-unknown-unknown` Rust target.
- Trunk.
- Tauri system dependencies for your OS.
- Cargo Tauri CLI.

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
```

On Linux, install the WebKitGTK and build dependencies required by Tauri 2 for your distribution.

### Run The App

```bash
cargo tauri dev
```

The Tauri dev command starts Trunk automatically through `src-tauri/tauri.conf.json`. The frontend serves on `http://localhost:1420`.

> **First-build tip:** Tauri's `devUrl` connection has a hard 180-second timeout. The cold WASM build can exceed that on slower machines. If `cargo tauri dev` fails with *"Could not connect to `http://localhost:1420/` after 180s"*, warm the Trunk cache once, then re-run:
>
> ```bash
> trunk build
> cargo tauri dev
> ```

### Build

```bash
cargo tauri build
```

### Release Automation

Use the release scripts for local bundle builds, version bumps, changelog finalization, tags, and GitHub release uploads:

```bash
./scripts/release.sh
./scripts/release-macos.sh
```

```powershell
scripts\release.cmd
powershell -ExecutionPolicy Bypass -File scripts/release.ps1 --platform windows
```

Release scripts are unsigned by default for local builds. Copy `.env.release.example` to `.env.release` only when you need signing keys, notarization credentials, or GitHub upload overrides.

### Useful Checks

```bash
cargo test --workspace
cargo check -p blxcode
cargo check -p blxcode-ui --target wasm32-unknown-unknown
trunk build
```

## Documentation

Full index: [Documentation Home](docs/README.md)

**User guides**

- [Getting Started](docs/user/getting-started.md)
- [Workspaces](docs/user/workspaces.md)
- [Memory And Tasks](docs/user/memory-and-tasks.md)
- [Plans](docs/user/plans.md)
- [Rules And Skills](docs/user/rules-and-skills.md)
- [Agent Harness](docs/user/agent-harness.md)
- [Subagents](docs/user/subagents.md)
- [Keyboard Shortcuts](docs/user/keyboard-shortcuts.md)
- [Image Mode](docs/user/image.md)
- [Agent Providers](docs/user/agent-providers.md)
- [Voice](docs/user/voice.md)
- [UI Language](docs/user/language.md)
- [Building](docs/user/building.md)
- [Troubleshooting](docs/user/troubleshooting.md)

**Developer guides**

- [Developer Setup](docs/developer/setup.md)
- [Architecture](docs/developer/architecture.md)
- [Agent Harness](docs/developer/agent-harness.md)
- [Subagents](docs/developer/subagents.md)
- [Tauri IPC](docs/developer/tauri-ipc.md)
- [Voice Architecture](docs/developer/voice.md)
- [Internationalization](docs/developer/i18n.md)
- [Contributing](docs/developer/contributing.md)

## Repository Layout

```text
.
├── src/                 # Leptos CSR frontend crate: blxcode-ui
├── src-tauri/           # Tauri 2 backend crate: blxcode
├── content/             # EULA markdown and bundled agent hook scripts
├── src-tauri/src/agent/harness_skills/  # Embedded core skill Markdown (Better Harness)
├── public/              # Static frontend assets copied by Trunk
├── scripts/             # Maintainer scripts
├── docs/                # User and developer documentation
├── Cargo.toml           # Workspace + frontend crate manifest
├── Trunk.toml           # Frontend build/dev server config
└── styles.css           # Global app styling
```

## Configuration

Most user-facing configuration is managed in the app UI and persisted in platform app config/data directories. Workspace-local data:

```text
<workspace>/.agents/memory/
<workspace>/.agents/learnings/
<workspace>/.agents/plans/
<workspace>/.agents/rules/
<workspace>/.agents/skills/
<workspace>/.blxcode/tasks/
<workspace>/.blxcode/generated/       # image mode output
<workspace>/.blxcode/agent-context/   # handoff exports
```

API keys are stored through the OS keyring when possible, with a private file fallback under the app config directory.

## Contributing

Contributions are welcome. Start with [Developer Setup](docs/developer/setup.md) and [Contributing](docs/developer/contributing.md).

Conventions:

- Keep the frontend (`blxcode-ui`) and Tauri backend (`blxcode`) boundaries clear.
- Register every Tauri command in `src-tauri/src/lib.rs` and add wrappers in `src/tauri_bridge.rs`.
- Prefer focused modules over monolithic files.
- Add or update docs when user-facing behavior changes.
- Run relevant checks before opening a pull request.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes.

## License

BLXCode is released under the [MIT License](LICENSE).
