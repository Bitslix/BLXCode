//! Commit graph data for the sidebar. The backend returns structured commits
//! and lane geometry; the frontend owns all rendering.

use crate::git_info::{git_cli_available, resolve_work_tree as resolve_git_work_tree};
use crate::git_remote::{remote_is_repository, remote_work_tree, run_git_remote};
use crate::proc::command;
use crate::pty_host::PtyManager;
use crate::ssh_exec::RemoteExecManager;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, State};

pub const GIT_MISSING_CODE: &str = "git_missing";
const DEFAULT_LIMIT: u32 = 100;
const RECORD_END: char = '\x02';
const FIELD_SEP: char = '\x1f';

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRefDecoration {
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitNode {
    pub oid: String,
    pub short_oid: String,
    pub parents: Vec<String>,
    pub subject: String,
    pub body: String,
    pub author: String,
    pub author_email: String,
    pub author_time: String,
    pub rel_time: String,
    pub decorations: Vec<GitRefDecoration>,
    pub files_changed: Option<u32>,
    pub insertions: Option<u32>,
    pub deletions: Option<u32>,
    pub remote_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphEdge {
    pub from_lane: usize,
    pub to_lane: usize,
    pub color_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphEntry {
    pub row_index: usize,
    pub lane: usize,
    pub lanes: usize,
    pub active_lanes: Vec<usize>,
    pub edges: Vec<GitGraphEdge>,
    pub is_merge: bool,
    pub is_head: bool,
    pub commit: GitCommitNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphLayout {
    pub entries: Vec<GitGraphEntry>,
    pub lane_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitFileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitDetails {
    pub oid: String,
    pub short_oid: String,
    pub subject: String,
    pub body: String,
    pub author: String,
    pub author_email: String,
    pub author_time: String,
    pub rel_time: String,
    pub decorations: Vec<GitRefDecoration>,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub remote_url: Option<String>,
    pub files: Vec<GitCommitFileChange>,
}

#[tauri::command]
pub async fn git_is_repository(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    cwd: String,
    connection_id: Option<String>,
) -> Result<bool, String> {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    if let Some(cid) = connection_id {
        return Ok(remote_is_repository(&app, &pty, &exec, &cid, trimmed));
    }
    let cwd = trimmed.to_string();
    crate::proc::run_blocking(move || Ok(crate::git_info::is_git_repository(Path::new(&cwd)))).await
}

#[tauri::command]
pub async fn git_commit_graph(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    cwd: String,
    limit: Option<u32>,
    connection_id: Option<String>,
) -> Result<GitGraphLayout, String> {
    if let Some(cid) = connection_id {
        return git_commit_graph_remote(&app, &pty, &exec, &cid, &cwd, limit);
    }
    crate::proc::run_blocking(move || git_commit_graph_impl(cwd, limit)).await
}

#[tauri::command]
pub async fn git_commit_details(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    cwd: String,
    oid: String,
    connection_id: Option<String>,
) -> Result<GitCommitDetails, String> {
    if let Some(cid) = connection_id {
        return git_commit_details_remote(&app, &pty, &exec, &cid, &cwd, &oid);
    }
    crate::proc::run_blocking(move || git_commit_details_impl(cwd, oid)).await
}

fn graph_log_args(limit: u32, pretty: &str) -> Vec<String> {
    vec![
        "log".into(),
        "--topo-order".into(),
        format!("-n{limit}"),
        "--date=iso-strict".into(),
        format!("--pretty=format:{pretty}"),
    ]
}

fn git_commit_graph_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    cwd: &str,
    limit: Option<u32>,
) -> Result<GitGraphLayout, String> {
    let work_tree = remote_work_tree(app, pty, exec, cid, cwd)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let pretty = graph_pretty_format();
    let args = graph_log_args(limit, &pretty);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let text = run_git_remote(app, pty, exec, cid, &work_tree, &arg_refs)?;
    let remote_url = remote_origin_url_remote(app, pty, exec, cid, &work_tree);
    Ok(parse_graph_layout(&text, remote_url))
}

fn git_commit_graph_impl(cwd: String, limit: Option<u32>) -> Result<GitGraphLayout, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let work_tree = resolve_work_tree(&cwd)?;
    fetch_graph_entries(&work_tree, limit.unwrap_or(DEFAULT_LIMIT))
}

fn fetch_graph_entries(work_tree: &Path, limit: u32) -> Result<GitGraphLayout, String> {
    let pretty = graph_pretty_format();
    let out = command("git")
        .arg("-C")
        .arg(work_tree)
        .args(graph_log_args(limit, &pretty))
        .output()
        .map_err(|e| format!("git log failed: {e}"))?;
    if !out.status.success() {
        return Err(format!("git log: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(parse_graph_layout(&text, remote_origin_url(work_tree)))
}

fn git_commit_details_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    cwd: &str,
    oid: &str,
) -> Result<GitCommitDetails, String> {
    let work_tree = remote_work_tree(app, pty, exec, cid, cwd)?;
    let pretty = details_pretty_format();
    let owned = [
        "show".to_string(),
        "--find-renames".to_string(),
        "--numstat".to_string(),
        "--name-status".to_string(),
        "--date=iso-strict".to_string(),
        format!("--format={pretty}"),
        oid.to_string(),
    ];
    let arg_refs: Vec<&str> = owned.iter().map(String::as_str).collect();
    let text = run_git_remote(app, pty, exec, cid, &work_tree, &arg_refs)?;
    parse_commit_details(
        &text,
        remote_origin_url_remote(app, pty, exec, cid, &work_tree),
    )
}

fn git_commit_details_impl(cwd: String, oid: String) -> Result<GitCommitDetails, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let work_tree = resolve_work_tree(&cwd)?;
    let pretty = details_pretty_format();
    let format_arg = format!("--format={pretty}");
    let out = command("git")
        .arg("-C")
        .arg(&work_tree)
        .args([
            "show",
            "--find-renames",
            "--numstat",
            "--name-status",
            "--date=iso-strict",
            &format_arg,
            &oid,
        ])
        .output()
        .map_err(|e| format!("git show failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git show: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    parse_commit_details(&text, remote_origin_url(&work_tree))
}

fn graph_pretty_format() -> String {
    format!(
        "%H{FIELD_SEP}%h{FIELD_SEP}%P{FIELD_SEP}%s{FIELD_SEP}%b{FIELD_SEP}%an{FIELD_SEP}%ae{FIELD_SEP}%aI{FIELD_SEP}%ar{FIELD_SEP}%D{RECORD_END}"
    )
}

fn details_pretty_format() -> String {
    graph_pretty_format()
}

fn resolve_work_tree(cwd: &str) -> Result<std::path::PathBuf, String> {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Err("cwd is empty".into());
    }
    resolve_git_work_tree(Path::new(trimmed)).ok_or_else(|| "not a git repository".to_string())
}

fn run_git(work_tree: &Path, args: &[&str]) -> Option<String> {
    let out = command("git")
        .arg("-C")
        .arg(work_tree)
        .args(args)
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn remote_origin_url(work_tree: &Path) -> Option<String> {
    run_git(work_tree, &["remote", "get-url", "origin"]).filter(|s| !s.is_empty())
}

fn remote_origin_url_remote(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    cid: &str,
    work_tree: &str,
) -> Option<String> {
    run_git_remote(
        app,
        pty,
        exec,
        cid,
        work_tree,
        &["remote", "get-url", "origin"],
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

fn parse_graph_layout(text: &str, remote_url: Option<String>) -> GitGraphLayout {
    let mut commits: Vec<GitCommitNode> = text
        .split(RECORD_END)
        .filter_map(|record| parse_commit_record(record, remote_url.clone()))
        .collect();
    let head_oid = commits.first().map(|c| c.oid.clone());
    let mut active: Vec<String> = Vec::new();
    let mut entries = Vec::with_capacity(commits.len());
    let mut max_lanes = 1usize;

    for (row_index, commit) in commits.drain(..).enumerate() {
        let lane = active
            .iter()
            .position(|oid| oid == &commit.oid)
            .unwrap_or_else(|| {
                active.insert(0, commit.oid.clone());
                0
            });
        let before_len = active.len();
        let mut after = active.clone();
        if commit.parents.is_empty() {
            after.remove(lane);
        } else {
            after[lane] = commit.parents[0].clone();
            for (offset, parent) in commit.parents.iter().skip(1).enumerate() {
                after.insert(lane + offset + 1, parent.clone());
            }
        }
        let edges = commit
            .parents
            .iter()
            .filter_map(|parent| after.iter().position(|oid| oid == parent))
            .map(|to_lane| GitGraphEdge {
                from_lane: lane,
                to_lane,
                color_index: to_lane,
            })
            .collect::<Vec<_>>();
        let lanes = before_len.max(after.len()).max(lane + 1).max(1);
        max_lanes = max_lanes.max(lanes);
        let active_lanes = (0..lanes).collect();
        entries.push(GitGraphEntry {
            row_index,
            lane,
            lanes,
            active_lanes,
            edges,
            is_merge: commit.parents.len() > 1,
            is_head: head_oid.as_deref() == Some(commit.oid.as_str()),
            commit,
        });
        active = after;
    }

    GitGraphLayout {
        entries,
        lane_count: max_lanes,
    }
}

fn parse_commit_record(record: &str, remote_url: Option<String>) -> Option<GitCommitNode> {
    let trimmed = record.trim_matches('\n').trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed.split(FIELD_SEP).collect();
    if parts.len() < 10 {
        return None;
    }
    let oid = parts[0].trim().to_string();
    if oid.is_empty() {
        return None;
    }
    let parents = parts[2]
        .split_whitespace()
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    Some(GitCommitNode {
        oid,
        short_oid: parts[1].trim().to_string(),
        parents,
        subject: parts[3].trim().to_string(),
        body: parts[4].trim().to_string(),
        author: parts[5].trim().to_string(),
        author_email: parts[6].trim().to_string(),
        author_time: parts[7].trim().to_string(),
        rel_time: parts[8].trim().to_string(),
        decorations: parse_decorations(parts.get(9).copied().unwrap_or("").trim()),
        files_changed: None,
        insertions: None,
        deletions: None,
        remote_url,
    })
}

fn parse_commit_details(
    text: &str,
    remote_url: Option<String>,
) -> Result<GitCommitDetails, String> {
    let (record, rest) = text
        .split_once(RECORD_END)
        .ok_or_else(|| "missing commit detail header".to_string())?;
    let commit = parse_commit_record(record, remote_url.clone())
        .ok_or_else(|| "invalid commit detail header".to_string())?;
    let files = parse_detail_files(rest);
    let insertions = files.iter().filter_map(|f| f.added).sum();
    let deletions = files.iter().filter_map(|f| f.removed).sum();
    Ok(GitCommitDetails {
        oid: commit.oid,
        short_oid: commit.short_oid,
        subject: commit.subject,
        body: commit.body,
        author: commit.author,
        author_email: commit.author_email,
        author_time: commit.author_time,
        rel_time: commit.rel_time,
        decorations: commit.decorations,
        files_changed: files.len() as u32,
        insertions,
        deletions,
        remote_url,
        files,
    })
}

fn parse_detail_files(text: &str) -> Vec<GitCommitFileChange> {
    let mut by_path = std::collections::BTreeMap::<String, GitCommitFileChange>::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 && is_numstat_count(parts[0]) && is_numstat_count(parts[1]) {
            let path = parts.last().copied().unwrap_or_default().to_string();
            let entry = by_path
                .entry(path.clone())
                .or_insert_with(|| GitCommitFileChange {
                    path,
                    old_path: None,
                    status: "modified".into(),
                    added: None,
                    removed: None,
                });
            entry.added = parts[0].parse::<u32>().ok();
            entry.removed = parts[1].parse::<u32>().ok();
            continue;
        }
        if let Some((status, path, old_path)) = parse_name_status(&parts) {
            let entry = by_path
                .entry(path.clone())
                .or_insert_with(|| GitCommitFileChange {
                    path,
                    old_path: old_path.clone(),
                    status: status.clone(),
                    added: None,
                    removed: None,
                });
            entry.status = status;
            entry.old_path = old_path;
        }
    }
    by_path.into_values().collect()
}

fn is_numstat_count(value: &str) -> bool {
    value == "-" || value.parse::<u32>().is_ok()
}

fn parse_name_status(parts: &[&str]) -> Option<(String, String, Option<String>)> {
    let raw = *parts.first()?;
    let first = raw.chars().next()?;
    let status = match first {
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'M' => "modified",
        _ => "modified",
    }
    .to_string();
    if first == 'R' || first == 'C' {
        let old_path = parts.get(1)?.to_string();
        let path = parts.get(2)?.to_string();
        Some((status, path, Some(old_path)))
    } else {
        let path = parts.get(1)?.to_string();
        Some((status, path, None))
    }
}

fn parse_decorations(raw: &str) -> Vec<GitRefDecoration> {
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(',')
        .filter_map(|chunk| {
            let chunk = chunk.trim();
            if chunk.is_empty() {
                return None;
            }
            let (kind, label) = if let Some((k, l)) = chunk.split_once(": ") {
                (k.trim().to_string(), l.trim().to_string())
            } else if chunk.contains(" -> ") {
                let mut parts = chunk.splitn(2, " -> ");
                let _head = parts.next()?;
                let label = parts.next()?.trim().to_string();
                ("branch".to_string(), label)
            } else {
                ("ref".to_string(), chunk.to_string())
            };
            if label.is_empty() {
                return None;
            }
            Some(GitRefDecoration { label, kind })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(oid: &str, parents: &str, subject: &str) -> String {
        format!(
            "{oid}{FIELD_SEP}{short}{FIELD_SEP}{parents}{FIELD_SEP}{subject}{FIELD_SEP}{FIELD_SEP}Maik{FIELD_SEP}m@example.com{FIELD_SEP}2026-06-01T12:00:00+00:00{FIELD_SEP}1 day ago{FIELD_SEP}{RECORD_END}",
            short = &oid[..oid.len().min(7)]
        )
    }

    #[test]
    fn parse_graph_layout_linear_history_uses_lane_zero() {
        let text = format!(
            "{}{}",
            rec("aaaaaaaa", "bbbbbbbb", "one"),
            rec("bbbbbbbb", "", "two")
        );
        let layout = parse_graph_layout(&text, None);
        assert_eq!(layout.entries.len(), 2);
        assert!(layout.entries.iter().all(|e| e.lane == 0));
    }

    #[test]
    fn parse_graph_layout_merge_commit_adds_edges() {
        let text = format!(
            "{}{}{}",
            rec("mmmmmmmm", "aaaaaaaa bbbbbbbb", "merge"),
            rec("aaaaaaaa", "", "left"),
            rec("bbbbbbbb", "", "right")
        );
        let layout = parse_graph_layout(&text, None);
        assert_eq!(layout.entries[0].edges.len(), 2);
        assert!(layout.lane_count >= 2);
    }

    #[test]
    fn parse_decorations_branch() {
        let d = parse_decorations("HEAD -> main, origin/main");
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].label, "main");
    }

    #[test]
    fn parse_commit_details_reads_name_status_and_numstat() {
        let text = format!(
            "{}\n3\t2\tsrc/a.rs\n0\t1\told.txt\nM\tsrc/a.rs\nD\told.txt\n",
            rec("deadbeef", "", "subject")
        );
        let details = parse_commit_details(&text, Some("https://github.com/acme/repo.git".into()))
            .expect("details");
        assert_eq!(details.files_changed, 2);
        assert_eq!(details.insertions, 3);
        assert_eq!(details.deletions, 3);
        assert!(details.files.iter().any(|f| f.status == "deleted"));
    }

    #[test]
    fn parse_detail_files_reads_renamed_paths() {
        let files = parse_detail_files("1\t0\tnew.rs\nR100\told.rs\tnew.rs\n");
        assert_eq!(files[0].path, "new.rs");
        assert_eq!(files[0].old_path.as_deref(), Some("old.rs"));
        assert_eq!(files[0].status, "renamed");
    }
}
