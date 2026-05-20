//! Commit graph layout for the sidebar (git CLI + swim-lane layout).

use crate::git_info::{find_git_dir, git_cli_available};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub const GIT_MISSING_CODE: &str = "git_missing";
const DEFAULT_LIMIT: u32 = 100;

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
    pub parents: Vec<String>,
    pub subject: String,
    pub author: String,
    pub rel_time: String,
    pub decorations: Vec<GitRefDecoration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphRow {
    pub oid: String,
    pub lane: usize,
    pub lane_color_index: usize,
    pub continues_up: bool,
    pub continues_down: bool,
    pub merge_from_lane: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphLayout {
    pub commits: Vec<GitCommitNode>,
    pub rows: Vec<GitGraphRow>,
}

#[tauri::command]
pub fn git_is_repository(cwd: String) -> bool {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return false;
    }
    crate::git_info::is_git_repository(Path::new(trimmed))
}

#[tauri::command]
pub fn git_commit_graph(cwd: String, limit: Option<u32>) -> Result<GitGraphLayout, String> {
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
        .ok_or_else(|| "invalid git dir".to_string())?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let commits = fetch_commits(work_tree, limit)?;
    let rows = compute_lane_layout(&commits);
    Ok(GitGraphLayout { commits, rows })
}

fn fetch_commits(work_tree: &Path, limit: u32) -> Result<Vec<GitCommitNode>, String> {
    let format = "%H\x1f%P\x1f%s\x1f%an\x1f%ar\x1f%D\x1e";
    let out = Command::new("git")
        .arg("-C")
        .arg(work_tree)
        .args([
            "log",
            "--topo-order",
            &format!("-n{limit}"),
            &format!("--format={format}"),
        ])
        .output()
        .map_err(|e| format!("git log failed: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git log: {stderr}"));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut commits = Vec::new();
    for record in text.split('\x1e').filter(|s| !s.trim().is_empty()) {
        let parts: Vec<&str> = record.split('\x1f').collect();
        if parts.len() < 5 {
            continue;
        }
        let oid = parts[0].trim().to_string();
        let parents: Vec<String> = parts[1]
            .split_whitespace()
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        let subject = parts[2].trim().to_string();
        let author = parts[3].trim().to_string();
        let rel_time = parts[4].trim().to_string();
        let deco_raw = parts.get(5).copied().unwrap_or("").trim();
        let decorations = parse_decorations(deco_raw);
        commits.push(GitCommitNode {
            oid,
            parents,
            subject,
            author,
            rel_time,
            decorations,
        });
    }
    Ok(commits)
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

/// Swim-lane layout: `commits` are newest-first (git log order); rows match that order.
fn compute_lane_layout(commits: &[GitCommitNode]) -> Vec<GitGraphRow> {
    if commits.is_empty() {
        return Vec::new();
    }
    let mut oid_to_idx: HashMap<&str, usize> = HashMap::new();
    for (i, c) in commits.iter().enumerate() {
        oid_to_idx.insert(c.oid.as_str(), i);
    }

    // Oldest-first for lane assignment
    let mut oldest_first: Vec<&GitCommitNode> = commits.iter().collect();
    oldest_first.reverse();

    let mut lane_of: HashMap<String, usize> = HashMap::new();
    let mut active: Vec<Option<String>> = Vec::new();
    let mut merge_from: HashMap<String, usize> = HashMap::new();

    for commit in oldest_first {
        let oid = commit.oid.clone();
        let parent_lanes: Vec<usize> = commit
            .parents
            .iter()
            .filter_map(|p| lane_of.get(p).copied())
            .collect();

        let lane = if let Some(&pl) = parent_lanes.first() {
            pl
        } else {
            let new_lane = active.len();
            active.push(None);
            new_lane
        };

        lane_of.insert(oid.clone(), lane);
        active[lane] = Some(oid.clone());

        if commit.parents.len() > 1 {
            for parent in commit.parents.iter().skip(1) {
                if let Some(&pl) = lane_of.get(parent.as_str()) {
                    merge_from.insert(oid.clone(), pl);
                } else {
                    let new_lane = active.len();
                    active.push(Some(parent.clone()));
                    lane_of.insert(parent.clone(), new_lane);
                    merge_from.insert(oid.clone(), new_lane);
                }
            }
        }

        // Close lanes that are not continued by any child
        for (idx, tip) in active.iter_mut().enumerate() {
            if let Some(tip_oid) = tip.clone() {
                let continued = commits.iter().any(|c| {
                    c.parents.contains(&tip_oid) && lane_of.get(c.oid.as_str()) == Some(&idx)
                });
                if !continued && tip_oid != oid {
                    *tip = None;
                }
            }
        }
    }

    commits
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let lane = lane_of.get(&c.oid).copied().unwrap_or(0);
            let continues_down = i + 1 < commits.len()
                && lane_of
                    .get(commits[i + 1].oid.as_str())
                    .map(|&l| l == lane)
                    .unwrap_or(false);
            let continues_up = i > 0
                && lane_of
                    .get(commits[i - 1].oid.as_str())
                    .map(|&l| l == lane)
                    .unwrap_or(false);
            GitGraphRow {
                oid: c.oid.clone(),
                lane,
                lane_color_index: lane % 6,
                continues_up,
                continues_down,
                merge_from_lane: merge_from.get(&c.oid).copied(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(oid: &str, parents: &[&str]) -> GitCommitNode {
        GitCommitNode {
            oid: oid.into(),
            parents: parents.iter().map(|s| (*s).to_string()).collect(),
            subject: format!("commit {oid}"),
            author: "test".into(),
            rel_time: "1 day ago".into(),
            decorations: Vec::new(),
        }
    }

    #[test]
    fn lane_layout_linear_chain() {
        let commits = vec![node("c3", &["c2"]), node("c2", &["c1"]), node("c1", &[])];
        let rows = compute_lane_layout(&commits);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].lane, rows[1].lane);
        assert_eq!(rows[1].lane, rows[2].lane);
    }

    #[test]
    fn lane_layout_merge_gets_second_parent_lane() {
        let commits = vec![
            node("m", &["b", "a"]),
            node("b", &["r"]),
            node("a", &["r"]),
            node("r", &[]),
        ];
        let rows = compute_lane_layout(&commits);
        assert_eq!(rows.len(), 4);
        assert!(rows[0].merge_from_lane.is_some());
    }

    #[test]
    fn parse_decorations_branch() {
        let d = parse_decorations("HEAD -> main, origin/main");
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].label, "main");
    }
}
