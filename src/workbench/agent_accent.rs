//! Per-agent accent colors for terminal focus rings and sidebar badges.

/// CSS color for an agent slug (fleet order in [`crate::workbench::state::WORKSPACE_FLEET_AGENT_SLUGS`]).
#[must_use]
pub fn agent_accent_color(slug: &str) -> &'static str {
    match slug.trim().to_lowercase().as_str() {
        "claude" => "#e8954a",
        "codex" => "#3db8a8",
        "gemini" => "#5b9cf5",
        "opencode" => "#a67cf0",
        "cursor" => "#5ecf7a",
        _ => "#72a0ff",
    }
}

/// CSS modifier class for terminal cells (`ws-term-cell--agent-{slug}`).
#[must_use]
pub fn agent_accent_class(slug: &str) -> Option<&'static str> {
    match slug.trim().to_lowercase().as_str() {
        "claude" => Some("ws-term-cell--agent-claude"),
        "codex" => Some("ws-term-cell--agent-codex"),
        "gemini" => Some("ws-term-cell--agent-gemini"),
        "opencode" => Some("ws-term-cell--agent-opencode"),
        "cursor" => Some("ws-term-cell--agent-cursor"),
        _ => None,
    }
}

/// Parse `"{workspace_id}:{slot_id}:{pane_id}"` → workspace id.
#[must_use]
pub fn terminal_key_workspace_id(key: &str) -> Option<u64> {
    key.split(':').next()?.parse().ok()
}
