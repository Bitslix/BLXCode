# SSH Remote Transport

How remote (SSH) workspaces are implemented. User-facing guide: [Remote Workspaces (SSH)](../user/remote-ssh.md).

## Overview

The remote transport **wraps the system `ssh` client** inside the existing PTY pipeline rather than
embedding an SSH stack. A remote terminal is just `ssh user@host …` running in a `portable-pty` PTY, so
all the existing terminal plumbing (drain/resize/kill, agent auto-launch, session env) is reused. A
separate **persistent exec channel** per connection powers the non-interactive remote work (file
browsing, git, resume discovery).

The frontend is **transport-agnostic**: every fs/git command already took a `cwd` string and now also
takes an optional `connectionId`. When present, the backend command routes to its remote implementation;
when absent, the original local path runs unchanged.

## Backend modules (`src-tauri/src/`)

| Module | Responsibility |
|--------|----------------|
| `ssh_remotes.rs` | Connection presets (`RemoteConnection`: id, label, host, port, username, `RemoteAuthKind`, key path, `RemoteResume`, default dir). Backend-owned store `remote_connections.json`. Commands: `ssh_remotes_list`, `ssh_remote_save`, `ssh_remote_delete`, `ssh_remote_test`. `resolve_spec()` turns a preset id + keychain secret into a spawn spec. |
| `ssh_secrets.rs` | Password / passphrase storage in the OS keychain (`keyring`) with a Linux encrypted-file fallback. Mirrors the `agent_settings` key pattern. Keyed `ssh:<connection_id>:{password,passphrase}`. |
| `pty_host.rs` | `spawn_remote_session()` builds the `ssh` argv (`build_ssh_args`) and spawns it in a PTY; `Injector` answers the password/passphrase prompt **through the PTY** (never argv/env). `RemoteSpawnSpec` / `RemoteAuthMode` / `ResumeMode`. `kill_all()` for teardown. tmux session naming + `sh_quote` for remote command building. |
| `ssh_exec.rs` | `RemoteExecManager`: one persistent authenticated login shell per connection, driven as a **marker-framed RPC** (`run`/`run_text`/`run_check`) — captures stdout/stderr to temp files and base64-streams them between per-call nonced markers (binary-safe, PTY-echo-proof). Also `remote_exec_close` and `agent_remote_latest_session_id` (remote resume discovery). |
| `git_remote.rs` | `run_git_remote*` helpers run `git -C <remote_top> …` over the exec channel; output is byte-identical to local, so every parser in `git_status`/`git_graph`/`git_sync`/`git_commit_ai` is reused. `GIT_TERMINAL_PROMPT=0`. |
| `fs_entries.rs` | Each fs command dispatches local-vs-remote on `connection_id`; remote impls run `ls -Ap1` / `head -c` / `wc -c` / `mkdir` over the exec channel. Remote paths are sandboxed (`remote_target`: reject `..`/absolute, prefix-guard). Local bodies extracted to `local_*` helpers. |
| `commands.rs` | `pty_spawn_remote` resolves the preset + secret server-side and spawns the terminal. `git_branch` gained remote routing. |
| `lib.rs` | Registers `RemoteExecManager`; the `RunEvent::ExitRequested/Exit` handler calls `PtyManager::kill_all()` so no `ssh`/PTY children are orphaned. |

## Frontend (`src/`)

- `WorkspaceEntry.remote_connection_id: Option<String>` marks a workspace remote (`#[serde(default)]`,
  back-compat). `WorkbenchService::active_remote_connection_id()` and
  `remote_connection_for_terminal_key()` resolve it.
- `tauri_bridge.rs` wrappers gained `connection_id: Option<String>` (skipped when `None`).
- `remote_settings_pane/` — the **Settings → Remote** pane (`RemoteSettingsPane`), registered as the
  `HarnessSettingsCategory::Remote` category in `harness_ui.rs`. The pane is a **master/detail view**:
  the left rail lists connection cards (name, `user@host:port`, auth badge, session-resume badge, and
  a masked `Stored` / `Not set` indicator for any associated keychain secret), the right pane is the
  connection editor (Back to connections, New/Edit title, the full preset form, Save / Test / Delete /
  Back buttons). The selected card is reflected in a `selected_connection_id` signal; choosing *New*
  or *Edit* swaps the detail pane for the editor.
