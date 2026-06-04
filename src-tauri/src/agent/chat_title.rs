//! Isolated chat-session title generation. Reuses the configured Agent
//! provider through `oneshot::complete_text` without touching chat history.

use crate::agent::oneshot;
use crate::agent_settings::{load_settings_pub, provider_key_pub};
use serde::Serialize;
use tauri::AppHandle;

const MAX_INPUT_CHARS: usize = 4_000;
const MAX_TOKENS: u64 = 48;

const SYSTEM_PROMPT: &str = "\
You generate short BLXCode chat tab titles. Return ONLY the title text: no JSON, no quotes, \
no punctuation wrapper, no commentary.\n\n\
Rules:\n\
- Use the user's language.\n\
- Keep it between 2 and 6 words when possible.\n\
- Preserve important product, file, branch, command, or error names.\n\
- Do not include secrets, credentials, URLs with tokens, or private path details.\n\
- Do not end with a period.";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedChatTitle {
    pub title: String,
}

#[tauri::command]
pub async fn agent_generate_chat_title(
    app: AppHandle,
    prompt: String,
) -> Result<GeneratedChatTitle, String> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("empty prompt".into());
    }

    let truncated = truncate_chars(&prompt, MAX_INPUT_CHARS);
    let settings = load_settings_pub(&app)?;
    let api_key = match provider_key_pub(&app, settings.provider) {
        Ok(k) if !k.trim().is_empty() => k,
        Ok(_) | Err(_) => {
            return Err(format!(
                "no API key configured for {}",
                settings.provider.as_str()
            ));
        }
    };

    let user = format!("First user turn:\n\n{truncated}");
    let raw = oneshot::complete_text(&settings, &api_key, SYSTEM_PROMPT, &user, MAX_TOKENS).await?;
    let title = clean_title(&raw);
    if title.is_empty() {
        return Err("model returned an empty title".into());
    }
    Ok(GeneratedChatTitle { title })
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn clean_title(raw: &str) -> String {
    let mut title = strip_wrapping_fence(raw)
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string();
    title = title
        .chars()
        .filter(|ch| !matches!(ch, '\n' | '\r' | '\t'))
        .collect::<String>();
    title = title.trim_matches(['.', ':', '-', ' ']).trim().to_string();
    truncate_chars(&title, 60)
}

fn strip_wrapping_fence(raw: &str) -> &str {
    let mut text = raw.trim();
    if !text.starts_with("```") {
        return text;
    }
    let Some(first_newline) = text.find('\n') else {
        return text;
    };
    text = &text[first_newline + 1..];
    if let Some(end) = text.rfind("```") {
        text = &text[..end];
    }
    text
}

#[cfg(test)]
mod tests {
    use super::clean_title;

    #[test]
    fn clean_title_strips_wrapping_fence() {
        assert_eq!(
            clean_title("```text\nRun menu plugins\n```"),
            "Run menu plugins"
        );
    }

    #[test]
    fn clean_title_limits_length() {
        let title = clean_title("abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz1234567890");
        assert_eq!(title.chars().count(), 60);
    }
}
