//! Commit graph for the sidebar via native `git log --graph` (no custom lane layout).

use crate::git_info::{find_git_dir, git_cli_available};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

pub const GIT_MISSING_CODE: &str = "git_missing";
const DEFAULT_LIMIT: u32 = 100;
/// Matches `--pretty=format:…%x02` record terminator.
const RECORD_END: char = '\x02';
/// Marks start of structured commit fields on a graph line.
const RECORD_START: char = '\x1e';
const FIELD_SEP: char = '\x1f';
/// Fixed graph column width (`git log -c log.graphWidth`).
const GRAPH_WIDTH: u32 = 14;

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
pub struct GitGraphEntry {
    /// Left gutter from `git log --graph` (may span multiple lines for merges).
    pub gutter: String,
    pub commit: GitCommitNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphLayout {
    pub entries: Vec<GitGraphEntry>,
    /// Max gutter line width (for monospace alignment).
    pub gutter_cols: usize,
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
    fetch_graph_entries(work_tree, limit)
}

fn fetch_graph_entries(work_tree: &Path, limit: u32) -> Result<GitGraphLayout, String> {
    let pretty = format!(
        "%x1e%H{FIELD_SEP}%P{FIELD_SEP}%s{FIELD_SEP}%an{FIELD_SEP}%ar{FIELD_SEP}%D%x02"
    );
    let out = Command::new("git")
        .arg("-C")
        .arg(work_tree)
        .arg("-c")
        .arg(format!("log.graphWidth={GRAPH_WIDTH}"))
        .args([
            "log",
            "--graph",
            "--topo-order",
            &format!("-n{limit}"),
            &format!("--pretty=format:{pretty}"),
        ])
        .output()
        .map_err(|e| format!("git log failed: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git log: {stderr}"));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut entries = Vec::new();
    let mut gutter_pending = String::new();

    for raw in text.split(RECORD_END).filter(|s| !s.is_empty()) {
        for line in raw.lines() {
            if let Some((gutter_line, record)) = split_graph_line(line) {
                let mut gutter = gutter_pending.clone();
                gutter.push_str(gutter_line);
                gutter_pending.clear();
                if let Some(commit) = parse_commit_record(record) {
                    entries.push(GitGraphEntry {
                        gutter: normalize_gutter(&gutter),
                        commit,
                    });
                }
            } else if !line.trim().is_empty() {
                gutter_pending.push_str(line);
                gutter_pending.push('\n');
            }
        }
    }

    let gutter_cols = entries
        .iter()
        .flat_map(|e| e.gutter.lines().map(|l| l.chars().count()))
        .max()
        .unwrap_or(2)
        .max(2);
    for entry in &mut entries {
        entry.gutter = pad_gutter_lines(&entry.gutter, gutter_cols);
    }

    Ok(GitGraphLayout {
        entries,
        gutter_cols,
    })
}

fn split_graph_line(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(RECORD_START)?;
    let gutter = line[..idx].trim_end();
    let record = line[idx + 1..].trim_start();
    if record.is_empty() {
        return None;
    }
    Some((gutter, record))
}

fn parse_commit_record(record: &str) -> Option<GitCommitNode> {
    let parts: Vec<&str> = record.split(FIELD_SEP).collect();
    if parts.len() < 5 {
        return None;
    }
    let oid = parts[0].trim().to_string();
    if oid.is_empty() {
        return None;
    }
    let parents: Vec<String> = parts[1]
        .split_whitespace()
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    Some(GitCommitNode {
        oid,
        parents,
        subject: parts[2].trim().to_string(),
        author: parts[3].trim().to_string(),
        rel_time: parts[4].trim().to_string(),
        decorations: parse_decorations(parts.get(5).copied().unwrap_or("").trim()),
    })
}

fn normalize_gutter(gutter: &str) -> String {
    let lines: Vec<&str> = gutter.lines().collect();
    if lines.is_empty() {
        return "* ".to_string();
    }
    lines.join("\n")
}

fn pad_gutter_lines(gutter: &str, cols: usize) -> String {
    gutter
        .lines()
        .map(|line| {
            let n = line.chars().count();
            if n >= cols {
                line.to_string()
            } else {
                format!("{line}{}", " ".repeat(cols - n))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

    #[test]
    fn split_graph_line_finds_record() {
        let (g, r) = split_graph_line("* \x1eabc\x1fdef").expect("split");
        assert_eq!(g, "*");
        assert!(r.starts_with("abc"));
    }

    #[test]
    fn parse_commit_record_fields() {
        let rec = format!("deadbeef{FIELD_SEP}{FIELD_SEP}subject{FIELD_SEP}author{FIELD_SEP}1 day ago{FIELD_SEP}");
        let c = parse_commit_record(&rec).expect("parse");
        assert_eq!(c.oid, "deadbeef");
        assert_eq!(c.subject, "subject");
        assert_eq!(c.author, "author");
    }

    #[test]
    fn parse_decorations_branch() {
        let d = parse_decorations("HEAD -> main, origin/main");
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].label, "main");
    }

    #[test]
    fn gutter_buffer_accumulates_connector_lines() {
        let sample = format!(
            "*   \x1em1{FIELD_SEP}{FIELD_SEP}merge{FIELD_SEP}a{FIELD_SEP}now{FIELD_SEP}{RECORD_END}\
             |\\  {RECORD_END}\
             | * \x1ef1{FIELD_SEP}{FIELD_SEP}feat{FIELD_SEP}a{FIELD_SEP}now{FIELD_SEP}{RECORD_END}\
             |/  {RECORD_END}\
             *   \x1em2{FIELD_SEP}{FIELD_SEP}main{FIELD_SEP}a{FIELD_SEP}now{FIELD_SEP}{RECORD_END}"
        );
        let mut entries = Vec::new();
        let mut gutter_pending = String::new();
        for raw in sample.split(RECORD_END).filter(|s| !s.is_empty()) {
            for line in raw.lines() {
                if let Some((gutter_line, record)) = split_graph_line(line) {
                    let mut gutter = gutter_pending.clone();
                    gutter.push_str(gutter_line);
                    gutter_pending.clear();
                    if let Some(commit) = parse_commit_record(record) {
                        entries.push(GitGraphEntry {
                            gutter: normalize_gutter(&gutter),
                            commit,
                        });
                    }
                } else if !line.trim().is_empty() {
                    gutter_pending.push_str(line);
                    gutter_pending.push('\n');
                }
            }
        }
        assert_eq!(entries.len(), 3);
        assert!(entries[1].gutter.contains('|'));
        assert!(entries[1].gutter.contains('*'));
    }
}
