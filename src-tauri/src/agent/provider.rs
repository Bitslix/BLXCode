//! Central registry for BLXCode text-agent providers.
//!
//! Anthropic keeps its native Messages API loop. Every other provider in the
//! registry is treated as OpenAI Chat Completions compatible and resolved into
//! a concrete endpoint/auth configuration at the call site.

use crate::agent_settings::{AgentProviderKind, AgentProviderSettings, ProviderModelEntry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderClass {
    Local,
    Cloud,
    Gateway,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMode {
    None,
    RequiredBearer,
    #[allow(dead_code)]
    OptionalBearer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelDiscovery {
    Anthropic,
    OpenRouter,
    OpenAiCompatible,
    Cloudflare,
}

#[derive(Clone, Copy, Debug)]
pub struct ProviderSpec {
    pub kind: AgentProviderKind,
    pub id: &'static str,
    pub label: &'static str,
    pub class: ProviderClass,
    pub default_base_url: &'static str,
    pub auth_mode: AuthMode,
    pub model_discovery: ModelDiscovery,
    pub supports_openai_chat: bool,
    pub supports_openai_reasoning_effort: bool,
    pub sends_openrouter_extras: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibleEndpoint {
    pub provider: AgentProviderKind,
    pub url: String,
    pub auth_mode: AuthMode,
    pub supports_openai_reasoning_effort: bool,
    pub sends_openrouter_extras: bool,
}

pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        kind: AgentProviderKind::Openrouter,
        id: "openrouter",
        label: "OpenRouter",
        class: ProviderClass::Cloud,
        default_base_url: "https://openrouter.ai/api/v1",
        auth_mode: AuthMode::RequiredBearer,
        model_discovery: ModelDiscovery::OpenRouter,
        supports_openai_chat: true,
        supports_openai_reasoning_effort: false,
        sends_openrouter_extras: true,
    },
    ProviderSpec {
        kind: AgentProviderKind::Anthropic,
        id: "anthropic",
        label: "Anthropic",
        class: ProviderClass::Cloud,
        default_base_url: "https://api.anthropic.com/v1",
        auth_mode: AuthMode::RequiredBearer,
        model_discovery: ModelDiscovery::Anthropic,
        supports_openai_chat: false,
        supports_openai_reasoning_effort: false,
        sends_openrouter_extras: false,
    },
    ProviderSpec {
        kind: AgentProviderKind::Openai,
        id: "openai",
        label: "OpenAI",
        class: ProviderClass::Cloud,
        default_base_url: "https://api.openai.com/v1",
        auth_mode: AuthMode::RequiredBearer,
        model_discovery: ModelDiscovery::OpenAiCompatible,
        supports_openai_chat: true,
        supports_openai_reasoning_effort: true,
        sends_openrouter_extras: false,
    },
    ProviderSpec {
        kind: AgentProviderKind::Ollama,
        id: "ollama",
        label: "Ollama",
        class: ProviderClass::Local,
        default_base_url: "http://localhost:11434/v1",
        auth_mode: AuthMode::None,
        model_discovery: ModelDiscovery::OpenAiCompatible,
        supports_openai_chat: true,
        supports_openai_reasoning_effort: false,
        sends_openrouter_extras: false,
    },
    ProviderSpec {
        kind: AgentProviderKind::LmStudio,
        id: "lmStudio",
        label: "LM Studio",
        class: ProviderClass::Local,
        default_base_url: "http://localhost:1234/v1",
        auth_mode: AuthMode::None,
        model_discovery: ModelDiscovery::OpenAiCompatible,
        supports_openai_chat: true,
        supports_openai_reasoning_effort: false,
        sends_openrouter_extras: false,
    },
    ProviderSpec {
        kind: AgentProviderKind::HuggingFace,
        id: "huggingFace",
        label: "Hugging Face",
        class: ProviderClass::Cloud,
        default_base_url: "https://router.huggingface.co/v1",
        auth_mode: AuthMode::RequiredBearer,
        model_discovery: ModelDiscovery::OpenAiCompatible,
        supports_openai_chat: true,
        supports_openai_reasoning_effort: false,
        sends_openrouter_extras: false,
    },
    ProviderSpec {
        kind: AgentProviderKind::Cloudflare,
        id: "cloudflare",
        label: "Cloudflare Workers AI",
        class: ProviderClass::Cloud,
        default_base_url: "https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1",
        auth_mode: AuthMode::RequiredBearer,
        model_discovery: ModelDiscovery::Cloudflare,
        supports_openai_chat: true,
        supports_openai_reasoning_effort: false,
        sends_openrouter_extras: false,
    },
    ProviderSpec {
        kind: AgentProviderKind::Together,
        id: "together",
        label: "Together AI",
        class: ProviderClass::Cloud,
        default_base_url: "https://api.together.ai/v1",
        auth_mode: AuthMode::RequiredBearer,
        model_discovery: ModelDiscovery::OpenAiCompatible,
        supports_openai_chat: true,
        supports_openai_reasoning_effort: false,
        sends_openrouter_extras: false,
    },
    ProviderSpec {
        kind: AgentProviderKind::Portkey,
        id: "portkey",
        label: "Portkey",
        class: ProviderClass::Gateway,
        default_base_url: "https://api.portkey.ai/v1",
        auth_mode: AuthMode::RequiredBearer,
        model_discovery: ModelDiscovery::OpenAiCompatible,
        supports_openai_chat: true,
        supports_openai_reasoning_effort: false,
        sends_openrouter_extras: false,
    },
];

pub fn all_providers() -> &'static [ProviderSpec] {
    PROVIDERS
}

pub fn spec(provider: AgentProviderKind) -> &'static ProviderSpec {
    PROVIDERS
        .iter()
        .find(|entry| entry.kind == provider)
        .expect("every AgentProviderKind has a ProviderSpec")
}

