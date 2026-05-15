//! Pi-inspirierend: später austauschbarer HTTP-/LLM-Provider; Stub.

use crate::agent::state::ProviderEnv;
use serde_json::Value;

/// Abstraktion für kommende Inference-Backend-Implementierungen (Stub).
#[allow(dead_code)]
pub trait InferenceProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn status_json(&self) -> Value;
}

impl InferenceProvider for ProviderEnv {
    fn id(&self) -> &'static str {
        if self.anthropic_api_key.is_some() {
            "anthropic_stub"
        } else {
            "mock"
        }
    }

    fn status_json(&self) -> Value {
        ProviderEnv::status_json(self)
    }
}

#[allow(dead_code)]
pub(crate) fn current_provider_stub() -> impl InferenceProvider {
    ProviderEnv::from_environment()
}
