//! Git tools scoped to the workspace root.

use crate::agent::environment;
use crate::agent::tools::{ToolOutcome, WorkspaceRootGuard};
use crate::proc::command;
use serde_json::{json, Value};
use std::path::Path;

const MAX_GIT_OUTPUT: usize = 64 * 1024;

fn run_git(root: &WorkspaceRootGuard, args: &[&str]) -> ToolOutcome {
    let output = command("git")
        .args(args)
        .current_dir(root.as_str())
        .output();
    match output {
        Ok(o) => {
            let mut body = String::from_utf8_lossy(&o.stdout).into_owned();
            if !o.stderr.is_empty() {
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(&String::from_utf8_lossy(&o.stderr));
            }
            if body.len() > MAX_GIT_OUTPUT {
                body.truncate(MAX_GIT_OUTPUT);
                body.push_str("\n… (truncated)");
            }
            ToolOutcome {
                ok: o.status.success(),
                content: body,
            }
        }
        Err(e) => ToolOutcome {
            ok: false,
            content: format!("git failed: {e}"),
        },
    }
}

fn require_env(root: &WorkspaceRootGuard) -> Result<(), ToolOutcome> {
    environment::require_environment(&root.as_str())
}

fn optional_path<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn resolve_cwd(
    root: &WorkspaceRootGuard,
    rel: Option<&str>,
) -> Result<std::path::PathBuf, ToolOutcome> {
    let base = root.as_str();
    let base_path = Path::new(&base);
    match rel {
        None | Some("") | Some(".") => Ok(base_path.to_path_buf()),
        Some(p) => {
            if p.contains("..") {
                return Err(ToolOutcome {
                    ok: false,
                    content: "path escapes workspace".into(),
                });
            }
            let full = base_path.join(p.trim_start_matches('/'));
            if !full.starts_with(base_path) {
                return Err(ToolOutcome {
                    ok: false,
                    content: "path escapes workspace".into(),
                });
            }
            Ok(full)
        }
    }
}

pub fn tool_git_status(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let cwd = match resolve_cwd(root, optional_path(args, "cwd")) {
        Ok(p) => p,
        Err(o) => return o,
    };
    let output = command("git")
        .args(["status", "--short", "--branch"])
        .current_dir(&cwd)
        .output();
    match output {
        Ok(o) => ToolOutcome {
            ok: o.status.success(),
            content: String::from_utf8_lossy(&o.stdout).into_owned(),
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: format!("git status: {e}"),
        },
    }
}

pub fn tool_git_diff(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let cwd = match resolve_cwd(root, optional_path(args, "cwd")) {
        Ok(p) => p,
        Err(o) => return o,
    };
    let mut cmd_args = vec!["diff"];
    if args.get("staged").and_then(|v| v.as_bool()) == Some(true) {
        cmd_args.push("--staged");
    }
    let output = command("git").args(&cmd_args).current_dir(&cwd).output();
    match output {
        Ok(o) => ToolOutcome {
            ok: true,
            content: String::from_utf8_lossy(&o.stdout).into_owned(),
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: format!("git diff: {e}"),
        },
    }
}

pub fn tool_git_log(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(20)
        .min(100);
    run_git(root, &["log", "--oneline", &format!("-{limit}")])
}

pub fn tool_git_show(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let rev = match args.get("rev").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolOutcome {
                ok: false,
                content: "missing rev".into(),
            };
        }
    };
    run_git(root, &["show", rev])
}

pub fn tool_git_branch_info(root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    run_git(root, &["branch", "-vv"])
}

pub fn tool_git_ls_files(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let mut cmd_args = vec!["ls-files"];
    if let Some(p) = optional_path(args, "path") {
        cmd_args.push(p);
    }
    run_git(root, &cmd_args)
}