- `create_workspace_wizard.rs` — Local/Remote selector + "+ Add connection" (opens Settings → Remote via
  `ui.settings_category().set(Remote)` + `wb.open_center_settings_tab(Remote)`, and the wizard's
  remote-mode combobox is the same card list the Remote pane shows).
- `terminal_cell.rs` — routes spawning to `pty_spawn_remote` and resume discovery to the remote path.
- `file_diff_section` / `git_graph` — when remote, poll (~4 s `gloo_timers::Interval`) and bump
  `sidebar_repo_epoch`; both load effects treat an epoch change as a forced reload (also fixes
  post-sync staleness locally). The `notify` watcher is skipped for remote (`git_status_watch_start`
  returns a sentinel token).
- `sidebar.rs` — SSH badge + `--remote` row modifier on remote workspaces.

## Connection editor model

`RemoteConnection` is the on-disk preset (`id`, `label`, `host`, `port`, `username`,
`RemoteAuthKind { Key | Password | None }`, `key_path`, `RemoteResume { Off | NewestSession |
TmuxSession(name) }`, `default_dir`). Secrets are **never** in this struct — passwords and
passphrases live in the OS keychain keyed `ssh:<connection_id>:{password,passphrase}`. The
settings UI's *Stored* / *Not set* indicator on each card is a summary of the keychain's
presence for the connection, computed on the backend; the indicator never reveals the secret.

`resolve_spec(connection_id, secret)` in `ssh_remotes.rs` is the single point that combines a
preset with the resolved keychain secret (or environment fallback) and returns a
`RemoteSpawnSpec` that `pty_host` consumes. The auth dispatcher in `pty_host` (`Injector`) is
the only path that injects passwords / passphrases — through the PTY, never on the `ssh`
command line or in env.

## Data flow

```
Frontend (cwd + connectionId)
      │  Tauri invoke
      ▼
fs/git command ── connectionId? ──► remote impl ──► RemoteExecManager.run ──► exec channel (1 ssh)
      │ None                                              ▲
      ▼                                                   │ marker-RPC over PTY
local impl (std::fs / git -C)                     pty_host spawn_remote_session

Terminal: pty_spawn_remote ──► spawn_remote_session ──► ssh PTY (1 per terminal)
```

Presets persist in the backend `remote_connections.json`; `WorkspaceEntry.remote_connection_id` persists
in the frontend-owned workbench snapshot.

## Security model

- Secrets only in the OS keychain / Rust memory; never in the preset file, the bridge/JS, or `ssh`
  argv/env (password/passphrase injected through the PTY by `Injector`).
- Host keys: `StrictHostKeyChecking=accept-new` (TOFU via `known_hosts`); changed keys are refused.
- Remote fs sandbox is weaker than the local `canonicalize` path (no symlink resolution) — it rejects
  `..`/absolute and prefix-guards under the workspace root. Documented limitation.

## Known limitations / follow-up

- **One SSH connection per terminal** (+ one exec channel) — a workspace with *N* terminals makes ~*N+1*
  connections, so large grids can hit `sshd MaxSessions` (default 10) and `MaxStartups` (connection
  rate/burst). Each connection also re-authenticates.
- No `~/.ssh/config` / ProxyJump (wrapped `ssh` would honour config, but presets are explicit).
- Remote agent resume uses a "newest session for this cwd" heuristic (no hooks installed remotely);
  tmux mode is the exact-resume path.

These are addressed by the planned **russh** migration (multiplex all terminals + exec over one
authenticated connection per preset, Windows-friendly, no `ControlMaster`):
[`.agents/plans/russh-transport-refactor/plan.md`](../../.agents/plans/russh-transport-refactor/plan.md).

## Tests

`cargo test -p blxcode` covers the exec marker-frame parser (rc/stdout/stderr round-trip, binary
payload, whitespace-tolerant base64), the `ssh` argv builder (auth modes, tmux resume, BatchMode rules),
`tmux_session_name`/`sh_quote`/`probe_failure`, and `remote_target` sandbox rejection.
