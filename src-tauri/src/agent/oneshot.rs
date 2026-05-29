//! Minimal, non-streaming text completion against the configured agent
//! provider. Unlike [`crate::agent::session_orchestrator`], this does not
//! touch the persisted conversation, emit `AgentEvent`s, or run tools — it
//! issues a single request and returns the assistant's text.
//!
//! Used for one-off utility generations (e.g. AI commit messages) that want
//! to reuse the user's provider/model/key without entering the chat loop.

use crate::agent::openrouter::Endpoint;
use crate::agent_settings::{AgentProviderKind, AgentProviderSettings};
use serde_json::{json, Value};
use std::time::Duration;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Run one completion and return the assistant text. `system` is the system
/// prompt; `user` is the single user message.
pub async fn complete_text(
    settings: &AgentProviderSettings,
    api_key: &str,
    system: &str,
    user: &str,
    max_tokens: u64,
) -> Result<String, String> {
    if settings.model_id.trim().is_empty() {
        return Err("no model configured".into());
    }
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    match settings.provider {
        AgentProviderKind::Anthropic => {
            anthropic_complete(&client, api_key, &settings.model_id, system, user, max_tokens).await
        }
        AgentProviderKind::Openrouter | AgentProviderKind::Openai => {
            let endpoint = Endpoint::from_provider(settings.provider)
                .expect("openrouter/openai endpoint mapping");
            openai_complete(
                &client,
                endpoint,
                api_key,
                &settings.model_id,
                system,
                user,
                max_tokens,
            )
            .await
        }
    }
}

async fn anthropic_complete(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    max_tokens: u64,
) -> Result<String, String> {
    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system,
        "messages": [{ "role": "user", "content": user }],
    });
    let resp = client
        .post(ANTHROPIC_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("provider error {status}: {}", truncate(&text)));
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("bad response: {e}"))?;
    // content: [{ "type": "text", "text": "…" }, …]
    let out = value
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    Ok(out)
}

async fn openai_complete(
    client: &reqwest::Client,
    endpoint: Endpoint,
    api_key: &str,
    model: &str,
    system: &str,
    user: &str,
    max_tokens: u64,
) -> Result<String, String> {
    let body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
    });
    let mut req = client
        .post(endpoint.url())
        .bearer_auth(api_key)
        .header("Content-Type", "application/json");
    if matches!(endpoint, Endpoint::Openrouter) {
        req = req
            .header("HTTP-Referer", "https://bitslix.com/blxcode")
            .header("X-Title", "blxcode");
    }
    let resp = req
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("provider error {status}: {}", truncate(&text)));
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("bad response: {e}"))?;
    let out = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(out)
}

fn truncate(s: &str) -> String {
    const MAX: usize = 300;
    if s.len() <= MAX {
        s.to_string()
    } else {
        format!("{}…", &s[..MAX])
    }
}
