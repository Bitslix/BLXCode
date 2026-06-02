//! Built-in **session roles** ("BLXCode Harness Session Mode"). Each role is a
//! specialized harness skill embedded from `harness_skills/specialized/*.md`.
//!
//! A role is chosen per workspace in the Create-Workspace flow and handed to
//! the BLXCode Agent by appending its operational text to the shared
//! `system_prompt` (see [`crate::agent::system_prompt`]). The role's
//! frontmatter (`name`, `description`, `tools`, `model`, `color`) drives the
//! picker dropdown and the colored sub-line in the agent name badge.
//!
//! Roles are read-only built-ins, parallel to but separate from the user
//! `.agents/skills` store.

use serde::Serialize;

/// `(slug, markdown_with_frontmatter)` for every specialized role, embedded at
/// compile time. The slug must equal the `name:` frontmatter value.
pub const SPECIALIZED_ROLES: &[(&str, &str)] = &[
    (
        "architect",
        include_str!("harness_skills/specialized/architect.md"),
    ),
    (
        "coordinator",
        include_str!("harness_skills/specialized/coordinator.md"),
    ),
    (
        "doc-updater",
        include_str!("harness_skills/specialized/doc-updater.md"),
    ),
    (
        "harness-optimizer",
        include_str!("harness_skills/specialized/harness-optimizer.md"),
    ),
    (
        "pr-test-analyzer",
        include_str!("harness_skills/specialized/pr-test-analyzer.md"),
    ),
    (
        "refactor-cleaner",
        include_str!("harness_skills/specialized/refactor-cleaner.md"),
    ),
    (
        "security-reviewer",
        include_str!("harness_skills/specialized/security-reviewer.md"),
    ),
];

/// Frontmatter-derived metadata for one role, surfaced to the UI picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleMeta {
    /// Stable identifier (== frontmatter `name`, == registry slug).
    pub slug: String,
    /// Human title for the dropdown (defaults to a Title-Cased slug).
    pub title: String,
    /// One-line description shown as the dropdown sub-line.
    pub description: String,
    /// Tool names declared in the role frontmatter.
    pub tools: Vec<String>,
    /// Accent color from frontmatter (theme keyword or hex). Empty if absent.
    pub color: String,
    /// Preferred terminal CLI-agent slug for this role (advisory; e.g.
    /// `claude`). Empty if absent. Never changes the BLXCode Agent's own
    /// provider/model — that always comes from Settings.
    pub provider: String,
    /// Advisory list of model identifiers the role is designed for, across the
    /// built-in CLI agents. Shown in the role picker; does not pin a model.
    pub models: Vec<String>,
}

/// Returns metadata for every embedded role, in registry order.
#[must_use]
pub fn list_roles() -> Vec<RoleMeta> {
    SPECIALIZED_ROLES
        .iter()
        .map(|(slug, body)| parse_meta(slug, body))
        .collect()
}

/// Returns metadata for one role by slug, if it exists.
#[must_use]
pub fn role_meta(slug: &str) -> Option<RoleMeta> {
    SPECIALIZED_ROLES
        .iter()
        .find(|(s, _)| *s == slug)
        .map(|(s, body)| parse_meta(s, body))
}

