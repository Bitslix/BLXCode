//! Conversation compaction.
//!
//! Summarizes the running session into a compact briefing and replaces the
//! persisted history with a tiny synthetic `user`/`assistant` pair carrying
//! that summary. This frees context-window budget while keeping continuity —
//! the next real turn resumes from the briefing. Triggered manually from the
//! chat header's **Compact** button or automatically when occupancy nears the
//! configured threshold.
//!
//! A single **non-tool** provider call does the summarization, so it cannot
//! enter a tool loop. The conversation is flattened to a plain-text transcript
//! first, which keeps this provider-agnostic (OpenAI-style and Anthropic-style
//! message shapes both reduce to readable text).

use crate::agent::state::AgentEngineState;
use crate::agent_settings::{load_settings_pub, provider_key_pub, AgentProviderKind};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, State};

const SUMMARY_MAX_TOKENS: u64 = 2048;
const TOOL_RESULT_CLAMP: usize = 600;

const SUMMARY_SYSTEM_PROMPT: &str = "You are compacting a coding-assistant conversation to free context-window space. \
Produce a dense, factual briefing that lets the assistant continue seamlessly. \
Preserve: the user's goals and open requests, decisions already made, files and paths touched, \
commands run and their outcomes, current task/plan state, unresolved questions, and any constraints or preferences stated. \
Drop greetings, filler, and redundant restatements. Use compact bullet points grouped by topic. \
Do not invent facts. Keep it as short as faithfully possible.";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionResult {
    /// The generated briefing text.
    pub summary: String,
    /// Tokens occupied before compaction (the meter's live value when the
    /// caller supplied it, otherwise a transcript estimate).
    pub before_tokens: u64,
    /// Rough token estimate of the post-compaction conversation.
    pub after_tokens_estimate: u64,
    /// Number of conversation messages that were folded into the summary.
    pub messages_before: usize,
}

/// Summarize + replace the persisted conversation. `current_tokens` is the
/// frontend meter's live occupancy, used for an accurate `before_tokens`.
#[tauri::command]
pub async fn agent_compact_conversation(
    app: AppHandle,
    current_tokens: Option<u64>,
    agent: State<'_, Arc<AgentEngineState>>,
) -> Result<CompactionResult, String> {
    if agent.busy() {
        return Err("Agent ist noch beschäftigt. Bitte zuerst abbrechen oder warten.".into());
    }
    let convo = agent.conversation_snapshot();
    // Need at least one exchange to be worth compacting.
    if convo.len() < 2 {
        return Err("nothing-to-compact".into());
    }
    let agent: Arc<AgentEngineState> = agent.inner().clone();

    let settings = load_settings_pub(&app)?;
    let api_key = provider_key_pub(&app, settings.provider)?;
    if api_key.trim().is_empty() {
        return Err(format!(
            "Kein API-Key für {} hinterlegt.",
            settings.provider.as_str()
        ));
    }

    let transcript = flatten_conversation(&convo);
    let messages_before = convo.len();
    let before_tokens = current_tokens
        .filter(|&n| n > 0)
        .unwrap_or_else(|| estimate_tokens(&transcript));

    // Block concurrent turns while the summarization call is in flight.
    agent.set_busy(true);
    let outcome = summarize(&settings.model_id, settings.provider, &api_key, &transcript).await;
    let summary = match outcome {
        Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
        Ok(_) => {
            agent.set_busy(false);
            return Err("Die Zusammenfassung war leer.".into());
        }
        Err(e) => {
            agent.set_busy(false);
            return Err(e);
        }
    };

    let after_tokens_estimate = estimate_tokens(&summary).saturating_add(48);
    agent.set_conversation(synthetic_history(&summary));
    agent.set_busy(false);

    Ok(CompactionResult {
        summary,
        before_tokens,
        after_tokens_estimate,
        messages_before,
    })
}

/// Replacement history: a minimal, role-valid `user` → `assistant` pair so
/// the next real turn appends cleanly (valid for both OpenAI and Anthropic,
/// which require the first message to be `user`).
fn synthetic_history(summary: &str) -> Vec<Value> {
    vec![
        json!({
            "role": "user",
            "content": format!(
                "Summary of our earlier conversation, compacted for context. Continue from here:\n\n{summary}"
            ),
        }),
        json!({
            "role": "assistant",
            "content": "Understood — continuing from the compacted summary above.",
        }),
    ]
}

/// Flatten provider-native messages into a readable transcript.
fn flatten_conversation(convo: &[Value]) -> String {
    let mut out = String::new();
    for msg in convo {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("?");
        let label = match role {
            "user" => "User",
            "assistant" => "Assistant",
            "tool" => "Tool result",
            "system" => "System",
            other => other,
        };

        // OpenAI-style assistant tool calls live in a sibling `tool_calls`.
        if let Some(calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
            for call in calls {
                if let Some(name) = call
                    .pointer("/function/name")
                    .and_then(|v| v.as_str())
                {
                    out.push_str(&format!("Assistant tool call: {name}\n"));
                }
            }
        }

        let text = extract_text(msg.get("content"));
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            out.push_str(label);
            out.push_str(": ");
            out.push_str(trimmed);
            out.push_str("\n\n");
        }
    }
    out
}

