//! USD cost resolution for one `ModelRound`. OpenRouter is the canonical
//! pricing source: its `/v1/models` payload carries per-token rates that
//! we cache on [`ProviderModelEntry::pricing`]. Direct Anthropic /
//! OpenAI requests don't expose pricing in their `/models` endpoints, so
//! we map their model ids onto the matching OpenRouter entry via a small
//! static table and reuse those rates.
//!
//! When no mapping or no pricing data is available, `resolve_cost`
//! returns `None` and the UI surfaces `—` instead of a fake number.

use crate::agent_settings::{AgentProviderKind, AgentProviderSettings, ProviderModelEntry};

/// Look up the USD cost for a single round on `(provider, model_id)`.
///
/// `settings` is the persisted provider-settings struct (carries the
/// per-provider model caches from the most recent `/models` refresh).
/// `input_tokens` / `output_tokens` are the round's reported usage —
/// if either is `None` we still try to price the side we do know.
/// Returns `None` when no pricing can be resolved (missing tokens
/// **and** missing rates).
#[must_use]
pub fn resolve_cost(
    settings: &AgentProviderSettings,
    provider: AgentProviderKind,
    model_id: &str,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
) -> Option<f64> {
    let pricing = pricing_for(settings, provider, model_id)?;
    let mut total = 0.0_f64;
    let mut have_any = false;
    if let Some(p) = input_tokens {
        total += (p as f64) * pricing.prompt;
        have_any = true;
    }
    if let Some(c) = output_tokens {
        total += (c as f64) * pricing.completion;
        have_any = true;
    }
    have_any.then_some(total)
}

fn pricing_for(
    settings: &AgentProviderSettings,
    provider: AgentProviderKind,
    model_id: &str,
) -> Option<crate::agent_settings::ModelPricing> {
    // OpenRouter ships pricing in its `/models` payload — use the
    // cached entry directly.
    if matches!(provider, AgentProviderKind::Openrouter) {
        return find_in_cache(&settings.model_cache_openrouter, model_id);
    }

    // Direct providers — map id to the matching OpenRouter entry.
    let or_id = map_direct_to_openrouter(provider, model_id)?;
    find_in_cache(&settings.model_cache_openrouter, or_id)
}

fn find_in_cache(
    cache: &[ProviderModelEntry],
    model_id: &str,
) -> Option<crate::agent_settings::ModelPricing> {
    cache
        .iter()
        .find(|m| m.id == model_id)
        .and_then(|m| m.pricing)
}

