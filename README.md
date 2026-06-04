<p align="center">
  <img src="public/blxcode.png" alt="BLXCode" width="96" />
</p>

<h1 align="center">BLXCode</h1>

<p align="center">
  <strong>Local-first desktop workbench for AI-assisted development</strong><br/>
  Terminals · Agent · MCP · Memory · Plans · Kanban · Git — in one Tauri shell
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-f4a261?style=for-the-badge" alt="MIT License" /></a>
  <a href="CHANGELOG.md"><img src="https://img.shields.io/badge/version-0.5.0-8a7cff?style=for-the-badge" alt="Version 0.5.0" /></a>
  <img src="https://img.shields.io/badge/Rust-2021-b7410e?style=for-the-badge&logo=rust&logoColor=white" alt="Rust 2021" />
  <img src="https://img.shields.io/badge/Tauri-2-24c8db?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri 2" />
  <img src="https://img.shields.io/badge/Leptos-0.8-ef3939?style=for-the-badge" alt="Leptos 0.8" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Linux%20%7C%20macOS%20%7C%20Windows-desktop-2f334d?style=for-the-badge&logo=linux&logoColor=white" alt="Linux, macOS, Windows" />
  <a href="docs/user/language.md"><img src="https://img.shields.io/badge/languages-14%20locales-5c6bc0?style=for-the-badge&logo=googletranslate&logoColor=white" alt="14 locales" /></a>
  <a href="docs/user/appearance-themes.md"><img src="https://img.shields.io/badge/themes-32%20presets-e06c75?style=for-the-badge&logo=materialdesignicons&logoColor=white" alt="32 themes" /></a>
  <a href=".github/workflows/pr-check.yml"><img src="https://img.shields.io/badge/CI-cargo%20check-3fb950?style=for-the-badge&logo=githubactions&logoColor=white" alt="CI" /></a>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> ·
  <a href="#features">Features</a> ·
  <a href="#screenshots">Screenshots</a> ·
  <a href="docs/README.md">Documentation</a> ·
  <a href="CHANGELOG.md">Changelog</a> ·
  <a href="#community">Community</a> ·
  <a href="https://github.com/Bitslix/BLXCode/wiki">Wiki</a>
</p>

---

**BLXCode** is an open-source desktop workbench for running AI coding agents beside real terminals, MCP tools, project memory, Markdown plans, tasks, diagrams, Git, and an embedded browser. Built with **Tauri 2**, **Rust**, **Leptos**, and **Trunk**.

Create a workspace, assign terminal slots to Claude, Codex, Gemini, OpenCode, or Cursor, connect local or remote MCP servers, keep durable notes under `.agents/`, track work with plans and a Kanban board, generate Mermaid diagrams, preview and diff files in the center pane, and talk to model providers from the same interface — all local-first, with data you can inspect on disk.

## Features

### Workbench

| | |
|---|---|
| 🖥️ **Native shell** | Tauri 2 + Leptos 0.8 WASM (Trunk) on Linux, macOS, and Windows |
| 📑 **Center tabs** | VS Code–style tab strip: Terminals, file preview, diff viewer, Settings |
| 🧩 **Multi-terminal grids** | Preset layouts, split panes, drag-and-drop slot reorder, session resume |
| 📂 **Sidebar** | Project Files (new file/folder), **File Diff** (stage, commit, push), Git graph (fetch/pull) |
| 🔔 **Titlebar & status** | Custom cross-platform titlebar, notification feed, process status line, Navigate menu |
| ⌨️ **Shortcuts** | tmux-style `Ctrl+b` chords, command palette actions, Vim editor mode |
| 🎨 **32 themes** | Redesigned BLXCode, BLXCode Legacy, Dracula, Catppuccin, Nord, light variants, and more |

### Files & Git

| | |
|---|---|
| 👁️ **Rich preview** | Images, video, Markdown, Mermaid, and CodeMirror 6 code preview/editing |
| 📜 **Policy docs** | `LICENSE`, `README`, `CONTRIBUTING`, `SECURITY` — rendered with hero banners |
| 🔀 **Diff viewer** | Unified diffs in center tabs; commit (optional AI message) from the toolbar |
| 🌿 **Git sync** | Fetch, pull, and push from the sidebar; VS Code–style graph; live status watcher |
| ✂️ **Code handoff** | Drag-select line ranges → insert into terminal or attach to agent |

