//! Remote sync for the sidebar File Diff section: branch/upstream status plus
//! fetch, pull (fetch + merge) and push. Free-form git stderr is classified
//! into stable `kind` codes the frontend maps to localized toasts.
//!
//! Every git invocation sets `GIT_TERMINAL_PROMPT=0` so a missing credential
//! fails fast instead of hanging the (poll-based) UI on an interactive prompt.

use crate::git_info::{find_git_dir, git_cli_available};
use crate::git_status::GIT_MISSING_CODE;
use crate::proc::command;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Output;

/// Branch / upstream / divergence snapshot used to enable-disable the sync
/// buttons and fill their tooltips.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// Current branch, or `None` when detached.
    pub branch: Option<String>,
    /// Tracking branch (e.g. `origin/main`), or `None` when unset.
    pub upstream: Option<String>,
    /// Commits the local branch is ahead of its upstream.
    pub ahead: u32,
    /// Commits the local branch is behind its upstream.
    pub behind: u32,
    /// Whether any remote is configured.
    pub has_remote: bool,
    /// Whether HEAD is detached.
    pub detached: bool,
    /// Whether the working tree has uncommitted changes.
    pub dirty: bool,
}

/// Result of a fetch / pull / push. `kind` is a stable code; `detail` carries a
/// trimmed git output tail for tooltips/diagnostics.
///
/// Success kinds: `ok`, `up_to_date`, `updated`.
/// Soft-failure kinds: `no_remote`, `no_upstream`, `dirty`, `conflict`,
/// `non_fast_forward`, `auth`, `network`, `lock`, `error`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub kind: String,
    pub detail: String,
}

impl SyncOutcome {
    fn new(kind: &str, detail: impl Into<String>) -> Self {
        Self {
            kind: kind.to_string(),
            detail: detail.into(),
        }
    }
}

fn work_tree(cwd: &str) -> Result<PathBuf, String> {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Err("cwd is empty".into());
    }
    let git_dir =
        find_git_dir(Path::new(trimmed)).ok_or_else(|| "not a git repository".to_string())?;
    git_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "invalid git dir".to_string())
}

fn run(work_tree: &Path, args: &[&str]) -> Result<Output, String> {
    command("git")
        .arg("-C")
        .arg(work_tree)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("git {}: {e}", args.first().copied().unwrap_or("?")))
}

fn has_remote(work_tree: &Path) -> bool {
    run(work_tree, &["remote"])
        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false)
}

/// Keep the last ~600 chars of git output for the tooltip (char-boundary safe).
fn tail(s: &str) -> String {
    let t = s.trim();
    let chars: Vec<char> = t.chars().collect();
    if chars.len() <= 600 {
        return t.to_string();
    }
    let tail: String = chars[chars.len() - 600..].iter().collect();
    format!("…{tail}")
}

/// Map git stderr/stdout to a stable failure `kind`. Order matters — the more
/// specific patterns are checked before the generic fallback.
fn classify(text: &str) -> &'static str {
    let s = text.to_lowercase();
    if s.contains("authentication failed")
        || s.contains("could not read from remote")
        || s.contains("permission denied")
        || s.contains("invalid username or password")
        || s.contains("terminal prompts disabled")
        || s.contains("host key verification failed")
        || s.contains("access denied")
    {
        "auth"
    } else if s.contains("conflict") || s.contains("fix conflicts") {
        "conflict"
    } else if s.contains("would be overwritten")
        || s.contains("commit your changes or stash")
        || s.contains("your local changes")
        || s.contains("please commit your changes")
    {
        "dirty"
    } else if s.contains("non-fast-forward")
        || s.contains("fetch first")
        || s.contains("updates were rejected")
    {
        "non_fast_forward"
    } else if s.contains("no tracking information") || s.contains("no upstream") {
        "no_upstream"
    } else if s.contains("index.lock") || s.contains("another git process") {
        "lock"
    } else if s.contains("could not resolve host")
        || s.contains("unable to access")
        || s.contains("connection timed out")
        || s.contains("network is unreachable")
        || s.contains("operation timed out")
    {
        "network"
    } else {
        "error"
    }
}

/// Branch / upstream / ahead-behind / dirty snapshot.
#[tauri::command]
pub async fn git_sync_status(cwd: String) -> Result<SyncStatus, String> {
    crate::proc::run_blocking(move || git_sync_status_impl(cwd)).await
}

fn git_sync_status_impl(cwd: String) -> Result<SyncStatus, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let wt = work_tree(&cwd)?;

    let head = run(&wt, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch_raw = String::from_utf8_lossy(&head.stdout).trim().to_string();
    let detached = !head.status.success() || branch_raw.is_empty() || branch_raw == "HEAD";
    let branch = if detached { None } else { Some(branch_raw) };

    let has_remote = has_remote(&wt);

    let upstream_out = run(
        &wt,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    )?;
    let upstream = if upstream_out.status.success() {
        let u = String::from_utf8_lossy(&upstream_out.stdout).trim().to_string();
        if u.is_empty() {
            None
        } else {
            Some(u)
        }
    } else {
        None
    };

    let (mut ahead, mut behind) = (0u32, 0u32);
    if upstream.is_some() {
        // `--left-right --count @{u}...HEAD` → "<behind>\t<ahead>".
        let counts = run(&wt, &["rev-list", "--left-right", "--count", "@{u}...HEAD"])?;
        if counts.status.success() {
            let text = String::from_utf8_lossy(&counts.stdout);
            let mut it = text.split_whitespace();
            behind = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            ahead = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        }
    }

    let status = run(&wt, &["status", "--porcelain"])?;
    let dirty = !String::from_utf8_lossy(&status.stdout).trim().is_empty();

    Ok(SyncStatus {
        branch,
        upstream,
        ahead,
        behind,
        has_remote,
        detached,
        dirty,
    })
}

