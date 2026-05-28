//! Generic "agent pointer" block management.
//!
//! Several blxcode subsystems (workspace memory, project rules) want to
//! advertise their on-disk location to **external** coding agents
//! (Claude Code, Codex, Gemini, Cursor, OpenCode) by writing a marked
//! block into the agent's well-known config file (`CLAUDE.md`,
//! `AGENTS.md`, `GEMINI.md`, `.cursorrules`). Each subsystem owns its
//! own marker pair so multiple blocks can coexist in the same file.
//!
//! This module owns the *generic* machinery: the agent → filename
//! mapping, the splice/strip helpers, and the install/uninstall/status
//! drivers. Callers supply their marker pair and a body builder.
//!
//! Marker convention: HTML comment for Markdown agents
//! (`<!-- blxcode-<topic>:begin/end -->`), Markdown header for the
//! plain-text `.cursorrules` file (`# blxcode-<topic>:begin/end`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Wire-format result of an install / uninstall / status query for a
/// single agent.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PointerResult {
    pub agent: String,
    pub path: String,
    pub installed: bool,
    pub note: Option<String>,
}

/// Map a frontend-supplied agent id to the per-agent target filename.
/// Returns `None` for unknown ids.
pub fn pointer_filename(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("CLAUDE.md"),
        "codex" => Some("AGENTS.md"),
        "gemini" => Some("GEMINI.md"),
        "cursor" => Some(".cursorrules"),
        "opencode" => Some("AGENTS.md"),
        _ => None,
    }
}

/// `.cursorrules` is plain Markdown without HTML-comment support, so it
/// uses heading-style markers instead.
pub fn pointer_cursor_style(agent: &str) -> bool {
    agent == "cursor"
}

/// All agent ids the status driver iterates over. Stable for UI tests.
pub const ALL_AGENTS: &[&str] = &["claude", "codex", "gemini", "cursor", "opencode"];

/// Replace or append the begin/end block in `existing`. Idempotent.
pub fn splice_block(existing: &str, begin: &str, end: &str, new_body: &str) -> String {
    let block = format!("{begin}\n{new_body}{end}\n");
    if let (Some(bi), Some(ei)) = (existing.find(begin), existing.find(end)) {
        if ei > bi {
            let ei_end = ei + end.len();
            let tail_start = if ei_end < existing.len() && existing.as_bytes()[ei_end] == b'\n' {
                ei_end + 1
            } else {
                ei_end
            };
            let mut out = String::with_capacity(bi + block.len() + existing.len() - tail_start);
            out.push_str(&existing[..bi]);
            out.push_str(&block);
            out.push_str(&existing[tail_start..]);
            return out;
        }
    }
    if existing.trim().is_empty() {
        block
    } else {
        format!("{}\n\n{block}", existing.trim_end())
    }
}

/// Remove the begin..end block; everything else stays untouched.
pub fn strip_block(existing: &str, begin: &str, end: &str) -> String {
    if let (Some(bi), Some(ei)) = (existing.find(begin), existing.find(end)) {
        if ei <= bi {
            return existing.to_owned();
        }
        let ei_end = ei + end.len();
        let tail_start = if ei_end < existing.len() && existing.as_bytes()[ei_end] == b'\n' {
            ei_end + 1
        } else {
            ei_end
        };
        let mut out = String::new();
        out.push_str(&existing[..bi]);
        out.push_str(&existing[tail_start..]);
        return out.trim_end().to_owned();
    }
    existing.to_owned()
}

pub fn block_installed(content: &str, begin: &str, end: &str) -> bool {
    content.contains(begin) && content.contains(end)
}

/// Marker pair for HTML-comment / Markdown-heading variants.
pub struct Markers<'a> {
    pub html: (&'a str, &'a str),
    pub cursor: (&'a str, &'a str),
}

fn pick_markers<'a>(markers: &'a Markers<'_>, cursor_style: bool) -> (&'a str, &'a str) {
    if cursor_style {
        markers.cursor
    } else {
        markers.html
    }
}

/// Install (or refresh) the block in every requested agent file. The
/// caller passes a `build_body(workspace_path, cursor_style)` closure
/// that produces the block's payload — generic helper has no opinion on
/// what goes in there.
///
/// Target files **must already exist**: blxcode never creates an agent
/// config from scratch, to avoid surprising the user.
pub fn install_pointer_block<F>(
    workspace_cwd: &str,
    agents: Vec<String>,
    markers: &Markers<'_>,
    build_body: F,
) -> Result<Vec<PointerResult>, String>
where
    F: Fn(&Path, bool) -> String,
{
    let ws = validate_ws(workspace_cwd)?;
    let mut results: Vec<PointerResult> = Vec::new();
    let mut written: BTreeSet<String> = Default::default();
    for agent in agents {
        let Some(fname) = pointer_filename(&agent) else {
            results.push(PointerResult {
                agent,
                path: String::new(),
                installed: false,
                note: Some("unknown agent".into()),
            });
            continue;
        };
        let path = ws.join(fname);
        if written.contains(fname) {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some("shared file already written".into()),
            });
            continue;
        }
        written.insert(fname.to_owned());
        if !path.exists() {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some("file missing — create it first".into()),
            });
            continue;
        }
        let cursor_style = pointer_cursor_style(&agent);
        let body = build_body(&ws, cursor_style);
        let (begin, end) = pick_markers(markers, cursor_style);
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let updated = splice_block(&existing, begin, end, &body);
        match fs::write(&path, updated.as_bytes()) {
            Ok(()) => results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: true,
                note: None,
            }),
            Err(e) => results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some(format!("write failed: {e}")),
            }),
        }
    }
    Ok(results)
}

