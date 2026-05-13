use crate::agent::protocol::AgentEvent;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct AgentEngineState {
    events: Mutex<VecDeque<AgentEvent>>,
    busy: AtomicBool,
    cancel: AtomicBool,
}

impl AgentEngineState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            events: Mutex::new(VecDeque::new()),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
        })
    }

    pub fn push_batch(&self, evs: impl IntoIterator<Item = AgentEvent>) {
        let mut q = self.events.lock().expect("agent queue lock poisoned");
        for e in evs {
            q.push_back(e);
        }
    }

    pub fn push(&self, ev: AgentEvent) {
        self.push_batch(std::iter::once(ev));
    }

    pub fn drain(&self, max: usize) -> Vec<AgentEvent> {
        let mut q = self.events.lock().expect("agent queue lock poisoned");
        let mut out = Vec::new();
        for _ in 0..max {
            if let Some(e) = q.pop_front() {
                out.push(e);
            } else {
                break;
            }
        }
        out
    }

    pub fn set_busy(&self, v: bool) {
        self.busy.store(v, Ordering::SeqCst);
    }

    #[must_use]
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn clear_cancel(&self) {
        self.cancel.store(false, Ordering::SeqCst);
    }
}

/// Optional env-based provider config (no network in stub).
#[derive(Clone, Debug)]
pub struct ProviderEnv {
    pub anthropic_api_key: Option<String>,
}

impl ProviderEnv {
    pub fn from_environment() -> Self {
        Self {
            anthropic_api_key: std::env::var("BLX_ANTHROPIC_API_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
        }
    }

    /// Returns JSON for settings UI (secrets redacted).
    pub fn status_json(&self) -> Value {
        let provider = if self.anthropic_api_key.is_some() {
            "anthropic_env_configured_stub"
        } else {
            "mock_local"
        };

        serde_json::json!({
            "phase": "mock_engine",
            "provider": provider,
            "anthropicConfigured": self.anthropic_api_key.is_some(),
        })
    }
}