/// `git fetch --prune` on the configured remote.
#[tauri::command]
pub async fn git_fetch(cwd: String) -> Result<SyncOutcome, String> {
    crate::proc::run_blocking(move || git_fetch_impl(cwd)).await
}

fn git_fetch_impl(cwd: String) -> Result<SyncOutcome, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let wt = work_tree(&cwd)?;
    if !has_remote(&wt) {
        return Ok(SyncOutcome::new("no_remote", ""));
    }
    let out = run(&wt, &["fetch", "--prune"])?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        Ok(SyncOutcome::new("ok", tail(&stderr)))
    } else {
        Ok(SyncOutcome::new(classify(&stderr), tail(&stderr)))
    }
}

/// `git pull --no-edit --no-rebase` (fetch + merge).
#[tauri::command]
pub async fn git_pull(cwd: String) -> Result<SyncOutcome, String> {
    crate::proc::run_blocking(move || git_pull_impl(cwd)).await
}

fn git_pull_impl(cwd: String) -> Result<SyncOutcome, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let wt = work_tree(&cwd)?;
    if !has_remote(&wt) {
        return Ok(SyncOutcome::new("no_remote", ""));
    }
    let out = run(&wt, &["pull", "--no-edit", "--no-rebase"])?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        let kind = if stdout.contains("Already up to date") {
            "up_to_date"
        } else {
            "updated"
        };
        return Ok(SyncOutcome::new(kind, tail(&stdout)));
    }
    let combined = format!("{stdout}\n{stderr}");
    Ok(SyncOutcome::new(classify(&combined), tail(&stderr)))
}

/// `git push`. When `set_upstream` is true, pushes the current branch with
/// `--set-upstream origin <branch>` (used when no tracking branch exists yet).
#[tauri::command]
pub async fn git_push(cwd: String, set_upstream: bool) -> Result<SyncOutcome, String> {
    crate::proc::run_blocking(move || git_push_impl(cwd, set_upstream)).await
}

fn git_push_impl(cwd: String, set_upstream: bool) -> Result<SyncOutcome, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let wt = work_tree(&cwd)?;
    if !has_remote(&wt) {
        return Ok(SyncOutcome::new("no_remote", ""));
    }
    let out = if set_upstream {
        let head = run(&wt, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        let branch = String::from_utf8_lossy(&head.stdout).trim().to_string();
        if branch.is_empty() || branch == "HEAD" {
            return Ok(SyncOutcome::new("error", "detached HEAD"));
        }
        run(&wt, &["push", "--set-upstream", "origin", &branch])?
    } else {
        run(&wt, &["push"])?
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        let kind = if stderr.contains("Everything up-to-date") {
            "up_to_date"
        } else {
            "ok"
        };
        Ok(SyncOutcome::new(kind, tail(&stderr)))
    } else {
        Ok(SyncOutcome::new(classify(&stderr), tail(&stderr)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_auth_failures() {
        assert_eq!(classify("fatal: Authentication failed for 'https://...'"), "auth");
        assert_eq!(classify("git@github.com: Permission denied (publickey)."), "auth");
        assert_eq!(
            classify("could not read from remote repository"),
            "auth"
        );
        assert_eq!(classify("Host key verification failed."), "auth");
    }

    #[test]
    fn classify_non_fast_forward() {
        let stderr = "! [rejected]        main -> main (non-fast-forward)\nerror: failed to push some refs";
        assert_eq!(classify(stderr), "non_fast_forward");
        assert_eq!(classify("Updates were rejected because the tip is behind"), "non_fast_forward");
    }

    #[test]
    fn classify_conflict_and_dirty() {
        assert_eq!(classify("CONFLICT (content): Merge conflict in a.txt"), "conflict");
        assert_eq!(
            classify("error: Your local changes to the following files would be overwritten by merge"),
            "dirty"
        );
        assert_eq!(classify("Please commit your changes or stash them"), "dirty");
    }

    #[test]
    fn classify_upstream_lock_network() {
        assert_eq!(
            classify("There is no tracking information for the current branch."),
            "no_upstream"
        );
        assert_eq!(classify("fatal: Unable to create '.git/index.lock': File exists"), "lock");
        assert_eq!(classify("fatal: unable to access 'https://...': Could not resolve host: github.com"), "network");
    }

    #[test]
    fn classify_unknown_is_error() {
        assert_eq!(classify("something completely unexpected"), "error");
    }
}
