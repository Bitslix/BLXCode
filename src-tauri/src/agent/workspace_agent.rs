//! Workspace search and scoped git helpers.

use crate::agent::tools::{ToolOutcome, WorkspaceRootGuard};
use serde_json::Value;
use std::process::Command;

const MAX_SEARCH_HITS: usize = 50;

pub fn tool_workspace_search(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => {
            return ToolOutcome {
                ok: false,
                content: "missing query".into(),
            };
        }
    };
    let max = args
        .get("maxResults")
        .and_then(|v| v.as_u64())
        .unwrap_or(MAX_SEARCH_HITS as u64)
        .min(100) as usize;

    let output = Command::new("rg")
        .args([
            "--line-number",
            "--max-count",
            &max.to_string(),
            "--no-heading",
            query,
        ])
        .current_dir(root.as_str())
        .output();

    match output {
        Ok(o) => {
            let body = String::from_utf8_lossy(&o.stdout).into_owned();
            ToolOutcome {
                ok: o.status.success() || o.status.code() == Some(1),
                content: if body.is_empty() {
                    "no matches".into()
                } else {
                    body
                },
            }
        }
        Err(_) => ToolOutcome {
            ok: false,
            content: "rg not available; install ripgrep for workspace_search".into(),
        },
    }
}
