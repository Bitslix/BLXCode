# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Rules

Always read and follow every rule in `.agents/rules/` before making changes. Each file in that directory is a binding rule for work in this repo.

## Project Overview

**blxcode** is a Tauri 2 + Leptos 0.7 desktop application. The frontend is a Leptos CSR (client-side rendered) WASM app built with Trunk; the backend is a Tauri 2 Rust process.

## Commands

```bash
# Dev (starts Trunk + Tauri together)
cargo tauri dev

# Production build
cargo tauri build

# Frontend only (WASM, served at http://localhost:1420)
trunk serve
trunk build

# Run all tests
cargo test --workspace

# Check frontend crate only
cargo check -p blxcode-ui --target wasm32-unknown-unknown

# Check Tauri crate only
cargo check -p blxcode
```

> The Tauri dev command invokes `trunk serve` automatically via `beforeDevCommand` in `tauri.conf.json`.

## Architecture

### Two-crate workspace

| Crate | Path | Role |
|---|---|---|
| `blxcode-ui` | `src/` | Leptos CSR WASM frontend |
| `blxcode` | `src-tauri/` | Tauri 2 backend (native) |

### Frontend (`src/`)

Entry point: `src/main.rs` → mounts `<App/>`.

`App` controls the top-level rendering gate:
1. **EULA gate** – checked via `localStorage` (`EULA_STORAGE_KEY`). Decline calls `exit_app` Tauri command.
2. **WorkbenchShell** – rendered after EULA acceptance. The app is free to use with no sign-in.

Module layout:
- `config/app.config.rs` — all constants: localStorage keys, embedded browser defaults.
- `service/` — `I18nService`.
- `i18n/` — `Locale` + `APP_LOCALES` (language picker metadata), `parse_bcp47` / `infer_from_browser_lang`, `lookup` + `locales/*.rs` (one `msg` match per language), `content/eula/*.md` + `eula.rs`. Default locale is **`EnUs`**. Regenerate non-English UI tables from `en_us.rs` with `scripts/render_i18n_locales_from_en.py` (default: missing keys only; use `--full` for a full rewrite; requires `deep-translator` in a venv). Adding an `I18nKey` requires a new string in **every** `locales/*.rs` file (compile-time exhaustiveness).
- `workbench/` — three-pane shell: `Sidebar`, `WorkspacePanel`, `RightPanel`; agent panel, browser tab.
- Workspace data: `.agents/memory/`, `.agents/learnings/` (memory API). Agent tasks live globally under `{app_data_dir}/tasks/<workspace_hash>/index.json` (see `src-tauri/src/app_paths.rs`), so they are *not* committed with the workspace.
- `agent_wire.rs` — mirrors `src-tauri/src/agent/protocol.rs`; shared Serde types for IPC events.
- `tauri_bridge.rs` — wrappers around `invoke()` calls to Tauri commands.

### Backend (`src-tauri/src/`)

`lib.rs` wires Tauri state and registers all commands.

**Agent subsystem** (`agent/`):
- `state.rs` — `AgentEngineState`: thread-safe event queue (`VecDeque` behind `Mutex`) + atomic `busy`/`cancel` flags; persisted model `conversation` (cleared via `agent_clear_conversation`). `ProviderEnv` reads `BLX_ANTHROPIC_API_KEY` from the environment.
- `protocol.rs` — `UserTurn` (input) and `AgentEvent` enum (output: `AssistantDelta`, `ToolCall`, `ToolResult`, `Done`, `Error`).
- `system_prompt.rs` — shared system prompt for OpenRouter, OpenAI-compatible, and Anthropic paths.
- `session_orchestrator.rs` — `dispatch_user_turn`: loads settings + API key, spawns the configured HTTP provider turn.
- `openrouter.rs` / `anthropic.rs` — streaming tool-call loops against remote APIs.
- `provider.rs` — `InferenceProvider` trait stub for future pluggable backends.
- `tools.rs` — `WorkspaceRootGuard` / `ScopedReadOps`: sandboxed file reads within the user-set workspace root.

**IPC pattern**: The frontend polls `agent_poll_events` on a timer to drain `AgentEvent`s from the queue. Submissions go via `agent_submit_turn`. Chat + model context reset: `agent_clear_conversation` (disabled while the agent is busy). This is a poll-based design, not push/SSE.

**Browser host** (`browser_host.rs`): Embeds a native child webview on Windows/macOS (`add_child` via Tauri unstable API). On Linux, falls back to an `<iframe>` inside the SPA because `native_child_inset_supported()` returns `false`.

## Key Configuration

- **`src/config/app.config.rs`** — localStorage keys, embedded-browser defaults.
- **`BLX_ANTHROPIC_API_KEY`** env var — enables real Anthropic provider path (currently stub only).
- **`tauri.conf.json`** — app identifier `com.bitslix.blxcode`, window size, CSP, bundle targets.

## Architecture Map

Workspace memory includes a harness-generated architecture map:

- `.agents/memory/ARCHITECTURE.md` — curated index with a harness-managed Generated block and a Manual section for human prose.
- `.agents/memory/architecture/modules/*.md` — generated per-crate Rust module skeletons.
- `.agents/memory/architecture/flows/` — reserved for hand-authored flow notes.
- `.agents/memory/.meta/architecture-state.json` — local state used for staleness checks; ignored by git.

Regenerate after large refactors with `memory_rebuild_architecture` from the app/agent tooling. Use `memory_lint_architecture` to mark generated notes stale when git `HEAD` changed without a rebuild. Commit `ARCHITECTURE.md` and `architecture/modules/*.md` so structural diffs travel with PRs; do not commit `.agents/memory/.meta/`.
