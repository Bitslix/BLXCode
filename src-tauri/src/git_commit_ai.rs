//! AI-assisted commit message generation. Reuses the agent tab's configured
//! provider/model/key (via [`crate::agent_settings`]) to turn the staged diff
//! into a Conventional-Commits-style message. Non-streaming, one-shot.

use crate::agent::oneshot;
use crate::agent_settings::{load_settings_pub, provider_key_pub};
use crate::git_info::{find_git_dir, git_cli_available};
use crate::git_status::GIT_MISSING_CODE;
use crate::proc::command;
use std::path::Path;
use tauri::AppHandle;

/// Cap the diff we send to the model so a huge staged change set doesn't blow
/// the context window (and cost). The model still sees enough to summarize.
const MAX_DIFF_BYTES: usize = 24 * 1024;
const MAX_TOKENS: u64 = 512;

const SYSTEM_PROMPT: &str = "You are a tool that writes git commit messages. \
Given a staged diff, reply with a single Conventional Commits message and nothing else. \
Use a concise lowercase type prefix (feat, fix, docs, style, refactor, test, chore) \
followed by a short imperative summary under ~72 characters. Add a blank line and a \
brief body only if the change genuinely needs explanation. Do not wrap the message in \
quotes or code fences, and do not add any commentary.";

#[tauri::command]
pub async fn git_generate_commit_message(app: AppHandle, cwd: String) -> Result<String, String> {
    if !git_cli_available() {
        return Err(GIT_MISSING_CODE.into());
    }
    let work_tree = resolve_work_tree(&cwd)?;

    let diff = staged_diff(&work_tree)?;
    if diff.trim().is_empty() {
        return Err("nothing staged".into());
    }
    let mut diff = diff;
    if diff.len() > MAX_DIFF_BYTES {
        // Truncate on a char boundary so the slice is valid UTF-8.
        let mut end = MAX_DIFF_BYTES;
        while end > 0 && !diff.is_char_boundary(end) {
            end -= 1;
        }
        diff.truncate(end);
        diff.push_str("\n… (diff truncated)");
    }

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

    let user = format!("Staged diff:\n\n{diff}");
    let raw = oneshot::complete_text(&settings, &api_key, SYSTEM_PROMPT, &user, MAX_TOKENS).await?;
    let cleaned = clean_message(&raw);
    if cleaned.is_empty() {
        return Err("model returned an empty message".into());
    }
    Ok(cleaned)
}

fn resolve_work_tree(cwd: &str) -> Result<std::path::PathBuf, String> {
    let trimmed = cwd.trim();
    if trimmed.is_empty() {
        return Err("cwd is empty".into());
    }
    let git_dir = find_git_dir(Path::new(trimmed)).ok_or_else(|| "not a git repository".to_string())?;
    git_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "invalid git dir".to_string())
}

fn staged_diff(work_tree: &Path) -> Result<String, String> {
    let out = command("git")
        .arg("-C")
        .arg(work_tree)
        .args(["diff", "--cached", "--no-color"])
        .output()
        .map_err(|e| format!("git diff: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git diff: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Strip wrapping code fences / surrounding quotes some models add despite the
/// instruction, and trim trailing whitespace.
fn clean_message(raw: &str) -> String {
    let mut text = raw.trim();
    if text.starts_with("```") {
        // Drop the opening fence line and a trailing fence if present.
        if let Some(nl) = text.find('\n') {
            text = &text[nl + 1..];
        }
        if let Some(end) = text.rfind("```") {
            text = &text[..end];
        }
        text = text.trim();
    }
    // Strip a single pair of wrapping quotes around the whole message.
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        text = text[1..text.len() - 1].trim();
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::clean_message;

    #[test]
    fn strips_code_fences() {
        let raw = "```\nfeat: add thing\n```";
        assert_eq!(clean_message(raw), "feat: add thing");
    }

    #[test]
    fn strips_language_fence() {
        let raw = "```text\nfix: bug\n\nbody line\n```";
        assert_eq!(clean_message(raw), "fix: bug\n\nbody line");
    }

    #[test]
    fn strips_wrapping_quotes() {
        let raw = "\"chore: tidy up\"";
        assert_eq!(clean_message(raw), "chore: tidy up");
    }

    #[test]
    fn leaves_plain_message() {
        let raw = "  refactor: simplify loop  ";
        assert_eq!(clean_message(raw), "refactor: simplify loop");
    }
}
