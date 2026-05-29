//! Working-tree status, per-file diffs, stage/unstage, and a debounced
//! filesystem watcher that emits `git_status_dirty` whenever the work tree
//! or `.git` index changes. Used by the sidebar `File Diff` section.

use crate::git_info::{find_git_dir, git_cli_available};
use notify::event::{AccessKind, EventKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const GIT_MISSING_CODE: &str = "git_missing";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineStats {
    pub added: u32,
    pub removed: u32,
}

/// Minimal change record exposed to the frontend. One row per `rel_path`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub rel_path: String,
    /// `"modified" | "added" | "deleted" | "renamed" | "untracked" | "conflicted"`.
    pub status: String,
    pub staged: bool,
    pub unstaged: bool,
    pub staged_stats: Option<LineStats>,
    pub unstaged_stats: Option<LineStats>,
}

/// Payload of the `git_status_dirty` window event. Frontend listeners
/// debounce on top of the backend's 300ms window.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusDirtyPayload {
    pub cwd: String,
}

#[tauri::command]
pub fn git_status_changes(cwd: String) -> Result<Vec<ChangedFile>, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Err("cwd is empty".into());
    }
    let start = Path::new(trimmed);
    let git_dir = find_git_dir(start).ok_or_else(|| "not a git repository".to_string())?;
    let work_tree = git_dir
        .parent()
        .ok_or_else(|| "invalid git dir".to_string())?
        .to_path_buf();

    let porcelain = run_git(&work_tree, &["status", "--porcelain=v1", "-z"])?;
    let mut entries = parse_porcelain(&porcelain);

    let unstaged = run_git(&work_tree, &["diff", "--numstat", "-z"])?;
    let staged = run_git(&work_tree, &["diff", "--cached", "--numstat", "-z"])?;
    let unstaged_counts = parse_numstat(&unstaged);
    let staged_counts = parse_numstat(&staged);

    for entry in &mut entries {
        let path = entry.rel_path.clone();
        let mut unstaged_stats = unstaged_counts
            .get(&path)
            .copied()
            .and_then(normalize_stats);
        let mut staged_stats = staged_counts.get(&path).copied().and_then(normalize_stats);
        if entry.status == "untracked" {
            let lines = count_file_lines(&work_tree.join(&path)).unwrap_or(0);
            unstaged_stats = normalize_stats((lines, 0));
            staged_stats = None;
        }
        entry.staged_stats = if entry.staged { staged_stats } else { None };
        entry.unstaged_stats = if entry.unstaged { unstaged_stats } else { None };
    }

    entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(entries)
}

#[tauri::command]
pub fn git_file_diff(cwd: String, rel_path: String, staged: bool) -> Result<String, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Err("cwd is empty".into());
    }
    let rel = rel_path.trim();
    if rel.is_empty() {
        return Err("rel_path is empty".into());
    }
    let start = Path::new(trimmed);
    let git_dir = find_git_dir(start).ok_or_else(|| "not a git repository".to_string())?;
    let work_tree = git_dir
        .parent()
        .ok_or_else(|| "invalid git dir".to_string())?
        .to_path_buf();

    let mut args: Vec<&str> = vec!["diff"];
    if staged {
        args.push("--cached");
    }
    args.push("--no-color");
    args.push("--");
    args.push(rel);
    let output = run_git(&work_tree, &args)?;
    if !output.is_empty() {
        return Ok(output);
    }
    let synth = synthesize_untracked_diff(&work_tree, rel);
    Ok(synth.unwrap_or_default())
}

#[tauri::command]
pub fn git_stage_file(cwd: String, rel_path: String) -> Result<(), String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let work_tree = resolve_work_tree(&cwd)?;
    let rel = rel_path.trim();
    if rel.is_empty() {
        return Err("rel_path is empty".into());
    }
    run_git(&work_tree, &["add", "--", rel]).map(|_| ())
}

#[tauri::command]
pub fn git_unstage_file(cwd: String, rel_path: String) -> Result<(), String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let work_tree = resolve_work_tree(&cwd)?;
    let rel = rel_path.trim();
    if rel.is_empty() {
        return Err("rel_path is empty".into());
    }
    run_git(&work_tree, &["restore", "--staged", "--", rel]).map(|_| ())
}

