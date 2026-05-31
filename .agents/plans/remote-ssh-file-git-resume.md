# Plan: Remote (SSH) File/Git Browsing + Remote Session Resume

## Context

The SSH remote-workspace feature shipped, but two gaps remain (called out as scope notes):

1. **File explorer, file preview/diff, and the git sidebar sections operate on the local
   filesystem.** They pass the workspace `cwd` (a string) to backend commands that use `std::fs`
   (`src-tauri/src/fs_entries.rs`) and `git -C <cwd>` (`git_info/status/graph/sync/commit_ai.rs`) plus a
   local `notify` FS watcher. For a remote workspace `cwd` is a *remote* path, so all of this shows
   nothing useful.
2. **Agent-CLI session resume on remotes** currently only works via tmux persistence (the live agent
   survives reattach). After the remote process actually dies (keepalive mode, remote reboot) there is
   no `<agent> --resume <id>` because the resume metadata lives on the remote, unreachable by the local
   app.

Both gaps share one missing primitive: the **backend has no way to run a command or read a file on the
remote over SSH** outside the interactive terminal PTY. This plan adds that primitive and builds both
features on top.

**Decisions (from user):**

- **Exec transport:** one **persistent authenticated exec channel per connection** (Windows OpenSSH has
  no `ControlMaster`, so per-command `ssh` is out for password auth). Reuses our existing PTY + password
  injection; one auth, fast, cross-platform.
- **Git change detection:** **poll (~4 s while a section is open) + manual refresh** — no remote
  dependency, no `notify`.
- **Scope:** **full read + write parity** (browse/preview *and* create/stage/commit/push).

The frontend is already transport-agnostic — every component passes `cwd` as a string to a Tauri
command (`ProjectExplorerBody`, `FilePreviewDock`, `CodeView`, `FileDiffDock`, `FileDiffSection`,
`GitGraphSection`, `git_sync_controls.rs`). So the work is mostly **backend routing** plus threading a
`connection_id` through the bridge.

---

## Phase 0 — Remote exec channel (the foundation)

**New: `src-tauri/src/ssh_exec.rs`** — `RemoteExecManager` (Tauri-managed state), a
`HashMap<connection_id, ExecChannel>` guarded by a `Mutex`; each channel serializes one in-flight
command.

- **Open:** reuse `pty_host::RemoteSpawnSpec` + `build_ssh_args` + injection by calling
  `PtyManager::spawn_remote_session` with a dedicated `terminal_key = "exec:<connection_id>"`,
  `ResumeMode::KeepaliveOnly`, no remote command (login shell). This reuses auth/injection/`kill_all`
  and keeps the exec PTY out of the workspace terminal UI. Resolve the spec via
  `ssh_remotes::resolve_spec` (already exists). After connect, send `stty -echo 2>/dev/null; printf
  'BLX_READY\n'` and drain until `BLX_READY` to confirm the shell prompt.
- **Run (marker-framed RPC):** for each call pick a random nonce and send a one-liner that captures
  stdout/stderr to temp files, prints the exit code, then base64-streams both blobs between unique
  markers:

  ```sh
  t=$(mktemp); e=$(mktemp); { <CMD>; } >"$t" 2>"$e"; rc=$?; \
  printf '\n<<BLX:%s:S>>\n' N; printf '%s\n' "$rc"; base64 <"$t"; \
  printf '<<BLX:%s:M>>\n' N; base64 <"$e"; printf '<<BLX:%s:E>>\n' N; rm -f "$t" "$e"
  ```

  Read via `PtyManager::drain_output_wait` until the `:E` marker; strip all whitespace from each base64
  blob before decoding (portable across GNU/BSD `base64`, survives PTY newline/echo mangling and binary
  payloads). Returns `ExecOutput { code: i32, stdout: Vec<u8>, stderr: Vec<u8> }`.
- **Helpers** (also new, in `ssh_exec.rs`): `run_text(cmd) -> String`, `run_check(cmd) -> Result<()>`,
  and `sh_quote` reuse (make `pty_host::sh_quote` `pub(crate)`).
- **Lifecycle:** lazily `ensure_channel` on first remote fs/git/resume call; `close(connection_id)` when
  the last terminal for that connection closes and in the app-exit handler (extend the existing
  `RunEvent::ExitRequested` path in `lib.rs`; `PtyManager::kill_all` already kills the underlying
  session, so this mainly drops the map entry).
- Register `RemoteExecManager::default()` via `.manage(...)` in `lib.rs`.

A small `PtyManager` helper — `read_until_marker(session_id, marker, timeout)` — keeps the RPC loop
tidy; otherwise drive it with the existing `pty_write` + `drain_output_wait`.

