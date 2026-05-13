//! Pi-inspirierend: später austauschbarer HTTP-/LLM-Provider; Stub + optionaler Reachability‑Check.

use crate::agent::protocol::AgentEvent;
use crate::agent::state::{AgentEngineState, ProviderEnv};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

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

/// Optional TLS/HTTP ohne API‑Key‑Payload wenn `BLX_ANTHROPIC_API_KEY` gesetzt ist.
pub(crate) async fn maybe_emit_network_hint(agent: Arc<AgentEngineState>) {
    if ProviderEnv::from_environment().anthropic_api_key.is_none() {
        return;
    }
    let hint = outbound_probe_summary().await;
    agent.push(AgentEvent::AssistantDelta {
        delta: format!("\n{hint}"),
    });
}

async fn outbound_probe_summary() -> String {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("(Provider‑HTTP) reqwest konnte nicht gebaut werden: {e}"),
    };
    match client.head("https://api.anthropic.com/v1/").send().await {
        Ok(resp) => {
            format!(
                "(Provider‑HTTP) Erreichbarkeit api.anthropic.com → HTTP {}",
                resp.status().as_u16()
            )
        }
        Err(e) => format!("(Provider‑HTTP) Verbindungsfehler: {e}"),
    }
}
