//! Web search/fetch tools (Tavily / Brave).

use crate::agent::tools::{ToolOutcome, WorkspaceRootGuard};
use crate::agent::web_settings::{self, WebProviderKind};
use serde_json::Value;

pub fn web_tools_enabled() -> bool {
    web_settings::web_tools_enabled()
}

pub fn tool_web_search(args: &Value, _root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => {
            return ToolOutcome {
                ok: false,
                content: "missing query".into(),
            };
        }
    };
    let Some((provider, key)) = web_settings::resolve_active_key() else {
        return ToolOutcome {
            ok: false,
            content: "web tools disabled: configure API keys in Settings → Agent → Web Tools".into(),
        };
    };
    match provider {
        WebProviderKind::Tavily => tavily_search(query, &key),
        WebProviderKind::Brave => ToolOutcome {
            ok: false,
            content: "Brave search not implemented in v1; select Tavily in Web Tools settings".into(),
        },
        WebProviderKind::None => ToolOutcome {
            ok: false,
            content: "no web provider selected".into(),
        },
    }
}

fn tavily_search(query: &str, api_key: &str) -> ToolOutcome {
    let body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "max_results": 5,
    });
    let result: Result<(bool, String), String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let resp = reqwest::Client::new()
                .post("https://api.tavily.com/search")
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let ok = resp.status().is_success();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            Ok((ok, text))
        })
    });
    match result {
        Ok((ok, text)) => ToolOutcome { ok, content: text },
        Err(e) => ToolOutcome {
            ok: false,
            content: format!("web_search failed: {e}"),
        },
    }
}

pub fn tool_web_fetch(args: &Value, _root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let _key = match web_settings::resolve_active_key() {
        Some((_, k)) => k,
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
