//! Per-agent accent helpers for terminal focus rings and titlebar pulses.

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

/// Parse `"{storage_key}:{slot_id}:{pane_id}"` → workspace storage key
/// (UUID v4 hex). The storage key contains no colons, so a single split
/// suffices. Empty or malformed inputs return `None`.
#[must_use]
pub fn terminal_key_storage_key(key: &str) -> Option<String> {
    let first = key.split(':').next()?;
    if first.is_empty() {
        return None;
    }
    Some(first.to_string())
}

/// Legacy parser for pre-UUID terminal keys (`"{workspace_id}:{slot}:{pane}"`).
///
/// New workbench state must use [`terminal_key_storage_key`] because the
/// terminal key prefix is now a workspace UUID, not the numeric UI id.
#[allow(dead_code)]
#[deprecated(note = "use terminal_key_storage_key for UUID-backed terminal keys")]
#[must_use]
pub fn terminal_key_workspace_id(key: &str) -> Option<u64> {
    key.split(':').next()?.parse().ok()
}