#[tauri::command]
pub fn git_stage_all(cwd: String) -> Result<(), String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let work_tree = resolve_work_tree(&cwd)?;
    // `add -A` stages modifications, new files, and deletions in one pass.
    run_git(&work_tree, &["add", "-A"]).map(|_| ())
}

#[tauri::command]
pub fn git_unstage_all(cwd: String) -> Result<(), String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let work_tree = resolve_work_tree(&cwd)?;
    // `reset` needs a HEAD to reset against. A repo without an initial commit
    // has nothing to reset to, so unstage by removing entries from the index.
    let has_head = run_git(&work_tree, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_ok();
    if has_head {
        run_git(&work_tree, &["reset", "-q"]).map(|_| ())
    } else {
        run_git(&work_tree, &["rm", "-r", "--cached", "-q", "--", "."]).map(|_| ())
    }
}

#[tauri::command]
pub fn git_commit(cwd: String, message: String) -> Result<(), String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let work_tree = resolve_work_tree(&cwd)?;
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err("commit message is empty".into());
    }
    run_git(&work_tree, &["commit", "-m", trimmed]).map(|_| ())
}

fn resolve_work_tree(cwd: &str) -> Result<PathBuf, String> {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Err("cwd is empty".into());
    }
    let start = Path::new(trimmed);
    let git_dir = find_git_dir(start).ok_or_else(|| "not a git repository".to_string())?;
    git_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "invalid git dir".to_string())
}

fn run_git(work_tree: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(work_tree)
        .args(args)
        .output()
        .map_err(|e| format!("git {}: {e}", args.first().copied().unwrap_or("?")))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or("?"),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Parse `git status --porcelain=v1 -z` (NUL-separated, 2-byte XY status,
/// space, path; rename entries carry the source path in the next NUL-record).
fn parse_porcelain(text: &str) -> Vec<ChangedFile> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes.len().saturating_sub(i) < 3 {
            break;
        }
        let x = bytes[i] as char;
        let y = bytes[i + 1] as char;
        // bytes[i + 2] is a single space separator, then path until NUL.
        let mut j = i + 3;
        while j < bytes.len() && bytes[j] != 0 {
            j += 1;
        }
        let path = String::from_utf8_lossy(&bytes[i + 3..j]).to_string();
        let mut renamed_consumes_extra = false;
        if x == 'R' || y == 'R' {
            // Skip the source path of a rename (next NUL-record).
            let mut k = j + 1;
            while k < bytes.len() && bytes[k] != 0 {
                k += 1;
            }
            j = k;
            renamed_consumes_extra = true;
        }
        let staged = x != ' ' && x != '?';
        let unstaged = y != ' ' && y != '?';
        let status = match (x, y, renamed_consumes_extra) {
            ('?', '?', _) => "untracked",
            ('U', _, _) | (_, 'U', _) | ('A', 'A', _) | ('D', 'D', _) => "conflicted",
            (_, _, true) => "renamed",
            ('A', _, _) | (_, 'A', _) => "added",
            ('D', _, _) | (_, 'D', _) => "deleted",
            _ => "modified",
        };
        out.push(ChangedFile {
            rel_path: path,
            status: status.to_string(),
            staged,
            unstaged: unstaged || (x == '?' && y == '?'),
            staged_stats: None,
            unstaged_stats: None,
        });
        i = j + 1;
    }
    out
}

/// Parse `git diff --numstat -z` output. Format per record:
/// `added\tdeleted\t<NUL>oldname<NUL>newname<NUL>` for renames, otherwise
/// `added\tdeleted\tpath<NUL>`. Binary files use `-` for the counts.
fn normalize_stats((added, removed): (u32, u32)) -> Option<LineStats> {
    if added == 0 && removed == 0 {
        return None;
    }
    Some(LineStats { added, removed })
}