### Plans, memory & tasks

| | |
|---|---|
| 📋 **Plan Manager** | Markdown plans under `.agents/plans/`, auto `PLANS.md` index, load-into-agent |
| 📊 **Kanban board** | Pinned workspace board with plan/task lanes, search, DnD, Markdown write-back |
| 🧭 **Mermaid diagrams** | Agent-authored diagrams persisted with plans; gallery view; Markdown/PDF export |
| 🧠 **Memory** | Categories, learnings, architecture map, HeartBeat Memory Indexer, 2D/3D graph |
| ✅ **Tasks** | AI-authored plans/tasks, `.blxcode/tasks/`, plan-linked grouping in the agent panel |

### BLXCode Agent

| | |
|---|---|
| 🤖 **Providers** | OpenRouter, Anthropic, OpenAI, Ollama, LM Studio, Hugging Face, Cloudflare, Together, Portkey |
| 🔌 **MCP tools** | stdio/HTTP MCP servers for the in-app agent and terminal CLIs with managed project configs |
| 🛠️ **Core skills** | Slim system prompt + bundled skills; shell, git, workspace search, web tools |
| 🔍 **Subagents** | Parallel `scout` / `review` / `security_analyst` runs with timeline cards |
| 🖼️ **Image mode** | Inline chat images; fal.ai support; output under `.blxcode/generated/` |
| 🎙️ **Voice** | STT, TTS, push-to-talk, local Whisper; OpenAI, OpenRouter, AWS Polly |
| 📊 **Turn metrics** | Per-row tokens, TTFT, decode speed, context meter, compaction, session cost |
| ❓ **Ask user** | Multiple-choice clarifying questions inline in the chat timeline |
| 📜 **Rules & skills** | `.agents/rules/` and `.agents/skills/` with install dialog |

### Platform

| | |
|---|---|
| 🌍 **14-language UI** | Compile-time translations and localized EULA |
| 🔄 **Auto-updater** | Signed GitHub Releases via Tauri v2 updater, including beta-channel support |
| 🛠️ **Setup & release scripts** | `scripts/setup/` and release automation for Linux, macOS, and Windows |
| ✅ **CI** | PR workflow runs `cargo check` for backend and `wasm32` frontend |

## What's new

**Latest release: [0.5.0](docs/releases/v0.5.0.md)** · [technical changelog](CHANGELOG.md) — MCP, nine text providers, workspace Kanban, Mermaid diagrams, HeartBeat Memory Indexer, notifications, beta updates, 32 themes, Vim mode, push-to-talk, and named terminal agents.

Highlights:

- Register stdio/HTTP MCP servers once and expose them to the in-app agent plus Claude, Codex, Gemini, OpenCode, and Cursor terminal CLIs.
- Plan work in the new pinned Kanban board, create AI plans/tasks, and generate Mermaid diagrams that travel with `.agents/plans/`.
- Use Ollama and LM Studio locally, or Anthropic, OpenAI, OpenRouter, Hugging Face, Cloudflare Workers AI, Together AI, and Portkey from the same agent settings.
- Track background work through the notification feed, process status line, HeartBeat services, and Memory Indexer.

See [CHANGELOG.md](CHANGELOG.md) for the full technical history and [docs/releases/](docs/releases/) for user-facing release notes.

## Screenshots

<p align="center">
  <img src="docs/images/workspace-terminal-system-monitor.png" alt="BLXCode workbench with terminal grid, system monitor, and agent sidebar" width="920" />
</p>

| Workbench | Agent panel |
|:---:|:---:|
| <img src="docs/images/terminal-grid-claude-usage.png" alt="Terminal grid with Claude usage panel" width="420" /> | <img src="docs/images/agent-panel-session-stats.png" alt="BLXCode Agent panel with session stats and model metrics" width="420" /> |

| Kanban plans | Mermaid diagrams |
|:---:|:---:|
| <img src="docs/images/workspace-kanban-board.png" alt="Workspace Kanban board with grouped plans and task lanes" width="420" /> | <img src="docs/images/mermaid-diagram-gallery.png" alt="Mermaid diagram gallery with exported plan diagrams" width="420" /> |

