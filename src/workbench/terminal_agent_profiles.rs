//! Shared launch/resume metadata for terminal CLI agents.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalAgentProfile {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub binary: &'static str,
    pub resume_subcommand: &'static [&'static str],
    /// CLI flag used to pin a model, e.g. `--model`. Empty means the agent has
    /// no model flag and model selection is ignored for it.
    pub model_flag: &'static str,
    /// Built-in catalog of selectable model identifiers for this CLI agent.
    /// Sensible editable defaults; an empty selection always falls back to the
    /// agent's own default model (no flag is passed).
    pub models: &'static [&'static str],
}

pub const TERMINAL_AGENT_PROFILES: &[TerminalAgentProfile] = &[
    TerminalAgentProfile {
        slug: "claude",
        display_name: "Claude",
        binary: "claude",
        resume_subcommand: &["--resume"],
        model_flag: "--model",
        models: &["opus", "sonnet", "haiku"],
    },
    TerminalAgentProfile {
        slug: "codex",
        display_name: "Codex",
        binary: "codex",
        resume_subcommand: &["resume"],
        model_flag: "--model",
        models: &["gpt-5", "gpt-5-codex"],
    },
    TerminalAgentProfile {
        slug: "gemini",
        display_name: "Gemini",
        binary: "gemini",
        resume_subcommand: &["--resume"],
        model_flag: "--model",
        models: &["gemini-2.5-pro", "gemini-2.5-flash"],
    },
    TerminalAgentProfile {
        slug: "opencode",
        display_name: "OpenCode",
        binary: "opencode",
        resume_subcommand: &["--session"],
        model_flag: "--model",
        models: &["anthropic/claude-sonnet-4-5", "openai/gpt-5"],
    },
    TerminalAgentProfile {
        slug: "cursor",
        display_name: "Cursor",
        binary: "cursor-agent",
        resume_subcommand: &["--resume"],
        model_flag: "--model",
        models: &["gpt-5", "sonnet-4.5"],
    },
];

/// Built-in selectable model identifiers for a CLI agent slug (empty if the
/// slug is unknown).
pub fn terminal_agent_models(slug: &str) -> &'static [&'static str] {
    terminal_agent_profile(slug)
        .map(|p| p.models)
        .unwrap_or(&[])
}

pub fn supported_terminal_agent_slugs() -> impl Iterator<Item = &'static str> {
    TERMINAL_AGENT_PROFILES.iter().map(|profile| profile.slug)
}

pub fn terminal_agent_profile(slug: &str) -> Option<&'static TerminalAgentProfile> {
    let slug = slug.trim();
    TERMINAL_AGENT_PROFILES
        .iter()
        .find(|profile| profile.slug == slug)
}

pub fn is_supported_terminal_agent_slug(slug: &str) -> bool {
    terminal_agent_profile(slug).is_some()
}

/// Single-quote for POSIX shells: `'` becomes `'"'"'`.
fn shell_single_quoted_arg(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 8192 || trimmed.chars().any(char::is_control) {
        return None;
    }
    Some(format!("'{}'", trimmed.replace('\'', "'\"'\"'")))
}

/// Format the command pasted into a login shell to launch or resume a CLI
/// agent. The PTY path stays interactive; one-shot/headless modes are only
/// documented for users and are not used by the default harness launch.
pub fn terminal_agent_launch_command(
    slug: &str,
    resume_id: Option<&str>,
    model: Option<&str>,
) -> String {
    let Some(profile) = terminal_agent_profile(slug) else {
        return format!("{}\r", slug.trim());
    };
    // Optional `--model <id>` segment, only when the agent declares a flag and
    // the model identifier is safe to quote.
    let model_seg = match (profile.model_flag, model.and_then(shell_single_quoted_arg)) {
        (flag, Some(quoted)) if !flag.is_empty() => format!(" {flag} {quoted}"),
        _ => String::new(),
    };
    if let Some(resume_id) = resume_id.and_then(shell_single_quoted_arg) {
        let args = profile
            .resume_subcommand
            .iter()
            .copied()
            .chain(std::iter::once(resume_id.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
        return format!("{} {args}{model_seg}\r", profile.binary);
    }
    format!("{}{model_seg}\r", profile.binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_command_uses_profile_binary() {
        assert_eq!(
            terminal_agent_launch_command("cursor", None, None),
            "cursor-agent\r"
        );
        assert_eq!(terminal_agent_launch_command("codex", None, None), "codex\r");
    }

    #[test]
    fn resume_command_quotes_session_id() {
        assert_eq!(
            terminal_agent_launch_command("opencode", Some("abc def"), None),
            "opencode --session 'abc def'\r"
        );
        assert_eq!(
            terminal_agent_launch_command("codex", Some("abc'xyz"), None),
            "codex resume 'abc'\"'\"'xyz'\r"
        );
    }

    #[test]
    fn launch_command_appends_model_flag() {
        assert_eq!(
            terminal_agent_launch_command("claude", None, Some("opus")),
            "claude --model 'opus'\r"
        );
        // Model is appended after the resume segment too.
        assert_eq!(
            terminal_agent_launch_command("claude", Some("sess1"), Some("sonnet")),
            "claude --resume 'sess1' --model 'sonnet'\r"
        );
        // Empty / whitespace model is ignored.
        assert_eq!(
            terminal_agent_launch_command("claude", None, Some("   ")),
            "claude\r"
        );
    }

    #[test]
    fn models_catalog_lookup() {
        assert!(terminal_agent_models("claude").contains(&"opus"));
        assert!(terminal_agent_models("unknown").is_empty());
    }
}
