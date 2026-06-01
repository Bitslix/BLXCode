//! Static fallback table for model context-window sizes.
//!
//! The authoritative source is the provider's own model metadata —
//! OpenRouter returns `context_length` in `/models`, which we cache on each
//! `ProviderModelEntry`. Direct providers (Anthropic, OpenAI) do not expose
//! the window in their model list, so this table supplies a best-effort
//! value keyed by id substrings. Mirrors the "static id→value table for
//! direct providers" pattern in `pricing.rs`.
//!
//! Unknown models return `None`; the UI then shows the raw token count and
//! skips the percentage meter rather than inventing a denominator.

use crate::agent_settings::AgentProviderKind;

/// Resolve a context-window size (in tokens) for a model that has no
/// provider-reported `context_length`. Returns `None` for unrecognised ids.
pub fn fallback_context_length(provider: AgentProviderKind, model_id: &str) -> Option<u64> {
    let id = model_id.trim().to_ascii_lowercase();
    if id.is_empty() {
        return None;
    }

    // Substring heuristics — work for both bare ids (Anthropic/OpenAI) and
    // OpenRouter's `vendor/model` form, so the table covers all three
    // providers when the live `context_length` is unavailable (offline,
    // curated-only, or a direct provider).
    let by_family = context_length_by_family(&id);
    if by_family.is_some() {
        return by_family;
    }

    // Provider-level last resort for ids we don't otherwise recognise.
    match provider {
        // Modern Claude models are uniformly 200K (some expose a 1M beta,
        // but 200K is the safe default budget to show).
        AgentProviderKind::Anthropic if id.starts_with("claude") => Some(200_000),
        _ => None,
    }
}

/// Family-keyed lookup shared across providers. Ordered most-specific first.
fn context_length_by_family(id: &str) -> Option<u64> {
    // Anthropic Claude family (direct ids or `anthropic/...` on OpenRouter).
    if id.contains("claude") {
        return Some(200_000);
    }
    // Google Gemini 1.5/2.0/2.5 — 1M+ token windows.
    if id.contains("gemini-2.5") || id.contains("gemini-2.0") || id.contains("gemini-1.5") {
        return Some(1_048_576);
    }
    if id.contains("gemini") {
        return Some(1_000_000);
    }
    // OpenAI GPT-5 family — 400K combined context.
    if id.contains("gpt-5") {
        return Some(400_000);
    }
    // OpenAI GPT-4.1 family — 1M context.
    if id.contains("gpt-4.1") {
        return Some(1_047_576);
    }
    // GPT-4o / o-series — 128K context.
    if id.contains("gpt-4o") || id.contains("o1") || id.contains("o3") || id.contains("o4") {
        return Some(128_000);
    }
    // Meta Llama 3.x / 4 — commonly 128K.
    if id.contains("llama-3") || id.contains("llama3") || id.contains("llama-4") {
        return Some(128_000);
    }
    // DeepSeek V3 / R1 — 128K (164K on some hosts; 128K is the safe budget).
    if id.contains("deepseek") {
        return Some(128_000);
    }
    // Mistral Large / mixtral — 128K / 64K; use 128K for the large line.
    if id.contains("mistral-large") || id.contains("mistral large") {
        return Some(128_000);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_resolves_via_family_for_any_provider() {
        assert_eq!(
            fallback_context_length(AgentProviderKind::Anthropic, "claude-sonnet-4-5"),
            Some(200_000)
        );
        assert_eq!(
            fallback_context_length(AgentProviderKind::Openrouter, "anthropic/claude-opus-4-1"),
            Some(200_000)
        );
    }

    #[test]
    fn gpt5_and_gemini_known() {
        assert_eq!(
            fallback_context_length(AgentProviderKind::Openai, "gpt-5"),
            Some(400_000)
        );
        assert_eq!(
            fallback_context_length(AgentProviderKind::Openrouter, "google/gemini-2.5-pro"),
            Some(1_048_576)
        );
    }

    #[test]
    fn unknown_returns_none() {
        assert_eq!(
            fallback_context_length(AgentProviderKind::Openrouter, "some/unknown-model"),
            None
        );
        assert_eq!(fallback_context_length(AgentProviderKind::Openai, ""), None);
    }
}