fn parse_numstat(text: &str) -> HashMap<String, (u32, u32)> {
    let mut out = HashMap::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // Read header up to NUL.
        let mut j = i;
        while j < bytes.len() && bytes[j] != 0 {
            j += 1;
        }
        let header = String::from_utf8_lossy(&bytes[i..j]).to_string();
        let mut parts = header.splitn(3, '\t');
        let added = parts
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let removed = parts
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let path_field = parts.next().unwrap_or("");
        i = j + 1;
        let path = if path_field.is_empty() {
            // Rename: read source<NUL>dest<NUL>; we only care about the new name.
            let mut k = i;
            while k < bytes.len() && bytes[k] != 0 {
                k += 1;
            }
            i = k + 1;
            let mut m = i;
            while m < bytes.len() && bytes[m] != 0 {
                m += 1;
            }
            let new_name = String::from_utf8_lossy(&bytes[i..m]).to_string();
            i = m + 1;
            new_name
        } else {
            path_field.to_string()
        };
        if !path.is_empty() {
            out.insert(path, (added, removed));
        }
    }
    out
}

fn count_file_lines(path: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(path).ok()?;
    if text.is_empty() {
        return Some(0);
    }
    let mut lines = text.lines().count() as u32;
    if !text.ends_with('\n') {
        lines = lines.saturating_add(1);
    }
    Some(lines)
}

/// Build a `+++` only diff for files git does not yet know about so the
/// frontend can render them in the same viewer as tracked changes.
fn synthesize_untracked_diff(work_tree: &Path, rel: &str) -> Option<String> {
    let abs = work_tree.join(rel);
    let text = std::fs::read_to_string(&abs).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let mut out = String::new();
    out.push_str(&format!("diff --git a/{rel} b/{rel}\n"));
    out.push_str("new file mode 100644\n");
    out.push_str("--- /dev/null\n");
    out.push_str(&format!("+++ b/{rel}\n"));
    if total > 0 {
        out.push_str(&format!("@@ -0,0 +1,{total} @@\n"));
        for l in lines {
            out.push('+');
            out.push_str(l);
            out.push('\n');
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Filesystem watcher
// ---------------------------------------------------------------------------

/// Per-watcher record. Holding the `RecommendedWatcher` keeps inotify /
/// FSEvents / ReadDirectoryChangesW handles alive; dropping it stops events.
struct WatchHandle {
    /// Owned watcher; kept alive in the `HashMap` until the token is freed.
    _watcher: RecommendedWatcher,
    /// Set to `false` from `git_status_watch_stop` so the debounce thread
    /// exits cleanly on the next tick.
    active: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct GitWatcherState {
    next_token: AtomicU64,
    handles: Mutex<HashMap<u64, WatchHandle>>,
}

const DEBOUNCE_WINDOW: Duration = Duration::from_millis(300);

#[tauri::command]
pub fn git_status_watch_start(app: AppHandle, cwd: String) -> Result<u64, String> {
    let work_tree = resolve_work_tree(&cwd)?;
    let state = app.state::<GitWatcherState>();
    let token = state.next_token.fetch_add(1, Ordering::Relaxed) + 1;

    let active = Arc::new(AtomicBool::new(true));
    let active_for_thread = active.clone();
    let cwd_for_thread = cwd.clone();
    let app_for_thread = app.clone();
    let work_tree_for_filter = work_tree.clone();
    let last_dirty = Arc::new(Mutex::new(None::<Instant>));
    let last_dirty_for_thread = last_dirty.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else {
            return;
        };
        if !relevant_event(&event, &work_tree_for_filter) {
            return;
        }
        let mut guard = match last_dirty_for_thread.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        *guard = Some(Instant::now());
    })
    .map_err(|e| format!("notify init: {e}"))?;

    watcher
        .watch(&work_tree, RecursiveMode::Recursive)
        .map_err(|e| format!("notify watch root: {e}"))?;

    // Watch the resolved `.git` dir as well — submodule-style worktrees
    // place it outside `work_tree`, so the recursive watch above misses it.
    if let Some(git_dir) = find_git_dir(Path::new(work_tree.as_path())) {
        if !git_dir.starts_with(&work_tree) {
            let _ = watcher.watch(&git_dir, RecursiveMode::Recursive);
        }
    }

    std::thread::spawn(move || {
        let mut last_emitted: Option<Instant> = None;
        while active_for_thread.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(120));
            let pending = {
                match last_dirty.lock() {
                    Ok(g) => *g,
                    Err(_) => break,
                }
            };
            let Some(stamp) = pending else { continue };
            if stamp.elapsed() < DEBOUNCE_WINDOW {
                continue;
            }
            // Skip if we already emitted for this exact stamp.
            if last_emitted == Some(stamp) {
                continue;
            }
            last_emitted = Some(stamp);
            let payload = GitStatusDirtyPayload {
                cwd: cwd_for_thread.clone(),
            };
            let _ = app_for_thread.emit("git_status_dirty", payload);
        }
    });

    state
        .handles
        .lock()
        .map_err(|_| "watcher state poisoned".to_string())?
        .insert(
            token,
            WatchHandle {
                _watcher: watcher,
                active,
            },
        );
    Ok(token)
}

