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
2. **Auth gate** – `AuthGateState` cycles through `CheckingSession → NeedLogin | LoggedIn`.
3. **WorkbenchShell** – rendered when logged in.

Module layout:
- `config/app.config.rs` — all constants: API URL/path, localStorage keys, device client ID.
- `auth/` — session fetch, sign-in, sign-out, device flow, `LoginModal` component.
- `service/` — `ApiService` (HTTP via `gloo-net`), `I18nService`.
- `i18n/` — `Locale` + `APP_LOCALES` (language picker metadata), `parse_bcp47` / `infer_from_browser_lang`, `lookup` + `locales/*.rs` (one `msg` match per language), `content/eula/*.md` + `eula.rs`. Default locale is **`EnUs`**. Regenerate non-English UI tables from `en_us.rs` with `scripts/render_i18n_locales_from_en.py` (requires `deep-translator` in a venv). Adding an `I18nKey` requires a new string in **every** `locales/*.rs` file (compile-time exhaustiveness).
- `workbench/` — three-pane shell: `Sidebar`, `WorkspacePanel`, `RightPanel`; agent panel, browser tab.
- `agent_wire.rs` — mirrors `src-tauri/src/agent/protocol.rs`; shared Serde types for IPC events.
- `tauri_bridge.rs` — wrappers around `invoke()` calls to Tauri commands.

### Backend (`src-tauri/src/`)

`lib.rs` wires Tauri state and registers all commands.

**Agent subsystem** (`agent/`):
- `state.rs` — `AgentEngineState`: thread-safe event queue (`VecDeque` behind `Mutex`) + atomic `busy`/`cancel` flags. `ProviderEnv` reads `BLX_ANTHROPIC_API_KEY` from the environment.
- `protocol.rs` — `UserTurn` (input) and `AgentEvent` enum (output: `AssistantDelta`, `ToolCall`, `ToolResult`, `Done`, `Error`).
- `orchestrator.rs` — `spawn_mock_turn`: current mock engine; streams chunked deltas, supports `READ:<relative-path>` inline command for scoped file reads.
- `session_orchestrator.rs` — `dispatch_user_turn`: thin facade; checks busy state before spawning.
- `provider.rs` — `InferenceProvider` trait stub; `maybe_emit_network_hint` probes `api.anthropic.com` when the API key is set.
- `tools.rs` — `WorkspaceRootGuard` / `ScopedReadOps`: sandboxed file reads within the user-set workspace root.

**IPC pattern**: The frontend polls `agent_poll_events` on a timer to drain `AgentEvent`s from the queue. Submissions go via `agent_submit_turn`. This is a poll-based design, not push/SSE.

**Browser host** (`browser_host.rs`): Embeds a native child webview on Windows/macOS (`add_child` via Tauri unstable API). On Linux, falls back to an `<iframe>` inside the SPA because `native_child_inset_supported()` returns `false`.

### Auth backend

The app expects a [Better Auth](https://www.better-auth.com/) server at `API_URL` (`http://localhost:3005` by default) under `API_PATH` (`/api/`). Required endpoints: `auth/get-session`, `auth/sign-in/email`, `auth/sign-out`, `auth/device/code`, `auth/device/token`. The server must set `trustedOrigins` to include `http://localhost:1420` and enable `CORS` with credentials + the `deviceAuthorization` and `Bearer` plugins.

## Key Configuration

- **`src/config/app.config.rs`** — change `API_URL`/`API_PATH` to point at a different backend.
- **`BLX_ANTHROPIC_API_KEY`** env var — enables real Anthropic provider path (currently stub only).
- **`tauri.conf.json`** — app identifier `com.bitslix.blxcode.app`, window size, CSP, bundle targets.