/// Static mapping from a direct provider's model id to the equivalent
/// OpenRouter id (whose pricing we reuse). Add entries as new direct
/// models are exposed in the UI — the test below guards against typos.
#[must_use]
pub fn map_direct_to_openrouter(
    provider: AgentProviderKind,
    model_id: &str,
) -> Option<&'static str> {
    match (provider, model_id) {
        // Anthropic Direct → OpenRouter
        (AgentProviderKind::Anthropic, "claude-sonnet-4-5") => Some("anthropic/claude-sonnet-4.5"),
        (AgentProviderKind::Anthropic, "claude-sonnet-4-6") => Some("anthropic/claude-sonnet-4.6"),
        (AgentProviderKind::Anthropic, "claude-opus-4-1") => Some("anthropic/claude-opus-4.1"),
        (AgentProviderKind::Anthropic, "claude-opus-4-7") => Some("anthropic/claude-opus-4.7"),
        (AgentProviderKind::Anthropic, "claude-haiku-4-5-20251001") => {
            Some("anthropic/claude-haiku-4.5")
        }

        // OpenAI Direct → OpenRouter
        (AgentProviderKind::Openai, "gpt-5") => Some("openai/gpt-5"),
        (AgentProviderKind::Openai, "gpt-5-mini") => Some("openai/gpt-5-mini"),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_settings::{
        AgentProviderKind, ModelPricing, ProviderModelEntry, ThinkingLevel,
    };

    fn settings_with_openrouter(entries: Vec<ProviderModelEntry>) -> AgentProviderSettings {
        AgentProviderSettings {
            provider: AgentProviderKind::Openrouter,
            model_id: String::new(),
            thinking_level: ThinkingLevel::Medium,
            model_cache_openrouter: entries,
            model_cache_anthropic: Vec::new(),
            model_cache_openai: Vec::new(),
        }
    }

    fn entry(id: &str, prompt: f64, completion: f64) -> ProviderModelEntry {
        ProviderModelEntry {
            id: id.into(),
            label: id.into(),
            description: None,
            pricing: Some(ModelPricing { prompt, completion }),
        }
    }

    #[test]
    fn openrouter_hit_sums_prompt_and_completion() {
        let settings = settings_with_openrouter(vec![entry("openai/gpt-5", 0.000_002, 0.000_010)]);
        let cost = resolve_cost(
            &settings,
            AgentProviderKind::Openrouter,
            "openai/gpt-5",
            Some(1_000),
            Some(500),
        )
        .expect("priced");
        // 1000*0.000_002 + 500*0.000_010 = 0.002 + 0.005 = 0.007
        assert!((cost - 0.007).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn openrouter_miss_returns_none() {
        let settings = settings_with_openrouter(vec![]);
        assert!(resolve_cost(
            &settings,
            AgentProviderKind::Openrouter,
            "unknown/model",
            Some(100),
            Some(50),
        )
        .is_none());
    }

    #[test]
    fn direct_anthropic_uses_mapped_openrouter_pricing() {
        let settings = settings_with_openrouter(vec![entry(
            "anthropic/claude-opus-4.7",
            0.000_015,
            0.000_075,
        )]);
        let cost = resolve_cost(
            &settings,
            AgentProviderKind::Anthropic,
            "claude-opus-4-7",
            Some(2_000),
            Some(1_000),
        )
        .expect("priced");
        // 2000*0.000_015 + 1000*0.000_075 = 0.03 + 0.075 = 0.105
        assert!((cost - 0.105).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn direct_provider_unmapped_returns_none() {
        let settings = settings_with_openrouter(vec![entry(
            "anthropic/claude-opus-4.7",
            0.000_015,
            0.000_075,
        )]);
        assert!(resolve_cost(
            &settings,
            AgentProviderKind::Anthropic,
            "claude-experimental",
            Some(100),
            Some(50),
        )
        .is_none());
    }

    #[test]
    fn missing_tokens_still_prices_known_side() {
        let settings = settings_with_openrouter(vec![entry("openai/gpt-5", 0.000_002, 0.000_010)]);
        let cost = resolve_cost(
            &settings,
            AgentProviderKind::Openrouter,
            "openai/gpt-5",
            None,
            Some(1_000),
        )
        .expect("priced");
        assert!((cost - 0.01).abs() < 1e-9, "got {cost}");
    }

    #[test]
    fn both_tokens_none_returns_none_even_with_pricing() {
        let settings = settings_with_openrouter(vec![entry("openai/gpt-5", 0.000_002, 0.000_010)]);
        assert!(resolve_cost(
            &settings,
            AgentProviderKind::Openrouter,
            "openai/gpt-5",
            None,
            None,
        )
        .is_none());
    }

    #[test]
    fn direct_mapping_table_contains_known_ids() {
        assert_eq!(
            map_direct_to_openrouter(AgentProviderKind::Anthropic, "claude-opus-4-7"),
            Some("anthropic/claude-opus-4.7"),
        );
        assert_eq!(
            map_direct_to_openrouter(AgentProviderKind::Openai, "gpt-5"),
            Some("openai/gpt-5"),
        );
        assert_eq!(
            map_direct_to_openrouter(AgentProviderKind::Anthropic, "made-up-model"),
            None,
        );
    }
}