#[tauri::command]
pub fn git_status_watch_stop(app: AppHandle, token: u64) -> Result<(), String> {
    let state = app.state::<GitWatcherState>();
    let removed = state
        .handles
        .lock()
        .map_err(|_| "watcher state poisoned".to_string())?
        .remove(&token);
    if let Some(handle) = removed {
        handle.active.store(false, Ordering::Relaxed);
    }
    Ok(())
}

/// Filter out noisy events that don't affect index/working-tree state.
/// `.git/objects/` rewrites every commit, packfile churns even on `git gc`;
/// status output is unchanged so suppressing them keeps the debounce sane.
fn relevant_event(event: &notify::Event, work_tree: &Path) -> bool {
    match event.kind {
        EventKind::Access(AccessKind::Read) | EventKind::Access(AccessKind::Open(_)) => {
            return false
        }
        _ => {}
    }
    if event.paths.is_empty() {
        return true;
    }
    event
        .paths
        .iter()
        .any(|p| !is_ignored_subpath(p, work_tree))
}

fn is_ignored_subpath(path: &Path, work_tree: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(work_tree) else {
        return false;
    };
    let mut comps = rel.components();
    let first = comps.next().and_then(|c| c.as_os_str().to_str());
    let second = comps.next().and_then(|c| c.as_os_str().to_str());
    matches!(
        (first, second),
        (Some(".git"), Some("objects")) | (Some(".git"), Some("logs"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_porcelain_basic() {
        // " M src/foo.rs\0?? notes.txt\0"
        let raw = " M src/foo.rs\0?? notes.txt\0";
        let v = parse_porcelain(raw);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].rel_path, "src/foo.rs");
        assert!(v[0].unstaged);
        assert!(!v[0].staged);
        assert_eq!(v[0].status, "modified");
        assert_eq!(v[1].rel_path, "notes.txt");
        assert_eq!(v[1].status, "untracked");
        assert!(v[1].unstaged);
    }

    #[test]
    fn parse_porcelain_rename_skips_source() {
        // "R  new\0old\0 M other\0"
        let raw = "R  new\0old\0 M other\0";
        let v = parse_porcelain(raw);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].rel_path, "new");
        assert_eq!(v[0].status, "renamed");
        assert_eq!(v[1].rel_path, "other");
        assert_eq!(v[1].status, "modified");
    }

    #[test]
    fn parse_numstat_simple() {
        let raw = "3\t2\tsrc/a.rs\09\t0\tnotes.md\0";
        let m = parse_numstat(raw);
        assert_eq!(m.get("src/a.rs"), Some(&(3, 2)));
        assert_eq!(m.get("notes.md"), Some(&(9, 0)));
    }

    #[test]
    fn synthesize_untracked_diff_emits_plus_only_block() {
        let tmp = std::env::temp_dir().join(format!("blx_diff_synth_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let target = tmp.join("hi.txt");
        std::fs::write(&target, "alpha\nbeta\n").unwrap();
        let diff = synthesize_untracked_diff(&tmp, "hi.txt").unwrap();
        assert!(diff.contains("+++ b/hi.txt"));
        assert!(diff.contains("+alpha"));
        assert!(diff.contains("@@ -0,0 +1,2 @@"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ignored_subpath_filters_objects_dir() {
        let root = Path::new("/tmp/blx-test");
        let p = root.join(".git").join("objects").join("pack").join("x");
        assert!(is_ignored_subpath(&p, root));
        let p = root.join(".git").join("HEAD");
        assert!(!is_ignored_subpath(&p, root));
        let p = root.join("src").join("foo.rs");
        assert!(!is_ignored_subpath(&p, root));
    }
}
