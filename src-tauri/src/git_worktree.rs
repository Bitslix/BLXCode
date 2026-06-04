use crate::git_info::{git_cli_available, resolve_work_tree};
use crate::git_remote::{remote_work_tree, run_git_remote};
use crate::git_status::GIT_MISSING_CODE;
use crate::proc::command;
use crate::pty_host::PtyManager;
use crate::ssh_exec::RemoteExecManager;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Output;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeEntry {
    pub path: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub detached: bool,
    pub bare: bool,
    pub locked: bool,
    pub locked_reason: Option<String>,
    pub prunable: bool,
    pub prunable_reason: Option<String>,
    pub is_main: bool,
    pub git_common_dir: Option<String>,
    pub main_worktree_cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeCreateOutcome {
    pub entry: GitWorktreeEntry,
    pub created: bool,
    pub matched_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeRemoveOutcome {
    pub path: String,
    pub removed: bool,
}

#[tauri::command]
pub async fn git_worktree_list(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    cwd: String,
    connection_id: Option<String>,
) -> Result<Vec<GitWorktreeEntry>, String> {
    if let Some(cid) = connection_id {
        return git_worktree_list_remote(&app, &pty, &exec, &cid, &cwd);
    }
    crate::proc::run_blocking(move || git_worktree_list_impl(&cwd)).await
}

#[tauri::command]
pub async fn git_worktree_open_info(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    cwd: String,
    connection_id: Option<String>,
) -> Result<GitWorktreeEntry, String> {
    if let Some(cid) = connection_id {
        return git_worktree_open_info_remote(&app, &pty, &exec, &cid, &cwd);
    }
    crate::proc::run_blocking(move || git_worktree_open_info_impl(&cwd)).await
}

#[tauri::command]
pub async fn git_worktree_create(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    base_cwd: String,
    branch: String,
    start_point: Option<String>,
    path: String,
    connection_id: Option<String>,
) -> Result<GitWorktreeCreateOutcome, String> {
    if let Some(cid) = connection_id {
        return git_worktree_create_remote(
            &app,
            &pty,
            &exec,
            &cid,
            &base_cwd,
            &branch,
            start_point.as_deref(),
            &path,
        );
    }
    crate::proc::run_blocking(move || {
        git_worktree_create_impl(&base_cwd, &branch, start_point.as_deref(), &path)
    })
    .await
}

#[tauri::command]
pub async fn git_worktree_remove(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    base_cwd: String,
    path: String,
    connection_id: Option<String>,
) -> Result<GitWorktreeRemoveOutcome, String> {
    if let Some(cid) = connection_id {
        return git_worktree_remove_remote(&app, &pty, &exec, &cid, &base_cwd, &path);
    }
    crate::proc::run_blocking(move || git_worktree_remove_impl(&base_cwd, &path)).await
}

fn git_worktree_list_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    cwd: &str,
) -> Result<Vec<GitWorktreeEntry>, String> {
    let base = remote_work_tree(app, pty, exec, cid, cwd)?;
    let stdout = run_git_remote(
        app,
        pty,
        exec,
        cid,
        &base,
        &["worktree", "list", "--porcelain", "-z"],
    )?;
    let mut entries = parse_worktree_porcelain(&stdout);
    enrich_remote_entries(app, pty, exec, cid, &mut entries);
    Ok(entries)
}

fn git_worktree_open_info_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    cwd: &str,
) -> Result<GitWorktreeEntry, String> {
    let work_tree = remote_work_tree(app, pty, exec, cid, cwd)?;
    let key = normalize_remote_path_key(&work_tree);
    git_worktree_list_remote(app, pty, exec, cid, &work_tree)?
        .into_iter()
        .find(|entry| normalize_remote_path_key(&entry.path) == key)
        .ok_or_else(|| "worktree not found in git worktree list".to_string())
}