| Memory graph | Agent timeline |
|:---:|:---:|
| <img src="docs/images/memory-graph-3d-architecture.png" alt="3D architecture memory graph" width="420" /> | <img src="docs/images/agent-timeline-tool-groups.png" alt="Agent timeline with grouped tool calls" width="420" /> |

<details>
<summary>More screenshots (welcome, workspace setup, settings, memory, canvas)</summary>

<p align="center">
  <img src="docs/images/memory-center-architecture-files.png" alt="Memory center with architecture files" width="720" />
</p>

| Welcome | Workspace setup |
|:---:|:---:|
| <img src="docs/images/welcome-screen-create-workspace.png" alt="Welcome screen with Create Workspace action" width="360" /> | <img src="docs/images/create-workspace-session-role-dropdown.png" alt="Create Workspace session role and model dropdown" width="360" /> |

| Appearance | Updates & hooks |
|:---:|:---:|
| <img src="docs/images/settings-appearance-theme-grid.png" alt="Appearance settings with theme grid" width="360" /> | <img src="docs/images/settings-app-hooks-updates-help.png" alt="Settings app hooks, updates, and help controls" width="360" /> |

| Workspace canvas | Swarm map |
|:---:|:---:|
| <img src="docs/images/workspace-canvas-terminal-node.png" alt="Workspace canvas with terminal node" width="360" /> | <img src="docs/images/workspace-swarm-agent-map.png" alt="Workspace swarm agent map" width="360" /> |

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

Use `--check-only` to inspect missing prerequisites, or `--with-bundle` to run `cargo tauri build` after checks.

### Prerequisites

- Rust stable and Cargo
- `wasm32-unknown-unknown` Rust target
- Trunk and Cargo Tauri CLI
- Tauri 2 system dependencies for your OS

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk tauri-cli
```

On Linux, install WebKitGTK and build dependencies for your distribution.

### Run

```bash
cargo tauri dev
```

Tauri starts Trunk automatically via `src-tauri/tauri.conf.json`. The frontend serves at `http://localhost:1420`.

> **First-build tip:** Tauri's dev connection times out after 180 seconds. On slower machines, warm the WASM cache first:
>
> ```bash
> trunk build
> cargo tauri dev
> ```

### Build

```bash
./scripts/release.sh --build --linux-arch native
```

For a plain local build without packaging helpers, run `cargo tauri build`.

### Release automation

```bash
./scripts/release.sh
./scripts/release-macos.sh
```

```powershell
scripts\release.cmd
powershell -ExecutionPolicy Bypass -File scripts/release.ps1 --platform windows
```

Copy `.env.release.example` to `.env.release` only when you need signing keys or GitHub upload overrides.

### Checks

```bash
cargo check --workspace --all-targets --all-features
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo tauri build
```

## Documentation

