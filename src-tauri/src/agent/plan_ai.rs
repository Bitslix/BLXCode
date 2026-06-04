//! AI-assisted plan (and task) generation. Reuses the agent tab's configured
//! provider/model/key (via [`crate::agent_settings`]) to turn a short prompt
//! into a Skill-conformant Markdown plan. Non-streaming, one-shot — the same
//! pattern as [`crate::git_commit_ai`]. The output is plain Markdown that the
//! existing `plan_create` / `plan_load` tools can persist and sync verbatim.

use crate::agent::oneshot;
use crate::agent_settings::{load_settings_pub, provider_key_pub};
use serde::Serialize;
use tauri::AppHandle;

/// Plans are longer than a commit message, so allow a roomier budget.
const MAX_TOKENS: u64 = 2048;

/// Shared rules for both modes: emit plain Markdown only.
const SYSTEM_PROMPT_BASE: &str =
    "You are a tool that writes durable implementation plans as Markdown. \
Reply with ONLY the plan Markdown: no preamble, no commentary, and do NOT wrap the whole \
document in a code fence. \
The document MUST start with a single top-level title line `# <Title>`, followed by short \
prose sections (for example `## Goal` and `## Steps`) describing the approach. \
The document MUST end with a `## Tasks` section.";

/// Plan-only: keep the `## Tasks` section present but empty.
const SYSTEM_PROMPT_PLAN: &str = "Leave the `## Tasks` section empty (just the heading), so tasks \
can be filled in later.";

/// Plan + tasks: populate `## Tasks` with concrete task lines in the skill format.
const SYSTEM_PROMPT_TASKS: &str = "Populate the `## Tasks` section with concrete task lines using \
EXACTLY this syntax, one per line:\n\
`- [ ] ` followed by a short kebab-case id in backticks, then ` - ` and an imperative title.\n\
Example: `- [ ] \\`setup-db\\` - Add the database schema migration`\n\
Task ids must be short, unique, and kebab-case. Only emit pending tasks (`- [ ]`); never use \
the other status markers (`- [>]`, `- [!]`, `- [x]`, `- [-]`).";

#[derive(Serialize)]
pub struct GeneratedPlan {
    pub title: String,
    pub markdown: String,
}

#[tauri::command]
pub async fn plan_generate_ai(
    app: AppHandle,
    prompt: String,
    with_tasks: bool,
) -> Result<GeneratedPlan, String> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("empty prompt".into());
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

    let system = if with_tasks {
        format!("{SYSTEM_PROMPT_BASE} {SYSTEM_PROMPT_TASKS}")
    } else {
        format!("{SYSTEM_PROMPT_BASE} {SYSTEM_PROMPT_PLAN}")
    };

    let user = format!("Write a plan for the following request:\n\n{prompt}");
    let raw = oneshot::complete_text(&settings, &api_key, &system, &user, MAX_TOKENS).await?;
    let cleaned = clean_markdown(&raw);
    if cleaned.is_empty() {
        return Err("model returned an empty plan".into());
    }
    let title = extract_title(&cleaned).unwrap_or_else(|| derive_title(&prompt));
    let markdown = ensure_tasks_section(&cleaned);
    Ok(GeneratedPlan { title, markdown })
}

/// Strip a code fence wrapping the whole document (some models add one despite
/// the instruction) and trim surrounding whitespace.
fn clean_markdown(raw: &str) -> String {
    let mut text = raw.trim();
    if text.starts_with("```") {
        if let Some(nl) = text.find('\n') {
            text = &text[nl + 1..];
        }
        if let Some(end) = text.rfind("```") {
            text = &text[..end];
        }
        text = text.trim();
    }
    text.to_string()
}

/// Pull the title out of the first `# ` heading line, if any.
fn extract_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let trimmed = line.trim_start();
        trimmed
            .strip_prefix("# ")
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    })
}

/// Fallback title derived from the prompt when the model omits a `# ` heading.
fn derive_title(prompt: &str) -> String {
    let first = prompt.lines().next().unwrap_or("").trim();
    let title: String = first.chars().take(80).collect();
    if title.is_empty() {
        "New plan".into()
    } else {
        title
    }
}

/// Guarantee a `## Tasks` section exists so `plan_load` never runs empty.
fn ensure_tasks_section(markdown: &str) -> String {
    let has_tasks = markdown.lines().any(|line| {
        let t = line.trim().to_ascii_lowercase();
        t == "## tasks" || t == "## todos"
    });
    if has_tasks {
        markdown.to_string()
    } else {
        format!("{}\n\n## Tasks\n", markdown.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::{clean_markdown, ensure_tasks_section, extract_title};

    #[test]
    fn strips_wrapping_fence() {
        let raw = "```markdown\n# Plan\n\n## Tasks\n```";
        assert_eq!(clean_markdown(raw), "# Plan\n\n## Tasks");
    }

    #[test]
    fn leaves_plain_markdown() {
        let raw = "  # Plan\n\n## Tasks\n  ";
        assert_eq!(clean_markdown(raw), "# Plan\n\n## Tasks");
    }

    #[test]
    fn extracts_title() {
        let md = "intro\n# My Great Plan\n## Tasks\n";
        assert_eq!(extract_title(md).as_deref(), Some("My Great Plan"));
    }

    #[test]
    fn extract_title_none_without_heading() {
        assert_eq!(extract_title("no heading here"), None);
    }

    #[test]
    fn appends_missing_tasks_section() {
        let md = "# Plan\n\nSome prose.";
        let out = ensure_tasks_section(md);
        assert!(out.ends_with("## Tasks\n"));
        assert!(out.contains("Some prose."));
    }

    #[test]
    fn keeps_existing_tasks_section() {
        let md = "# Plan\n\n## Tasks\n\n- [ ] `a` - do it\n";
        assert_eq!(ensure_tasks_section(md), md);
    }
}