/// Returns the operational prompt text for one role: the markdown body with
/// the YAML frontmatter and the duplicated "Prompt Defense Baseline" section
/// stripped (Security in the system prompt already covers it). Returns `None`
/// for an unknown slug.
#[must_use]
pub fn role_prompt_body(slug: &str) -> Option<String> {
    let (_, raw) = SPECIALIZED_ROLES.iter().find(|(s, _)| *s == slug)?;
    let body = strip_frontmatter(raw);
    Some(strip_prompt_defense_baseline(body).trim().to_owned())
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_meta(slug: &str, raw: &str) -> RoleMeta {
    let fm = frontmatter(raw);
    let title = fm_value(fm, "name")
        .filter(|v| !v.is_empty())
        .map(|name| title_case(&name))
        .unwrap_or_else(|| title_case(slug));
    RoleMeta {
        slug: slug.to_owned(),
        title,
        description: fm_value(fm, "description").unwrap_or_default(),
        tools: fm_value(fm, "tools").map(|v| parse_tools(&v)).unwrap_or_default(),
        color: fm_value(fm, "color").unwrap_or_default(),
        provider: fm_value(fm, "provider").unwrap_or_default(),
        models: fm_value(fm, "models").map(|v| parse_tools(&v)).unwrap_or_default(),
    }
}

/// Returns the raw frontmatter block (between the leading `---` fences),
/// without the fences. Empty string when no frontmatter is present.
fn frontmatter(raw: &str) -> &str {
    let Some(rest) = raw.strip_prefix("---") else {
        return "";
    };
    let rest = rest
        .strip_prefix('\r')
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or(rest);
    for marker in ["\n---\n", "\n---\r\n", "\r\n---\r\n", "\r\n---\n"] {
        if let Some(pos) = rest.find(marker) {
            return &rest[..pos];
        }
    }
    ""
}

/// Returns the markdown body after the frontmatter (frontmatter stripped).
fn strip_frontmatter(raw: &str) -> &str {
    let Some(rest) = raw.strip_prefix("---") else {
        return raw;
    };
    let rest = rest
        .strip_prefix('\r')
        .or_else(|| rest.strip_prefix('\n'))
        .unwrap_or(rest);
    for marker in ["\n---\n", "\n---\r\n", "\r\n---\r\n", "\r\n---\n"] {
        if let Some(pos) = rest.find(marker) {
            return &rest[pos + marker.len()..];
        }
    }
    raw
}

/// Drops a leading "## Prompt Defense Baseline" section (heading + its bullet
/// list) so the role text injected into the prompt does not duplicate the
/// Security section. Anything before the heading and everything from the next
/// non-bullet, non-blank line onward is preserved.
fn strip_prompt_defense_baseline(body: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut lines = body.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.starts_with('#')
            && trimmed
                .trim_start_matches('#')
                .trim()
                .eq_ignore_ascii_case("Prompt Defense Baseline")
        {
            // Consume the baseline body: blank lines and `-` bullets.
            while let Some(next) = lines.peek() {
                let t = next.trim();
                if t.is_empty() || t.starts_with('-') {
                    lines.next();
                } else {
                    break;
                }
            }
            continue;
        }
        out.push(line);
    }
    out.join("\n")
}

/// Reads a single scalar frontmatter value by key. Returns the trimmed value
/// with surrounding quotes removed; `None` if the key is absent.
fn fm_value(fm: &str, key: &str) -> Option<String> {
    for line in fm.lines() {
        let trimmed = line.trim();
        let Some((k, v)) = trimmed.split_once(':') else {
            continue;
        };
        if k.trim().eq_ignore_ascii_case(key) {
            let val = v.trim().trim_matches('"').trim_matches('\'').trim();
            return Some(val.to_owned());
        }
    }
    None
}

/// Parses a frontmatter tools list: `["Read", "Grep"]` or `[Read, Grep]`.
fn parse_tools(raw: &str) -> Vec<String> {
    raw.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|t| t.trim().trim_matches('"').trim_matches('\'').trim().to_owned())
        .filter(|t| !t.is_empty())
        .collect()
}

/// `harness-optimizer` -> `Harness Optimizer`.
fn title_case(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_all_embedded_roles() {
        let roles = list_roles();
        assert_eq!(roles.len(), SPECIALIZED_ROLES.len());
        assert!(roles.iter().any(|r| r.slug == "coordinator"));
        assert!(roles.iter().any(|r| r.slug == "security-reviewer"));
    }

    #[test]
    fn parses_meta_fields() {
        let coord = role_meta("coordinator").expect("coordinator role");
        assert_eq!(coord.slug, "coordinator");
        assert_eq!(coord.title, "Coordinator");
        assert!(!coord.description.is_empty());
        assert_eq!(coord.color, "violet");
        assert!(coord.tools.contains(&"Read".to_string()));
        assert!(coord.tools.contains(&"Bash".to_string()));
    }

    #[test]
    fn parses_provider_and_models_list() {
        // Every role declares a provider (CLI slug) and a non-empty models list.
        for r in list_roles() {
            assert!(!r.provider.is_empty(), "role {} missing provider", r.slug);
            assert!(!r.models.is_empty(), "role {} missing models", r.slug);
        }
    }

    #[test]
    fn every_role_has_a_color() {
        for r in list_roles() {
            assert!(!r.color.is_empty(), "role {} is missing color", r.slug);
        }
    }

    #[test]
    fn parses_unquoted_tools_list() {
        // pr-test-analyzer uses `[Read, Grep, Glob, Bash]` without quotes.
        let r = role_meta("pr-test-analyzer").expect("role");
        assert!(r.tools.contains(&"Read".to_string()));
        assert!(r.tools.contains(&"Bash".to_string()));
    }

    #[test]
    fn prompt_body_strips_frontmatter_and_baseline() {
        let body = role_prompt_body("architect").expect("body");
        assert!(!body.contains("Prompt Defense Baseline"));
        assert!(!body.starts_with("---"));
        assert!(!body.contains("name: architect"));
        assert!(body.contains("architect"));
    }

    #[test]
    fn unknown_slug_returns_none() {
        assert!(role_meta("nope").is_none());
        assert!(role_prompt_body("nope").is_none());
    }
}
