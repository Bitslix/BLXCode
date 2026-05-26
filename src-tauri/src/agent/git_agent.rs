//! Git tools scoped to the workspace root.

use crate::agent::environment;
use crate::agent::tools::{ToolOutcome, WorkspaceRootGuard};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const MAX_GIT_OUTPUT: usize = 64 * 1024;

fn run_git(root: &WorkspaceRootGuard, args: &[&str]) -> ToolOutcome {
    let output = Command::new("git")
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
    let output = Command::new("git")
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
    let output = Command::new("git")
        .args(&cmd_args)
        .current_dir(&cwd)
        .output();
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
    let mut child = match Command::new("git")
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
