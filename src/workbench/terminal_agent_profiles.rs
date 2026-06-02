//! Shared launch/resume metadata for terminal CLI agents.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalAgentProfile {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub binary: &'static str,
    pub resume_subcommand: &'static [&'static str],
}

pub const TERMINAL_AGENT_PROFILES: &[TerminalAgentProfile] = &[
    TerminalAgentProfile {
        slug: "claude",
        display_name: "Claude",
        binary: "claude",
        resume_subcommand: &["--resume"],
    },
    TerminalAgentProfile {
        slug: "codex",
        display_name: "Codex",
        binary: "codex",
        resume_subcommand: &["resume"],
    },
    TerminalAgentProfile {
        slug: "gemini",
        display_name: "Gemini",
        binary: "gemini",
        resume_subcommand: &["--resume"],
    },
    TerminalAgentProfile {
        slug: "opencode",
        display_name: "OpenCode",
        binary: "opencode",
        resume_subcommand: &["--session"],
    },
    TerminalAgentProfile {
        slug: "cursor",
        display_name: "Cursor",
        binary: "cursor-agent",
        resume_subcommand: &["--resume"],
    },
];

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
pub fn terminal_agent_launch_command(slug: &str, resume_id: Option<&str>) -> String {
    let Some(profile) = terminal_agent_profile(slug) else {
        return format!("{}\r", slug.trim());
    };
    if let Some(resume_id) = resume_id.and_then(shell_single_quoted_arg) {
        let args = profile
            .resume_subcommand
            .iter()
            .copied()
            .chain(std::iter::once(resume_id.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
        return format!("{} {args}\r", profile.binary);
    }
    format!("{}\r", profile.binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_command_uses_profile_binary() {
        assert_eq!(
            terminal_agent_launch_command("cursor", None),
            "cursor-agent\r"
        );
        assert_eq!(terminal_agent_launch_command("codex", None), "codex\r");
    }

    #[test]
    fn resume_command_quotes_session_id() {
        assert_eq!(
            terminal_agent_launch_command("opencode", Some("abc def")),
            "opencode --session 'abc def'\r"
        );
        assert_eq!(
            terminal_agent_launch_command("codex", Some("abc'xyz")),
            "codex resume 'abc'\"'\"'xyz'\r"
        );
    }
}
