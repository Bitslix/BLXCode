//! Remote (SSH) git plumbing. Mirrors the local `git -C <work_tree> …`
//! invocations from `git_status`/`git_graph`/`git_sync`/`git_commit_ai`, but
//! runs `git` on the remote host over the per-connection exec channel. The
//! output is byte-identical to the local path, so every existing parser is
//! reused unchanged by the calling modules.

use tauri::AppHandle;

use crate::pty_host::{sh_quote, PtyManager};
use crate::ssh_exec::{ExecOutput, RemoteExecManager, EXEC_TIMEOUT_LONG_MS, EXEC_TIMEOUT_MS};

/// Build `GIT_TERMINAL_PROMPT=0 git -C <work_tree> --no-optional-locks <args…>`
/// as a single remote shell command. `GIT_TERMINAL_PROMPT=0` makes auth
/// failures fail fast instead of wedging the channel on a hidden prompt.
fn build_git_cmd(work_tree: &str, args: &[&str]) -> String {
    let mut cmd = String::from("GIT_TERMINAL_PROMPT=0 git -C ");
    cmd.push_str(&sh_quote(work_tree));
    cmd.push_str(" --no-optional-locks");
    for a in args {
        cmd.push(' ');
        cmd.push_str(&sh_quote(a));
    }
    cmd
}

/// Run a git subcommand on the remote, returning stdout (lossy UTF-8). Errors
/// carry the git stderr, matching the local `run_git` contract.
pub fn run_git_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    work_tree: &str,
    args: &[&str],
) -> Result<String, String> {
    run_git_remote_timeout(app, pty, exec, connection_id, work_tree, args, EXEC_TIMEOUT_MS)
}

fn run_git_remote_timeout(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    work_tree: &str,
    args: &[&str],
    timeout_ms: u64,
) -> Result<String, String> {
    let cmd = build_git_cmd(work_tree, args);
    let out = exec.run(app, pty, connection_id, &cmd, timeout_ms)?;
    if out.ok() {
        Ok(out.stdout_string())
    } else {
        let stderr = out.stderr_string();
        let stderr = stderr.trim();
        let sub = args.first().copied().unwrap_or("?");
        if stderr.is_empty() {
            Err(format!("git {sub}: exit {}", out.code))
        } else {
            Err(format!("git {sub}: {stderr}"))
        }
    }
}

/// Run a git subcommand and return stdout regardless of exit code (used for
/// `diff --no-index`, which exits 1 when differences exist).
pub fn run_git_remote_lenient(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    work_tree: &str,
    args: &[&str],
) -> Result<String, String> {
    let cmd = build_git_cmd(work_tree, args);
    let out = exec.run(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
    Ok(out.stdout_string())
}

/// Run a git subcommand and return the raw [`ExecOutput`] (code + stdout +
/// stderr) so callers can classify failures themselves (used by git_sync,
/// which mirrors the local `Output`-based flow). `long` selects the network
/// timeout for fetch/pull/push.
pub fn run_git_remote_raw(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    work_tree: &str,
    args: &[&str],
    long: bool,
) -> Result<ExecOutput, String> {
    let cmd = build_git_cmd(work_tree, args);
    let timeout = if long {
        EXEC_TIMEOUT_LONG_MS
    } else {
        EXEC_TIMEOUT_MS
    };
    exec.run(app, pty, connection_id, &cmd, timeout)
}

/// Resolve the repository top level on the remote (`rev-parse --show-toplevel`).
/// Errors if `cwd` is not inside a git repository.
pub fn remote_work_tree(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    cwd: &str,
) -> Result<String, String> {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return Err("cwd is empty".into());
    }
    let top = run_git_remote(app, pty, exec, connection_id, cwd, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "not a git repository".to_string())?;
    let top = top.trim();
    if top.is_empty() {
        Err("not a git repository".into())
    } else {
        Ok(top.to_string())
    }
}

/// `true` when `cwd` is inside a git work tree on the remote.
pub fn remote_is_repository(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    cwd: &str,
) -> bool {
    matches!(
        run_git_remote(
            app,
            pty,
            exec,
            connection_id,
            cwd.trim(),
            &["rev-parse", "--is-inside-work-tree"],
        ),
        Ok(s) if s.trim() == "true"
    )
}