Full index: [Documentation Home](docs/README.md) · [GitHub Wiki](https://github.com/Bitslix/BLXCode/wiki)

**User guides**

| Topic | Guide |
|-------|-------|
| Getting started | [docs/user/getting-started.md](docs/user/getting-started.md) |
| Workspaces & center tabs | [docs/user/workspaces.md](docs/user/workspaces.md) |
| Settings & API keys | [docs/user/settings.md](docs/user/settings.md) |
| Themes | [docs/user/appearance-themes.md](docs/user/appearance-themes.md) |
| File preview & handoff | [docs/user/file-preview.md](docs/user/file-preview.md) |
| Memory & tasks | [docs/user/memory-and-tasks.md](docs/user/memory-and-tasks.md) |
| Plans & Kanban | [docs/user/plans.md](docs/user/plans.md) |
| Rules & skills | [docs/user/rules-and-skills.md](docs/user/rules-and-skills.md) |
| BLXCode Agent | [docs/user/agent-harness.md](docs/user/agent-harness.md) |
| Subagents | [docs/user/subagents.md](docs/user/subagents.md) |
| Providers & hooks | [docs/user/agent-providers.md](docs/user/agent-providers.md) |
| Image mode | [docs/user/image.md](docs/user/image.md) |
| Voice | [docs/user/voice.md](docs/user/voice.md) |
| Keyboard shortcuts | [docs/user/keyboard-shortcuts.md](docs/user/keyboard-shortcuts.md) |
| UI language | [docs/user/language.md](docs/user/language.md) |
| Building | [docs/user/building.md](docs/user/building.md) |
| Troubleshooting | [docs/user/troubleshooting.md](docs/user/troubleshooting.md) |

**Developer guides**

- [Setup](docs/developer/setup.md) · [Architecture](docs/developer/architecture.md) · [BLXCode Agent](docs/developer/agent-harness.md) · [Subagents](docs/developer/subagents.md)
- [Tauri IPC](docs/developer/tauri-ipc.md) · [Voice](docs/developer/voice.md) · [i18n](docs/developer/i18n.md) · [Themes](docs/developer/themes.md) · [Contributing](docs/developer/contributing.md)

## Repository layout

```text
.
├── src/                    # Leptos CSR frontend (blxcode-ui)
├── src-tauri/              # Tauri 2 backend (blxcode)
├── content/                # EULA markdown and bundled agent hook scripts
├── src-tauri/src/agent/harness_skills/  # Embedded core skill Markdown
├── public/                 # Static assets (Trunk)
├── themes/                 # CSS theme tokens
├── scripts/                # Setup, release, and maintainer scripts
├── docs/                   # User and developer documentation
├── Cargo.toml              # Workspace manifest
├── Trunk.toml              # Frontend build config
└── styles.css              # Global app styling
```

## Workspace data

```text
<workspace>/.agents/memory/       # notes; subfolders = categories
<workspace>/.agents/learnings/    # repo learnings
<workspace>/.agents/plans/        # Markdown plans
<workspace>/.agents/rules/        # binding rule-*.md files
<workspace>/.agents/skills/       # user skills
<workspace>/.blxcode/tasks/       # task files
<workspace>/.blxcode/generated/   # image mode output
<workspace>/.blxcode/agent-context/  # handoff exports
```

API keys are stored in the OS keyring when available, with a private file fallback under the app config directory.

## Internationalization

BLXCode ships **14 locales** with compile-time string checks. Change language via **Ctrl+Shift+P** → **BLXCode settings** → **App** → **UI language**.

- [UI Language guide](docs/user/language.md)
- [Contributor i18n guide](docs/developer/i18n.md)

## Status

BLXCode is early-stage open source. The workbench, BLXCode Agent, MCP support, Kanban plans, Mermaid diagrams, sidebar Git, memory architecture map, and settings revamp are in active use on `main`; APIs and on-disk formats may still evolve. Current crate version: **0.5.0**.

## Community

| | |
|---|---|
| 🐛 **Bug reports** | [GitHub Issues](https://github.com/Bitslix/BLXCode/issues) — use the **Bug report** template |
| 💬 **Questions & ideas** | [GitHub Discussions](https://github.com/Bitslix/BLXCode/discussions) — **Q&A**, **Ideas**, and **General** templates |
| 📦 **Downloads** | [GitHub Releases](https://github.com/Bitslix/BLXCode/releases) — installers for Linux, macOS, and Windows |
| 🆘 **Help** | [SUPPORT.md](SUPPORT.md) — docs, troubleshooting, and where to ask |
| 🤝 **Contributing** | [CONTRIBUTING.md](CONTRIBUTING.md) — setup, conventions, PR checklist |

The in-app auto-updater pulls signed builds from GitHub Releases. User-facing release notes: [docs/releases/](docs/releases/). Technical history: [CHANGELOG.md](CHANGELOG.md).

## Contributing

Contributions welcome. Start with [Developer Setup](docs/developer/setup.md) and [Contributing](docs/developer/contributing.md). For code changes, open a pull request; for bugs and feature requests, use [Issues](https://github.com/Bitslix/BLXCode/issues) or [Discussions](https://github.com/Bitslix/BLXCode/discussions).

- Keep frontend (`blxcode-ui`) and backend (`blxcode`) boundaries clear
- Register Tauri commands in `src-tauri/src/lib.rs` and add wrappers in `src/tauri_bridge.rs`
- Update docs when user-facing behavior changes
- Run relevant checks before opening a pull request

## License

BLXCode is released under the [MIT License](LICENSE).
