//! Web search/fetch tools (Tavily / Brave).

use crate::agent::tools::{ToolOutcome, WorkspaceRootGuard};
use serde_json::Value;

pub fn web_tools_enabled() -> bool {
    web_api_key().is_some()
}

pub fn web_api_key() -> Option<String> {
    std::env::var("BLX_TAVILY_API_KEY")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("BLX_BRAVE_API_KEY")
                .ok()
                .filter(|s| !s.is_empty())
        })
}

pub fn tool_web_search(args: &Value, _root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(key) = web_api_key() else {
        return ToolOutcome {
            ok: false,
            content: "web tools disabled: set BLX_TAVILY_API_KEY or BLX_BRAVE_API_KEY".into(),
        };
    };
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolOutcome {
                ok: false,
                content: "missing query".into(),
            };
        }
    };
    // Prefer Tavily when its key is set.
    if std::env::var("BLX_TAVILY_API_KEY")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        tavily_search(query, &key)
    } else {
        ToolOutcome {
            ok: false,
            content: "Brave web search not implemented in v1; set BLX_TAVILY_API_KEY".into(),
        }
    }
}

fn tavily_search(query: &str, api_key: &str) -> ToolOutcome {
    let body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "max_results": 5,
    });
    let resp = std::process::Command::new("curl")
        .args([
            "-sS",
            "-X",
            "POST",
            "https://api.tavily.com/search",
            "-H",
            "Content-Type: application/json",
            "-d",
            &body.to_string(),
        ])
        .output();
    match resp {
        Ok(o) if o.status.success() => ToolOutcome {
            ok: true,
            content: String::from_utf8_lossy(&o.stdout).into_owned(),
        },
        Ok(o) => ToolOutcome {
            ok: false,
            content: String::from_utf8_lossy(&o.stderr).into_owned(),
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: format!("web_search failed: {e}"),
        },
    }
}

pub fn tool_web_fetch(args: &Value, _root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let _key = match web_api_key() {
        Some(k) => k,
        None => {
            return ToolOutcome {
                ok: false,
                content: "web tools disabled".into(),
            };
        }
    };
    let url = match args.get("url").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return ToolOutcome {
                ok: false,
                content: "missing url".into(),
            };
        }
    };
    ToolOutcome {
        ok: false,
        content: format!("web_fetch not implemented for {url} in v1"),
    }
}