pub fn provider_requires_key(provider: AgentProviderKind) -> bool {
    matches!(spec(provider).auth_mode, AuthMode::RequiredBearer)
}

pub fn class_label(class: ProviderClass) -> &'static str {
    match class {
        ProviderClass::Local => "local",
        ProviderClass::Cloud => "cloud",
        ProviderClass::Gateway => "gateway",
    }
}

pub fn compatible_endpoint(settings: &AgentProviderSettings) -> Result<CompatibleEndpoint, String> {
    let provider = settings.provider;
    let spec = spec(provider);
    if !spec.supports_openai_chat {
        return Err(format!(
            "{} does not use the OpenAI-compatible loop",
            spec.label
        ));
    }
    let base_url = settings
        .base_url_for_provider(provider)
        .unwrap_or_else(|| spec.default_base_url.to_string());
    let base_url = if provider == AgentProviderKind::Cloudflare {
        let account_id = settings.cloudflare_account_id.trim();
        if account_id.is_empty() {
            return Err("Cloudflare Account ID is required for Workers AI.".into());
        }
        base_url.replace("{account_id}", account_id)
    } else {
        base_url
    };
    Ok(CompatibleEndpoint {
        provider,
        url: chat_completions_url(&base_url),
        auth_mode: spec.auth_mode,
        supports_openai_reasoning_effort: spec.supports_openai_reasoning_effort,
        sends_openrouter_extras: spec.sends_openrouter_extras,
    })
}

pub fn models_url(
    settings: &AgentProviderSettings,
    provider: AgentProviderKind,
) -> Result<String, String> {
    let spec = spec(provider);
    let base_url = settings
        .base_url_for_provider(provider)
        .unwrap_or_else(|| spec.default_base_url.to_string());
    let base_url = if provider == AgentProviderKind::Cloudflare {
        let account_id = settings.cloudflare_account_id.trim();
        if account_id.is_empty() {
            return Err("Cloudflare Account ID is required to refresh Workers AI models.".into());
        }
        base_url.replace("{account_id}", account_id)
    } else {
        base_url
    };
    Ok(match spec.model_discovery {
        ModelDiscovery::Anthropic
        | ModelDiscovery::OpenRouter
        | ModelDiscovery::OpenAiCompatible => {
            format!("{}/models", base_url.trim_end_matches('/'))
        }
        ModelDiscovery::Cloudflare => format!("{}/models", base_url.trim_end_matches('/')),
    })
}