/// Remove the block for every requested agent. If stripping leaves the
/// file empty (only whitespace), the file itself is deleted so a fresh
/// memory/rules install can later refuse with "file missing — create it
/// first" rather than silently resurrecting a blxcode-owned file.
pub fn uninstall_pointer_block(
    workspace_cwd: &str,
    agents: Vec<String>,
    markers: &Markers<'_>,
) -> Result<Vec<PointerResult>, String> {
    let ws = validate_ws(workspace_cwd)?;
    let mut results = Vec::new();
    let mut handled: BTreeSet<String> = Default::default();
    for agent in agents {
        let Some(fname) = pointer_filename(&agent) else {
            results.push(PointerResult {
                agent,
                path: String::new(),
                installed: false,
                note: Some("unknown agent".into()),
            });
            continue;
        };
        if handled.contains(fname) {
            results.push(PointerResult {
                agent,
                path: ws.join(fname).to_string_lossy().into_owned(),
                installed: false,
                note: Some("shared file already cleaned".into()),
            });
            continue;
        }
        let path = ws.join(fname);
        handled.insert(fname.to_owned());
        if !path.exists() {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some("no file".into()),
            });
            continue;
        }
        let cursor_style = pointer_cursor_style(&agent);
        let (begin, end) = pick_markers(markers, cursor_style);
        let existing = fs::read_to_string(&path).unwrap_or_default();
        let stripped = strip_block(&existing, begin, end);
        if stripped.trim().is_empty() {
            if let Err(e) = fs::remove_file(&path) {
                results.push(PointerResult {
                    agent,
                    path: path.to_string_lossy().into_owned(),
                    installed: false,
                    note: Some(format!("remove failed: {e}")),
                });
                continue;
            }
        } else if let Err(e) = fs::write(&path, format!("{stripped}\n").as_bytes()) {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some(format!("write failed: {e}")),
            });
            continue;
        }
        results.push(PointerResult {
            agent,
            path: path.to_string_lossy().into_owned(),
            installed: false,
            note: None,
        });
    }
    Ok(results)
}

/// Per-agent installation status for the given marker pair. Iterates
/// over `ALL_AGENTS` so the UI sees every brand even when nothing is
/// installed.
pub fn pointer_status(
    workspace_cwd: &str,
    markers: &Markers<'_>,
) -> Result<Vec<PointerResult>, String> {
    let ws = validate_ws(workspace_cwd)?;
    let mut out = Vec::new();
    let mut handled: BTreeSet<String> = Default::default();
    for a in ALL_AGENTS {
        let Some(fname) = pointer_filename(a) else {
            continue;
        };
        let path = ws.join(fname);
        if handled.contains(fname) {
            out.push(PointerResult {
                agent: (*a).to_owned(),
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some("shared file already handled".into()),
            });
            continue;
        }
        handled.insert(fname.to_owned());
        if !path.exists() {
            out.push(PointerResult {
                agent: (*a).to_owned(),
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some("file missing".into()),
            });
            continue;
        }
        let body = fs::read_to_string(&path).unwrap_or_default();
        let cursor_style = pointer_cursor_style(a);
        let (begin, end) = pick_markers(markers, cursor_style);
        out.push(PointerResult {
            agent: (*a).to_owned(),
            path: path.to_string_lossy().into_owned(),
            installed: block_installed(&body, begin, end),
            note: None,
        });
    }
    Ok(out)
}

fn validate_ws(workspace_cwd: &str) -> Result<PathBuf, String> {
    crate::agents_layout::validate_workspace_cwd(workspace_cwd)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEGIN: &str = "<!-- blxcode-test:begin -->";
    const END: &str = "<!-- blxcode-test:end -->";

    #[test]
    fn splice_appends_when_marker_absent() {
        let updated = splice_block("hello\n", BEGIN, END, "body\n");
        assert_eq!(updated, format!("hello\n\n{BEGIN}\nbody\n{END}\n"));
    }

    #[test]
    fn splice_replaces_existing_block() {
        let existing = format!("head\n{BEGIN}\nold\n{END}\ntail");
        let updated = splice_block(&existing, BEGIN, END, "new\n");
        assert!(updated.contains("new"));
        assert!(!updated.contains("old"));
        assert!(updated.contains("tail"));
    }

    #[test]
    fn strip_leaves_surrounding_text() {
        let existing = format!("A\n\n{BEGIN}\nbody\n{END}\n\nB\n");
        let stripped = strip_block(&existing, BEGIN, END);
        assert_eq!(stripped, "A\n\n\nB");
    }

    #[test]
    fn block_installed_requires_both_markers() {
        assert!(block_installed(&format!("{BEGIN}\nx\n{END}"), BEGIN, END));
        assert!(!block_installed("{BEGIN}\nx\n", BEGIN, END));
    }

    #[test]
    fn two_different_marker_pairs_coexist() {
        let other_begin = "<!-- blxcode-other:begin -->";
        let other_end = "<!-- blxcode-other:end -->";
        let mut content = splice_block("", BEGIN, END, "first\n");
        content = splice_block(&content, other_begin, other_end, "second\n");
        assert!(block_installed(&content, BEGIN, END));
        assert!(block_installed(&content, other_begin, other_end));
        // Strip one — the other survives.
        let after = strip_block(&content, BEGIN, END);
        assert!(!block_installed(&after, BEGIN, END));
        assert!(block_installed(&after, other_begin, other_end));
    }
}
