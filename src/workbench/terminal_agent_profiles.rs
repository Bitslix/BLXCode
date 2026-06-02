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
    /// How this CLI accepts a reasoning-effort override at launch time. Some
    /// agents only support effort through persistent config files; those expose
    /// no selectable launch effort.
    pub effort_passing: EffortPassing,
    /// Built-in selectable effort levels for this CLI agent. Empty means the
    /// launch command cannot safely override effort for this CLI.
    pub efforts: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffortPassing {
    None,
    #[allow(dead_code)]
    Flag(&'static str),
    ConfigOverride(&'static str),
    EnvVar(&'static str),
}

pub const TERMINAL_AGENT_PROFILES: &[TerminalAgentProfile] = &[
    TerminalAgentProfile {
        slug: "claude",
        display_name: "Claude",
        binary: "claude",
        resume_subcommand: &["--resume"],
        model_flag: "--model",
        models: &["opus", "sonnet", "haiku"],
        effort_passing: EffortPassing::EnvVar("CLAUDE_CODE_EFFORT_LEVEL"),
        efforts: &["low", "medium", "high", "xhigh", "max"],
    },
    TerminalAgentProfile {
        slug: "codex",
        display_name: "Codex",
        binary: "codex",
        resume_subcommand: &["resume"],
        model_flag: "--model",
        models: &["gpt-5.2-codex", "gpt-5.1-codex-max", "gpt-5"],
        effort_passing: EffortPassing::ConfigOverride("model_reasoning_effort"),
        efforts: &["minimal", "low", "medium", "high", "xhigh"],
    },
    TerminalAgentProfile {
        slug: "gemini",
        display_name: "Gemini",
        binary: "gemini",
        resume_subcommand: &["--resume"],
        model_flag: "--model",
        models: &["gemini-3-pro", "gemini-3-flash", "gemini-2.5-pro"],
        effort_passing: EffortPassing::None,
        efforts: &[],
    },
    TerminalAgentProfile {
        slug: "opencode",
        display_name: "OpenCode",
        binary: "opencode",
        resume_subcommand: &["--session"],
        model_flag: "--model",
        models: &["anthropic/claude-opus-4-8", "openai/gpt-5"],
        effort_passing: EffortPassing::None,
        efforts: &[],
    },
    TerminalAgentProfile {
        slug: "cursor",
        display_name: "Cursor",
        binary: "cursor-agent",
        resume_subcommand: &["--resume"],
        model_flag: "--model",
        models: &["gpt-5.5", "gpt-5.3-codex", "sonnet-4.5"],
        effort_passing: EffortPassing::None,
        efforts: &[],
    },
];

/// Built-in selectable model identifiers for a CLI agent slug (empty if the
/// slug is unknown).
pub fn terminal_agent_models(slug: &str) -> &'static [&'static str] {
    terminal_agent_profile(slug)
        .map(|p| p.models)
        .unwrap_or(&[])
}

/// Built-in selectable effort identifiers for a CLI agent slug. Empty means
/// the CLI supports effort only through config files or not at all.
pub fn terminal_agent_efforts(slug: &str) -> &'static [&'static str] {
    terminal_agent_profile(slug)
        .map(|p| p.efforts)
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

fn shell_env_assignment(key: &str, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        || trimmed.is_empty()
        || trimmed.len() > 256
        || trimmed.chars().any(char::is_control)
    {
        return None;
    }
    shell_single_quoted_arg(trimmed).map(|quoted| format!("{key}={quoted}"))
}

fn effort_segment(profile: &TerminalAgentProfile, effort: Option<&str>) -> (String, String) {
    let Some(raw) = effort else {
        return (String::new(), String::new());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || !profile.efforts.iter().any(|known| *known == trimmed) {
        return (String::new(), String::new());
    }
    match profile.effort_passing {
        EffortPassing::None => (String::new(), String::new()),
        EffortPassing::Flag(flag) => shell_single_quoted_arg(trimmed)
            .map(|quoted| (String::new(), format!(" {flag} {quoted}")))
            .unwrap_or_default(),
        EffortPassing::ConfigOverride(key) => {
            shell_single_quoted_arg(&format!("{key}={trimmed:?}"))
                .map(|quoted| (String::new(), format!(" -c {quoted}")))
                .unwrap_or_default()
        }
        EffortPassing::EnvVar(key) => shell_env_assignment(key, trimmed)
            .map(|env| (format!("{env} "), String::new()))
            .unwrap_or_default(),
    }
}

/// Format the command pasted into a login shell to launch or resume a CLI
/// agent. The PTY path stays interactive; one-shot/headless modes are only
/// documented for users and are not used by the default harness launch.
pub fn terminal_agent_launch_command(
    slug: &str,
    resume_id: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
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
    let (effort_prefix, effort_seg) = effort_segment(profile, effort);
    if let Some(resume_id) = resume_id.and_then(shell_single_quoted_arg) {
        let args = profile
            .resume_subcommand
            .iter()
            .copied()
            .chain(std::iter::once(resume_id.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
        return format!(
            "{effort_prefix}{} {args}{model_seg}{effort_seg}\r",
            profile.binary
        );
    }
    format!("{effort_prefix}{}{model_seg}{effort_seg}\r", profile.binary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_command_uses_profile_binary() {
        assert_eq!(
            terminal_agent_launch_command("cursor", None, None, None),
            "cursor-agent\r"
        );
        assert_eq!(
            terminal_agent_launch_command("codex", None, None, None),
            "codex\r"
        );
    }

    #[test]
    fn resume_command_quotes_session_id() {
        assert_eq!(
            terminal_agent_launch_command("opencode", Some("abc def"), None, None),
            "opencode --session 'abc def'\r"
        );
        assert_eq!(
            terminal_agent_launch_command("codex", Some("abc'xyz"), None, None),
            "codex resume 'abc'\"'\"'xyz'\r"
        );
    }

    #[test]
    fn launch_command_appends_model_flag() {
        assert_eq!(
            terminal_agent_launch_command("claude", None, Some("opus"), None),
            "claude --model 'opus'\r"
        );
        // Model is appended after the resume segment too.
        assert_eq!(
            terminal_agent_launch_command("claude", Some("sess1"), Some("sonnet"), None),
            "claude --resume 'sess1' --model 'sonnet'\r"
        );
        // Empty / whitespace model is ignored.
        assert_eq!(
            terminal_agent_launch_command("claude", None, Some("   "), None),
            "claude\r"
        );
    }

    #[test]
    fn launch_command_applies_effort_per_cli() {
        assert_eq!(
            terminal_agent_launch_command("claude", None, Some("opus"), Some("high")),
            "CLAUDE_CODE_EFFORT_LEVEL='high' claude --model 'opus'\r"
        );
        assert_eq!(
            terminal_agent_launch_command("codex", None, Some("gpt-5.2-codex"), Some("xhigh")),
            "codex --model 'gpt-5.2-codex' -c 'model_reasoning_effort=\"xhigh\"'\r"
        );
        assert_eq!(
            terminal_agent_launch_command("gemini", None, Some("gemini-3-pro"), Some("high")),
            "gemini --model 'gemini-3-pro'\r"
        );
        assert_eq!(
            terminal_agent_launch_command("claude", None, None, Some("unknown")),
            "claude\r"
        );
        let fake = TerminalAgentProfile {
            slug: "fake",
            display_name: "Fake",
            binary: "fake",
            resume_subcommand: &[],
            model_flag: "",
            models: &[],
            effort_passing: EffortPassing::Flag("--effort"),
            efforts: &["low"],
        };
        assert_eq!(
            effort_segment(&fake, Some("low")),
            (String::new(), " --effort 'low'".into())
        );
    }

    #[test]
    fn models_catalog_lookup() {
        assert!(terminal_agent_models("claude").contains(&"opus"));
        assert!(terminal_agent_efforts("codex").contains(&"xhigh"));
        assert!(terminal_agent_efforts("opencode").is_empty());
        assert!(terminal_agent_models("unknown").is_empty());
    }
}