/// Pull readable text out of a `content` field, handling string content,
/// OpenAI content-part arrays, and Anthropic block arrays (text / tool_use /
/// tool_result).
fn extract_text(content: Option<&Value>) -> String {
    let Some(content) = content else {
        return String::new();
    };
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    let Some(arr) = content.as_array() else {
        return String::new();
    };
    let mut buf = String::new();
    for block in arr {
        let btype = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match btype {
            "text" | "" => {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    buf.push_str(t);
                    buf.push('\n');
                }
            }
            "tool_use" => {
                if let Some(name) = block.get("name").and_then(|v| v.as_str()) {
                    buf.push_str(&format!("[tool call: {name}]\n"));
                }
            }
            "tool_result" => {
                let inner = extract_text(block.get("content"));
                buf.push_str("[tool result: ");
                buf.push_str(&clamp(inner.trim(), TOOL_RESULT_CLAMP));
                buf.push_str("]\n");
            }
            _ => {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    buf.push_str(t);
                    buf.push('\n');
                }
            }
        }
    }
    buf
}

fn clamp(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

/// Rough token estimate (~4 chars/token). Only used for display and the
/// post-compaction meter reset — the next real turn reports exact numbers.
fn estimate_tokens(s: &str) -> u64 {
    (s.chars().count() as u64).div_ceil(4)
}

/// One non-streaming completion that returns the summary text. Dispatches on
/// the active provider's API shape.
async fn summarize(
    model_id: &str,
    provider: AgentProviderKind,
    api_key: &str,
    transcript: &str,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let user_content = format!("Conversation to compact:\n\n{transcript}");

    match provider {
        AgentProviderKind::Anthropic => {
            let body = json!({
                "model": model_id,
                "max_tokens": SUMMARY_MAX_TOKENS,
                "system": SUMMARY_SYSTEM_PROMPT,
                "messages": [{ "role": "user", "content": user_content }],
                "stream": false,
            });
            let res = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("compaction request: {e}"))?;
            let res = res
                .error_for_status()
                .map_err(|e| format!("compaction request: {e}"))?;
            let v: Value = res
                .json()
                .await
                .map_err(|e| format!("compaction parse: {e}"))?;
            // Concatenate all text blocks from `content[]`.
            let summary = v
                .get("content")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            Ok(summary)
        }
        AgentProviderKind::Openrouter | AgentProviderKind::Openai => {
            let url = match provider {
                AgentProviderKind::Openrouter => {
                    "https://openrouter.ai/api/v1/chat/completions"
                }
                _ => "https://api.openai.com/v1/chat/completions",
            };
            let body = json!({
                "model": model_id,
                "messages": [
                    { "role": "system", "content": SUMMARY_SYSTEM_PROMPT },
                    { "role": "user", "content": user_content },
                ],
                "stream": false,
            });
            let mut req = client
                .post(url)
                .bearer_auth(api_key)
                .header("Content-Type", "application/json");
            if matches!(provider, AgentProviderKind::Openrouter) {
                req = req
                    .header("HTTP-Referer", "https://bitslix.com/blxcode")
                    .header("X-Title", "blxcode");
            }
            let res = req
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("compaction request: {e}"))?;
            let res = res
                .error_for_status()
                .map_err(|e| format!("compaction request: {e}"))?;
            let v: Value = res
                .json()
                .await
                .map_err(|e| format!("compaction parse: {e}"))?;
            let summary = v
                .pointer("/choices/0/message/content")
                .and_then(|c| c.as_str())
                .unwrap_or_default()
                .to_string();
            Ok(summary)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_openai_and_anthropic_shapes() {
        let convo = vec![
            json!({ "role": "user", "content": "fix the bug" }),
            json!({ "role": "assistant", "content": [{ "type": "text", "text": "looking" }] }),
            json!({ "role": "assistant", "content": [{ "type": "tool_use", "name": "read_file" }] }),
            json!({ "role": "user", "content": [{ "type": "tool_result", "content": [{ "type": "text", "text": "file body" }] }] }),
        ];
        let t = flatten_conversation(&convo);
        assert!(t.contains("User: fix the bug"));
        assert!(t.contains("Assistant: looking"));
        assert!(t.contains("[tool call: read_file]"));
        assert!(t.contains("[tool result: file body]"));
    }

    #[test]
    fn synthetic_history_is_user_first_pair() {
        let h = synthetic_history("recap");
        assert_eq!(h.len(), 2);
        assert_eq!(h[0]["role"], "user");
        assert_eq!(h[1]["role"], "assistant");
        assert!(h[0]["content"].as_str().unwrap().contains("recap"));
    }
}