---

## Phase 1 — Remote file browsing, preview & create (read + write)

**Thread a connection id through the bridge.** Add `connection_id: Option<String>` (camelCase
`connectionId`, `#[serde(skip_serializing_if = "Option::is_none")]`) to the fs command signatures in
`src-tauri/src/fs_entries.rs` and their wrappers in `src/tauri_bridge.rs`. Each frontend call site
already resolves the workspace — also read `remote_connection_id` (use the existing
`wb.with_active_workspace_entry()` / `workspaces().with_untracked(...)` lookups in
`project_explorer/mod.rs`, `file_preview/mod.rs` + `code_view.rs`, etc.) and pass it.

**Dispatch in each fs command:** `if let Some(cid) = connection_id { remote_*(exec, cid, root, path) }
else { /* existing local body */ }`. Remote impls run portable shell over the exec channel:

- `list_path_entries` → `ls -Ap1 <dir>` (trailing `/` ⇒ dir; leading `.` ⇒ hidden), sorted client-side
  to match the local order.
- `stat_workspace_file` → `wc -c < <file>` for `byte_len`; **`FileKind`/`PolicyKind` are derived from the
  path** (extension/stem), already done without touching the FS, so no portable `stat` needed; `modified_ms`
  best-effort/optional.
- `read_workspace_text_file` → `head -c <CAP> <file>` + `wc -c` (truncated flag).
- `read_workspace_image_file` / `_video_file` → `head -c <CAP> <file>` (exec base64 path already yields
  bytes); MIME from extension.
- `create_workspace_file` → `set -C; : > <file>` (noclobber errors if exists); `create_workspace_dir` →
  `mkdir <dir>` after `mkdir -p <parent>`.
- **Sandboxing:** reuse the local rel-path rules (reject absolute + `..`, the `resolve_new_under_root`
  logic) *before* sending; join to the remote root; `sh_quote` every path; add a remote
  `case "$p" in "$root"/*) ;; *) exit 9;; esac` guard. Document the weaker (non-canonicalized) guarantee.

Frontend file components are otherwise unchanged — they already render whatever the command returns.

---

## Phase 2 — Remote git (status/diff/graph/sync, read + write) + polling

**Single choke point per module.** Each git module funnels through one runner — `run_git` in
`git_status.rs`, `run` in `git_sync.rs`, `fetch_graph_entries` in `git_graph.rs`, `staged_diff` in
`git_commit_ai.rs`, and `current_branch` in `git_info.rs`. Add a remote runner
`run_git_remote(exec, cid, work_tree, args)` that executes `git -C <remote_path> <args...>` over the
exec channel and returns the same stdout bytes — **all existing porcelain/numstat/log parsing is
reused unchanged**.

Add `connection_id: Option<String>` to every git command (`git_status_changes`, `git_file_diff`,
`git_stage_file`/`_unstage_file`/`_stage_all`/`_unstage_all`, `git_commit`, `git_commit_graph`,
`git_sync_status`/`git_fetch`/`git_pull`/`git_push`, `git_generate_commit_message`,
`git_is_repository`, `git_branch`) and their bridge wrappers; dispatch local vs remote on it. Keep
`GIT_TERMINAL_PROMPT=0` semantics on the remote (prepend `GIT_TERMINAL_PROMPT=0` to the remote command)
so remote auth failures fail fast rather than hang the channel.

**Watcher → poll.** `git_status_watch_start`/`_stop` become **no-ops when `connection_id` is Some**
(return a sentinel token). In `file_diff_section/mod.rs` and `git_graph/mod.rs`, when the active
workspace is remote, start a `gloo_timers` interval (~4 s, only while the section is mounted/open) that
bumps the existing reload generation / `sidebar_repo_epoch`, and surface a manual **Refresh** button.
Local workspaces keep the `notify` watcher untouched. The frontend already passes `cwd`; just also pass
`connection_id` and gate the polling on it.

---

## Phase 3 — Remote agent-CLI session resume

Avoid installing hooks on the remote. Instead reuse the existing **"latest session for this cwd"**
discovery (already the local fallback in `agent_latest_session_id`) but run it over the exec channel:

- **New command** `agent_remote_latest_session_id(connection_id, agent, remote_cwd) -> Option<String>`
  in `workbench_state.rs` (or `ssh_exec.rs`), mirroring the local per-agent logic over SSH:
  - Claude: newest `*.jsonl` by mtime in `~/.claude/projects/<encoded-cwd>/` — `ls -t <dir>/*.jsonl |
    head -1`, strip `.jsonl`. Reuse the same cwd→dir encoding (non-alnum→`-`) the local code uses.
  - Codex: deepest-match scan under `~/.codex/sessions/`; Gemini: `~/.gemini/sessions/<id>/transcript.json`.
  - `agent_remote_session_exists(...)` → `test -f <path>` over the channel.
- **terminal_cell branch.** In `terminal_cell.rs` `lookup_resume_session` /
  `spawn_agent_launch_when_ready`, when the workspace is remote
  (`wb.remote_connection_for_terminal_key`), call the remote discovery instead of local
  `sessions.json`/`agent_session_exists`, then build the same `<agent> --resume <id>` command. **tmux
  resume already covers the live case**; this adds resume after the process has died (keepalive mode /
  reboot). Per-terminal exactness isn't available without remote hooks, so multi-terminal-same-cwd uses
  the latest-session heuristic (documented; tmux mode is the exact-resume path).

---

## Critical files

- **New:** `src-tauri/src/ssh_exec.rs` (exec channel + RPC), register module + `.manage()` in `lib.rs`.
- **Backend dispatch (local vs remote):** `fs_entries.rs`, `git_status.rs`, `git_sync.rs`,
  `git_graph.rs`, `git_commit_ai.rs`, `git_info.rs`, `commands.rs` (`git_branch`).
- **Reuse:** `pty_host::{RemoteSpawnSpec, build_ssh_args, spawn_remote_session, sh_quote, kill_all}`,
  `ssh_remotes::resolve_spec`, the local fs path-validation helpers, every git output parser, and
  `agent_latest_session_id`'s discovery logic.
- **Bridge:** `src/tauri_bridge.rs` — add `connection_id: Option<String>` to the fs + git wrappers and
  the new resume command.
- **Frontend call sites (pass `connection_id`, add polling/refresh):** `project_explorer/mod.rs`,
  `file_preview/mod.rs` + `code_view.rs`, `file_diff/mod.rs`, `file_diff_section/mod.rs`,
  `git_graph/mod.rs`, `git_sync_controls.rs`, `terminal_cell.rs`. A `wb` helper
  `active_remote_connection_id() -> Option<String>` (alongside the existing
  `remote_connection_for_terminal_key`) centralizes the lookup.
- **i18n:** a few keys (`RemoteGitRefresh`, remote-browse error strings); add to `keys.rs` + `en_us.rs`,
  then regenerate the 12 locales with `scripts/tools/render_i18n_locales_from_en.py` (venv +
  `deep-translator`).

## Security & performance notes

- Secrets never leave Rust — the exec channel authenticates exactly like a terminal (PTY injection), and
  remote paths are `sh_quote`d; no secret or unsanitized path reaches a shell string.
- Remote sandbox is weaker than local (no `canonicalize`): enforce reject-`..`/absolute before send + a
  remote prefix `case` guard; document the residual trust in the remote shell.
- One serialized exec channel per connection keeps auth cost at once-per-connection; polling is bounded
  (~4 s, only while a git section is open) and pauses when unmounted.
- `GIT_TERMINAL_PROMPT=0` on the remote prevents a credential prompt from wedging the channel.
- Binary/large reads keep the existing local caps (`MAX_*_PREVIEW_BYTES`) via `head -c`.

## Verification

1. `cargo check -p blxcode` and `cargo check -p blxcode-ui --target wasm32-unknown-unknown` compile.
2. Unit tests (`cargo test -p blxcode`): exec marker-frame parser (rc + stdout + stderr round-trip,
   binary payload, whitespace-stripped base64), `ls -Ap1` → `FsEntryBrief` parsing, remote rel-path
   sandbox rejection, remote `git status --porcelain` parse reuse.
3. `cargo tauri dev` against a real SSH host (try key, agent, and password auth):
   - **Explorer:** remote workspace lists the remote tree; expand dirs; create file/dir; open a file →
     preview text/image renders.
   - **Git:** the diff section lists remote changes; open a file diff; stage/unstage/commit; graph shows
     remote history; fetch/pull/push work; edit a remote file in the terminal → the section refreshes
     within the poll window (and immediately on manual Refresh).
   - **Resume:** keepalive-mode remote workspace, run `claude`, note the session; close the workspace,
     reopen → `claude --resume <id>` picks up the prior session (verify the discovered id matches the
     newest remote `~/.claude/projects/<cwd>/*.jsonl`). Confirm tmux mode still reattaches the live
     session.
   - **Teardown:** quit the app → no orphaned `ssh` exec processes; channels closed on workspace close.