fn git_worktree_create_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    base_cwd: &str,
    branch: &str,
    start_point: Option<&str>,
    path: &str,
) -> Result<GitWorktreeCreateOutcome, String> {
    let base = remote_work_tree(app, pty, exec, cid, base_cwd)?;
    let branch = validate_branch_name(branch)?;
    let path = validate_worktree_path(path)?;

    let before = git_worktree_list_remote(app, pty, exec, cid, &base)?;
    if let Some(existing) = find_existing_worktree_remote(&before, Some(&branch), Some(&path)) {
        return Ok(GitWorktreeCreateOutcome {
            entry: existing,
            created: false,
            matched_existing: true,
        });
    }

    let mut args = vec![
        "worktree".to_string(),
        "add".to_string(),
        "-b".to_string(),
        branch.clone(),
        path.clone(),
    ];
    if let Some(start_point) = start_point.map(str::trim).filter(|value| !value.is_empty()) {
        validate_start_point(start_point)?;
        args.push(start_point.to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_git_remote(app, pty, exec, cid, &base, &arg_refs)?;

    let after = git_worktree_list_remote(app, pty, exec, cid, &base)?;
    let entry = find_existing_worktree_remote(&after, Some(&branch), Some(&path))
        .ok_or_else(|| "created worktree was not reported by git worktree list".to_string())?;
    Ok(GitWorktreeCreateOutcome {
        entry,
        created: true,
        matched_existing: false,
    })
}

fn git_worktree_remove_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    base_cwd: &str,
    path: &str,
) -> Result<GitWorktreeRemoveOutcome, String> {
    let base = remote_work_tree(app, pty, exec, cid, base_cwd)?;
    let path = validate_worktree_path(path)?;
    let worktree = remote_work_tree(app, pty, exec, cid, &path)?;
    let status = run_git_remote(
        app,
        pty,
        exec,
        cid,
        &worktree,
        &["status", "--porcelain=v1", "-z"],
    )?;
    if !status.is_empty() {
        return Err("worktree has uncommitted changes".into());
    }
    run_git_remote(app, pty, exec, cid, &base, &["worktree", "remove", &path])?;
    Ok(GitWorktreeRemoveOutcome {
        path,
        removed: true,
    })
}

fn git_worktree_list_impl(cwd: &str) -> Result<Vec<GitWorktreeEntry>, String> {
    ensure_git_cli()?;
    let base = resolve_required_work_tree(cwd)?;
    let output = run_git(&base, &["worktree", "list", "--porcelain", "-z"])?;
    if !output.status.success() {
        return Err(git_error("worktree list", &output));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = parse_worktree_porcelain(&stdout);
    enrich_entries(&mut entries);
    Ok(entries)
}

fn git_worktree_open_info_impl(cwd: &str) -> Result<GitWorktreeEntry, String> {
    let work_tree = resolve_required_work_tree(cwd)?;
    let work_tree_key = normalize_path_key(&work_tree);
    let entries = git_worktree_list_impl(&work_tree_key)?;
    entries
        .into_iter()
        .find(|entry| normalize_path_key(Path::new(&entry.path)) == work_tree_key)
        .ok_or_else(|| "worktree not found in git worktree list".to_string())
}

fn git_worktree_create_impl(
    base_cwd: &str,
    branch: &str,
    start_point: Option<&str>,
    path: &str,
) -> Result<GitWorktreeCreateOutcome, String> {
    ensure_git_cli()?;
    let base = resolve_required_work_tree(base_cwd)?;
    let branch = validate_branch_name(branch)?;
    let path = validate_worktree_path(path)?;

    let before = git_worktree_list_impl(&normalize_path_key(&base))?;
    if let Some(existing) = find_existing_worktree(&before, Some(&branch), Some(&path)) {
        return Ok(GitWorktreeCreateOutcome {
            entry: existing,
            created: false,
            matched_existing: true,
        });
    }

    let mut args = vec![
        "worktree".to_string(),
        "add".to_string(),
        "-b".to_string(),
        branch.clone(),
        path.clone(),
    ];
    if let Some(start_point) = start_point.map(str::trim).filter(|value| !value.is_empty()) {
        validate_start_point(start_point)?;
        args.push(start_point.to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = run_git(&base, &arg_refs)?;
    if !output.status.success() {
        return Err(git_error("worktree add", &output));
    }

    let after = git_worktree_list_impl(&normalize_path_key(&base))?;
    let entry = find_existing_worktree(&after, Some(&branch), Some(&path))
        .ok_or_else(|| "created worktree was not reported by git worktree list".to_string())?;
    Ok(GitWorktreeCreateOutcome {
        entry,
        created: true,
        matched_existing: false,
    })
}

fn git_worktree_remove_impl(
    base_cwd: &str,
    path: &str,
) -> Result<GitWorktreeRemoveOutcome, String> {
    ensure_git_cli()?;
    let base = resolve_required_work_tree(base_cwd)?;
    let path = validate_worktree_path(path)?;
    let worktree = resolve_required_work_tree(&path)?;
    let status = run_git(&worktree, &["status", "--porcelain=v1", "-z"])?;
    if !status.status.success() {
        return Err(git_error("status", &status));
    }
    if !status.stdout.is_empty() {
        return Err("worktree has uncommitted changes".into());
    }

    let output = run_git(&base, &["worktree", "remove", "--", &path])?;
    if !output.status.success() {
        return Err(git_error("worktree remove", &output));
    }
    Ok(GitWorktreeRemoveOutcome {
        path,
        removed: true,
    })
}

pub(crate) fn parse_worktree_porcelain(text: &str) -> Vec<GitWorktreeEntry> {
    let mut entries = Vec::new();
    let mut current: Option<GitWorktreeEntry> = None;
    for raw in porcelain_fields(text) {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(GitWorktreeEntry {
                path: path.to_string(),
                branch: None,
                head: None,
                detached: false,
                bare: false,
                locked: false,
                locked_reason: None,
                prunable: false,
                prunable_reason: None,
                is_main: entries.is_empty(),
                git_common_dir: None,
                main_worktree_cwd: None,
            });
            continue;
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        if let Some(head) = line.strip_prefix("HEAD ") {
            entry.head = Some(head.to_string());
        } else if let Some(branch) = line.strip_prefix("branch ") {
            entry.branch = Some(normalize_branch_ref(branch));
        } else if line == "detached" {
            entry.detached = true;
        } else if line == "bare" {
            entry.bare = true;
        } else if let Some(reason) = line.strip_prefix("locked") {
            entry.locked = true;
            let reason = reason.trim();
            if !reason.is_empty() {
                entry.locked_reason = Some(reason.to_string());
            }
        } else if let Some(reason) = line.strip_prefix("prunable") {
            entry.prunable = true;
            let reason = reason.trim();
            if !reason.is_empty() {
                entry.prunable_reason = Some(reason.to_string());
            }
        }
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    if let Some(main) = entries.first().map(|entry| entry.path.clone()) {
        for entry in &mut entries {
            entry.main_worktree_cwd = Some(main.clone());
            entry.is_main = entry.path == main;
            entry.detached = entry.detached || entry.branch.is_none();
        }
    }
    entries
}

fn porcelain_fields(text: &str) -> Vec<&str> {
    if text.contains('\0') {
        text.split('\0').collect()
    } else {
        text.lines().collect()
    }
}

fn enrich_entries(entries: &mut [GitWorktreeEntry]) {
    let main = entries.first().map(|entry| entry.path.clone());
    for entry in entries {
        entry.main_worktree_cwd = main.clone();
        entry.git_common_dir = git_common_dir(Path::new(&entry.path));
    }
}

fn enrich_remote_entries(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    entries: &mut [GitWorktreeEntry],
) {
    let main = entries.first().map(|entry| entry.path.clone());
    for entry in entries {
        entry.main_worktree_cwd = main.clone();
        entry.git_common_dir = git_common_dir_remote(app, pty, exec, cid, &entry.path);
    }
}

fn git_common_dir(work_tree: &Path) -> Option<String> {
    let output = run_git(work_tree, &["rev-parse", "--git-common-dir"]).ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    let path = if path.is_absolute() {
        path
    } else {
        work_tree.join(path)
    };
    Some(normalize_path_key(&path))
}

fn git_common_dir_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    work_tree: &str,
) -> Option<String> {
    let output = run_git_remote(
        app,
        pty,
        exec,
        cid,
        work_tree,
        &["rev-parse", "--git-common-dir"],
    )
    .ok()?;
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('/') {
        Some(normalize_remote_path_key(trimmed))
    } else {
        Some(join_remote_path(work_tree, trimmed))
    }
}

fn find_existing_worktree(
    entries: &[GitWorktreeEntry],
    branch: Option<&str>,
    path: Option<&str>,
) -> Option<GitWorktreeEntry> {
    let branch = branch.map(normalize_branch_ref);
    let path = path.map(|value| normalize_path_key(Path::new(value)));
    entries.iter().find_map(|entry| {
        let branch_matches = branch
            .as_deref()
            .zip(entry.branch.as_deref())
            .is_some_and(|(expected, actual)| expected == actual);
        let path_matches = path
            .as_deref()
            .is_some_and(|expected| expected == normalize_path_key(Path::new(&entry.path)));
        if branch_matches || path_matches {
            Some(entry.clone())
        } else {
            None
        }
    })
}

fn find_existing_worktree_remote(
    entries: &[GitWorktreeEntry],
    branch: Option<&str>,
    path: Option<&str>,
) -> Option<GitWorktreeEntry> {
    let branch = branch.map(normalize_branch_ref);
    let path = path.map(normalize_remote_path_key);
    entries.iter().find_map(|entry| {
        let branch_matches = branch
            .as_deref()
            .zip(entry.branch.as_deref())
            .is_some_and(|(expected, actual)| expected == actual);
        let path_matches = path
            .as_deref()
            .is_some_and(|expected| expected == normalize_remote_path_key(&entry.path));
        if branch_matches || path_matches {
            Some(entry.clone())
        } else {
            None
        }
    })
}

fn ensure_git_cli() -> Result<(), String> {
    if git_cli_available() {
        Ok(())
    } else {
        Err(GIT_MISSING_CODE.into())
    }
}

fn resolve_required_work_tree(cwd: &str) -> Result<PathBuf, String> {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Err("cwd is empty".into());
    }
    resolve_work_tree(Path::new(trimmed)).ok_or_else(|| "not a git repository".to_string())
}

fn validate_branch_name(branch: &str) -> Result<String, String> {
    let branch = branch.trim();
    if branch.is_empty() {
        return Err("branch is empty".into());
    }
    if branch.chars().any(char::is_whitespace) {
        return Err("branch must not contain whitespace".into());
    }
    Ok(normalize_branch_ref(branch))
}

fn validate_start_point(start_point: &str) -> Result<(), String> {
    if start_point.chars().any(char::is_whitespace) {
        return Err("start point must not contain whitespace".into());
    }
    Ok(())
}

fn validate_worktree_path(path: &str) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("worktree path is empty".into());
    }
    if path.starts_with('-') {
        return Err("worktree path must not start with '-'".into());
    }
    Ok(path.to_string())
}

fn normalize_branch_ref(branch: &str) -> String {
    branch
        .trim()
        .strip_prefix("refs/heads/")
        .unwrap_or(branch.trim())
        .to_string()
}

fn normalize_path_key(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn normalize_remote_path_key(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed == "/" {
        return "/".into();
    }
    trimmed.trim_end_matches('/').to_string()
}

fn join_remote_path(base: &str, rel: &str) -> String {
    let base = normalize_remote_path_key(base);
    let rel = rel.trim_start_matches('/');
    if base == "/" {
        format!("/{rel}")
    } else {
        format!("{base}/{rel}")
    }
}

fn run_git(work_tree: &Path, args: &[&str]) -> Result<Output, String> {
    command("git")
        .arg("-C")
        .arg(work_tree)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("git {}: {e}", args.first().copied().unwrap_or("?")))
}

fn git_error(subcommand: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        format!("git {subcommand}: exit {}", output.status)
    } else {
        format!("git {subcommand}: {stderr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn run_git_test(work_tree: &Path, args: &[&str]) -> Output {
        command("git")
            .arg("-C")
            .arg(work_tree)
            .args(args)
            .output()
            .unwrap_or_else(|err| panic!("git {} failed to spawn: {err}", args.join(" ")))
    }

    #[test]
    fn parses_porcelain_worktree_list() {
        let text = concat!(
            "worktree /repo\0",
            "HEAD 1111111111111111111111111111111111111111\0",
            "branch refs/heads/main\0",
            "\0",
            "worktree /repo-feature\0",
            "HEAD 2222222222222222222222222222222222222222\0",
            "branch refs/heads/feature/worktrees\0",
            "locked reason text\0",
            "\0",
        );

        let parsed = parse_worktree_porcelain(text);

        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].is_main);
        assert_eq!(parsed[0].branch.as_deref(), Some("main"));
        assert_eq!(parsed[1].branch.as_deref(), Some("feature/worktrees"));
        assert!(parsed[1].locked);
        assert_eq!(parsed[1].locked_reason.as_deref(), Some("reason text"));
        assert_eq!(parsed[1].main_worktree_cwd.as_deref(), Some("/repo"));
    }

    #[test]
    fn detects_existing_worktree_by_branch_or_path() {
        let entries = parse_worktree_porcelain(concat!(
            "worktree /repo\n",
            "branch refs/heads/main\n",
            "\n",
            "worktree /repo-feature\n",
            "branch refs/heads/feature\n",
        ));

        assert!(find_existing_worktree(&entries, Some("refs/heads/feature"), None).is_some());
        assert!(find_existing_worktree(&entries, None, Some("/repo-feature")).is_some());
        assert!(find_existing_worktree(&entries, Some("missing"), None).is_none());
    }

    #[test]
    fn remote_existing_detection_normalizes_trailing_slashes() {
        let entries = parse_worktree_porcelain(concat!(
            "worktree /home/me/repo\n",
            "branch refs/heads/main\n",
            "\n",
            "worktree /home/me/repo-feature\n",
            "branch refs/heads/feature\n",
        ));

        assert!(
            find_existing_worktree_remote(&entries, None, Some("/home/me/repo-feature/")).is_some()
        );
        assert_eq!(
            join_remote_path("/home/me/repo/", ".git"),
            "/home/me/repo/.git"
        );
    }

    #[test]
    fn local_backend_creates_lists_opens_and_removes_worktree() {
        if !git_cli_available() {
            return;
        }
        let tmp =
            std::env::temp_dir().join(format!("blx_worktree_backend_{}", uuid::Uuid::new_v4()));
        let repo = tmp.join("repo");
        let feature = tmp.join("feature");
        fs::create_dir_all(&repo).unwrap();

        assert!(run_git_test(&repo, &["init"]).status.success());
        assert!(
            run_git_test(&repo, &["config", "user.email", "test@example.com"])
                .status
                .success()
        );
        assert!(
            run_git_test(&repo, &["config", "user.name", "BLXCode Test"])
                .status
                .success()
        );
        fs::write(repo.join("README.md"), "hello\n").unwrap();
        assert!(run_git_test(&repo, &["add", "README.md"]).status.success());
        assert!(run_git_test(&repo, &["commit", "-m", "init"])
            .status
            .success());

        let created = git_worktree_create_impl(
            repo.to_str().unwrap(),
            "feature/worktree-backend",
            Some("HEAD"),
            feature.to_str().unwrap(),
        )
        .unwrap();
        assert!(created.created);
        assert!(!created.matched_existing);
        assert_eq!(
            created.entry.branch.as_deref(),
            Some("feature/worktree-backend")
        );

        let listed = git_worktree_list_impl(repo.to_str().unwrap()).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(find_existing_worktree(&listed, Some("feature/worktree-backend"), None).is_some());

        let opened = git_worktree_open_info_impl(feature.to_str().unwrap()).unwrap();
        assert_eq!(opened.branch.as_deref(), Some("feature/worktree-backend"));

        fs::write(feature.join("dirty.txt"), "dirty\n").unwrap();
        let dirty_remove =
            git_worktree_remove_impl(repo.to_str().unwrap(), feature.to_str().unwrap())
                .expect_err("dirty worktree removal should be blocked");
        assert!(dirty_remove.contains("uncommitted changes"));
        fs::remove_file(feature.join("dirty.txt")).unwrap();

        let removed =
            git_worktree_remove_impl(repo.to_str().unwrap(), feature.to_str().unwrap()).unwrap();
        assert!(removed.removed);
        assert!(!feature.exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