pub fn tool_git_conflicts(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let cwd = match resolve_cwd(root, optional_path(args, "cwd")) {
        Ok(p) => p,
        Err(o) => return o,
    };
    let max_files = args
        .get("maxFiles")
        .and_then(|v| v.as_u64())
        .unwrap_or(20)
        .clamp(1, 100) as usize;
    let max_hunks = args
        .get("maxHunksPerFile")
        .and_then(|v| v.as_u64())
        .unwrap_or(6)
        .clamp(1, 20) as usize;

    let status = command("git")
        .args(["status", "--short", "--branch"])
        .current_dir(&cwd)
        .output();
    let status_text = match status {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: format!("git status: {e}"),
            };
        }
    };

    let names = command("git")
        .args(["diff", "--name-only", "--diff-filter=U", "-z"])
        .current_dir(&cwd)
        .output();
    let names = match names {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => {
            return ToolOutcome {
                ok: false,
                content: String::from_utf8_lossy(&o.stderr).into_owned(),
            };
        }
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: format!("git diff --name-only: {e}"),
            };
        }
    };
    let paths: Vec<String> = names
        .split(|b| *b == 0)
        .filter(|chunk| !chunk.is_empty())
        .filter_map(|chunk| String::from_utf8(chunk.to_vec()).ok())
        .take(max_files)
        .collect();

    let stages = command("git")
        .args(["ls-files", "-u"])
        .current_dir(&cwd)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    let mut files = Vec::new();
    for path in &paths {
        let full = cwd.join(path);
        if !full.starts_with(&cwd) {
            continue;
        }
        let hunks = std::fs::read_to_string(&full)
            .ok()
            .map(|content| extract_conflict_hunks(&content, max_hunks))
            .unwrap_or_default();
        files.push(json!({
            "path": path,
            "hunks": hunks,
        }));
    }

    ToolOutcome {
        ok: true,
        content: json!({
            "hasConflicts": !paths.is_empty(),
            "status": status_text,
            "unmergedPaths": paths,
            "indexStages": stages,
            "files": files,
            "agentInstruction": "Do not resolve conflicts silently. Present the user with concise resolution options and wait for their choice before editing, staging, committing, merging, rebasing, or aborting."
        })
        .to_string(),
    }
}

fn extract_conflict_hunks(content: &str, max_hunks: usize) -> Vec<Value> {
    #[derive(Clone, Copy)]
    enum Section {
        Ours,
        Base,
        Theirs,
    }

    fn trim_section(lines: &[String]) -> String {
        const MAX_SECTION_BYTES: usize = 4 * 1024;
        let mut s = lines.join("\n");
        if s.len() > MAX_SECTION_BYTES {
            s.truncate(MAX_SECTION_BYTES);
            s.push_str("\n… (truncated)");
        }
        s
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() && out.len() < max_hunks {
        if !lines[i].starts_with("<<<<<<<") {
            i += 1;
            continue;
        }

        let start_line = i + 1;
        let ours_label = lines[i].trim().to_owned();
        let mut base_label = String::new();
        let mut theirs_label = String::new();
        let mut section = Section::Ours;
        let mut ours = Vec::new();
        let mut base = Vec::new();
        let mut theirs = Vec::new();
        let mut raw = vec![lines[i].to_owned()];
        i += 1;

        while i < lines.len() {
            let line = lines[i];
            raw.push(line.to_owned());
            if line.starts_with("|||||||") {
                base_label = line.trim().to_owned();
                section = Section::Base;
            } else if line.starts_with("=======") {
                section = Section::Theirs;
            } else if line.starts_with(">>>>>>>") {
                theirs_label = line.trim().to_owned();
                i += 1;
                break;
            } else {
                match section {
                    Section::Ours => ours.push(line.to_owned()),
                    Section::Base => base.push(line.to_owned()),
                    Section::Theirs => theirs.push(line.to_owned()),
                }
            }
            i += 1;
        }

        out.push(json!({
            "startLine": start_line,
            "oursLabel": ours_label,
            "baseLabel": base_label,
            "theirsLabel": theirs_label,
            "ours": trim_section(&ours),
            "base": trim_section(&base),
            "theirs": trim_section(&theirs),
            "raw": trim_section(&raw),
        }));
    }
    out
}

pub fn tool_workspace_git_status(root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    tool_git_status(&Value::Null, root)
}

pub fn tool_workspace_diff(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let mut a = serde_json::json!({ "staged": false });
    if let Some(staged) = args.get("staged") {
        a["staged"] = staged.clone();
    }
    tool_git_diff(&a, root)
}

pub fn tool_git_apply_patch(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let patch = match args.get("patch").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolOutcome {
                ok: false,
                content: "missing patch".into(),
            };
        }
    };
    use std::io::Write;
    use std::process::Stdio;
    let mut child = match command("git")
        .args(["apply", "--whitespace=nowarn"])
        .current_dir(root.as_str())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: format!("git apply: {e}"),
            };
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(patch.as_bytes());
    }
    match child.wait_with_output() {
        Ok(o) => ToolOutcome {
            ok: o.status.success(),
            content: String::from_utf8_lossy(&o.stderr).into_owned(),
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: format!("git apply: {e}"),
        },
    }
}

pub fn tool_git_add(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let paths: Vec<&str> = args
        .get("paths")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_else(|| vec!["."]);
    let mut cmd_args = vec!["add"];
    cmd_args.extend(paths);
    run_git(root, &cmd_args)
}

pub fn tool_git_commit(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    if let Err(o) = require_env(root) {
        return o;
    }
    let message = match args.get("message").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolOutcome {
                ok: false,
                content: "missing message".into(),
            };
        }
    };
    run_git(root, &["commit", "-m", message])
}