pub fn curated_models(provider: AgentProviderKind) -> Vec<ProviderModelEntry> {
    let entry = |id: &str, label: &str, description: &str| ProviderModelEntry {
        id: id.into(),
        label: label.into(),
        description: Some(description.into()),
        pricing: None,
        context_length: None,
    };
    match provider {
        AgentProviderKind::Openrouter => vec![
            entry("openai/gpt-5", "GPT-5", "Default via OpenRouter"),
            entry(
                "anthropic/claude-sonnet-4.5",
                "Claude Sonnet 4.5",
                "Anthropic via OpenRouter",
            ),
            entry(
                "google/gemini-2.5-pro",
                "Gemini 2.5 Pro",
                "Google via OpenRouter",
            ),
        ],
        AgentProviderKind::Anthropic => vec![
            entry("claude-sonnet-4-5", "Claude Sonnet 4.5", "Balanced model"),
            entry("claude-opus-4-1", "Claude Opus 4.1", "Highest capability"),
        ],
        AgentProviderKind::Openai => vec![
            entry("gpt-5", "GPT-5", "Reasoning flagship"),
            entry("gpt-5-mini", "GPT-5 Mini", "Faster/cost-lean variant"),
        ],
        AgentProviderKind::Ollama => vec![
            entry("llama3.1", "Llama 3.1", "Local Ollama model"),
            entry("qwen2.5-coder", "Qwen2.5 Coder", "Local coding model"),
        ],
        AgentProviderKind::LmStudio => vec![entry(
            "local-model",
            "Local model",
            "Currently loaded LM Studio model",
        )],
        AgentProviderKind::HuggingFace => vec![
            entry(
                "meta-llama/Llama-3.1-8B-Instruct",
                "Llama 3.1 8B Instruct",
                "Hugging Face router",
            ),
            entry(
                "Qwen/Qwen2.5-Coder-32B-Instruct",
                "Qwen2.5 Coder 32B",
                "Hugging Face router",
            ),
        ],
        AgentProviderKind::Cloudflare => vec![
            entry(
                "@cf/meta/llama-3.1-8b-instruct",
                "Llama 3.1 8B Instruct",
                "Workers AI",
            ),
            entry(
                "@cf/qwen/qwen1.5-14b-chat-awq",
                "Qwen 1.5 14B Chat",
                "Workers AI",
            ),
        ],
        AgentProviderKind::Together => vec![
            entry(
                "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo",
                "Llama 3.1 8B Turbo",
                "Together AI",
            ),
            entry(
                "Qwen/Qwen2.5-Coder-32B-Instruct",
                "Qwen2.5 Coder 32B",
                "Together AI",
            ),
        ],
        AgentProviderKind::Portkey => vec![entry(
            "gpt-5",
            "GPT-5",
            "Gateway model ID configured in Portkey",
        )],
    }
}

fn chat_completions_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_settings::AgentProviderSettings;

    #[test]
    fn ollama_endpoint_uses_no_auth_default_url() {
        let settings = AgentProviderSettings {
            provider: AgentProviderKind::Ollama,
            ..AgentProviderSettings::default()
        };

        let endpoint = compatible_endpoint(&settings).expect("ollama endpoint");

        assert_eq!(endpoint.auth_mode, AuthMode::None);
        assert_eq!(endpoint.url, "http://localhost:11434/v1/chat/completions");
    }

    #[test]
    fn cloudflare_endpoint_requires_account_id() {
        let settings = AgentProviderSettings {
            provider: AgentProviderKind::Cloudflare,
            ..AgentProviderSettings::default()
        };

        let err = compatible_endpoint(&settings).expect_err("missing account id");

        assert!(err.contains("Account ID"));
    }

    #[test]
    fn cloudflare_endpoint_injects_account_id() {
        let settings = AgentProviderSettings {
            provider: AgentProviderKind::Cloudflare,
            cloudflare_account_id: "acct_123".into(),
            ..AgentProviderSettings::default()
        };

        let endpoint = compatible_endpoint(&settings).expect("cloudflare endpoint");

        assert_eq!(
            endpoint.url,
            "https://api.cloudflare.com/client/v4/accounts/acct_123/ai/v1/chat/completions"
        );
    }

    #[test]
    fn every_provider_has_curated_models() {
        for spec in all_providers() {
            assert!(
                !curated_models(spec.kind).is_empty(),
                "{} should have fallback models",
                spec.label
            );
        }
    }
}
