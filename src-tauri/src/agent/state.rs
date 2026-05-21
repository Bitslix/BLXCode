use crate::agent::protocol::AgentEvent;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// Result emitted by a UI-side tool back into the running turn.
#[derive(Clone, Debug)]
pub struct ClientToolResult {
    pub ok: bool,
    pub message: Option<String>,
    pub data: Option<Value>,
}

#[derive(Debug)]
pub struct AgentEngineState {
    events: Mutex<VecDeque<AgentEvent>>,
    busy: AtomicBool,
    cancel: AtomicBool,
    /// Senders keyed by tool-call id; the agent loop awaits the matching
    /// `oneshot` after emitting a client-side `ToolCall`.
    pending_client_tools: Mutex<HashMap<String, oneshot::Sender<ClientToolResult>>>,
    /// Conversation history across user turns (non-system messages only).
    /// The system prompt is rebuilt fresh per turn from the current workspace.
    conversation: Mutex<Vec<Value>>,
    /// Monotonic counter bumped each time the conversation is cleared.
    /// Stamped onto every `TurnUsage` event so the frontend can drop
    /// late events from a cancelled / cleared turn instead of polluting
    /// the next chat's running totals.
    turn_generation: std::sync::atomic::AtomicU64,
}

impl AgentEngineState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            events: Mutex::new(VecDeque::new()),
            busy: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            pending_client_tools: Mutex::new(HashMap::new()),
            conversation: Mutex::new(Vec::new()),
            turn_generation: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// Current generation. Stamp this onto every `TurnUsage` event the
    /// agent loop emits so the frontend can drop stale ones.
    pub fn turn_generation(&self) -> u64 {
        self.turn_generation
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Returns the persisted conversation (non-system messages) so the next
    /// turn can resume from prior context.
    pub fn conversation_snapshot(&self) -> Vec<Value> {
        self.conversation
            .lock()
            .expect("conversation lock poisoned")
            .clone()
    }

    /// Overwrites the persisted conversation with the latest non-system
    /// messages after a turn finishes.
    pub fn set_conversation(&self, msgs: Vec<Value>) {
        let mut g = self
            .conversation
            .lock()
            .expect("conversation lock poisoned");
        *g = msgs;
    }

    pub fn clear_conversation(&self) {
        self.conversation
            .lock()
            .expect("conversation lock poisoned")
            .clear();
        // Bump the generation so any TurnUsage events still in flight
        // from the prior turn are recognised as stale by the frontend.
        self.turn_generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
        // Drop all pending oneshots so any awaiting loop unblocks.
        let mut map = self
            .pending_client_tools
            .lock()
            .expect("pending tools lock poisoned");
        map.clear();
    }

    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn clear_cancel(&self) {
        self.cancel.store(false, Ordering::SeqCst);
    }

    /// Register a `oneshot::Sender` keyed by `call_id`; the agent loop
    /// awaits the receiver while the UI executes the tool.
    pub fn register_client_tool(&self, call_id: String, tx: oneshot::Sender<ClientToolResult>) {
        let mut map = self
            .pending_client_tools
            .lock()
            .expect("pending tools lock poisoned");
        map.insert(call_id, tx);
    }

    /// Frontend → backend bridge for `agent_submit_tool_result`. Returns
    /// `Err` when no matching pending call exists (turn already ended,
    /// duplicate submit, etc.).
    pub fn deliver_client_tool_result(
        &self,
        call_id: &str,
        ok: bool,
        message: Option<String>,
        data: Option<Value>,
    ) -> Result<(), String> {
        let tx_opt = {
            let mut map = self
                .pending_client_tools
                .lock()
                .expect("pending tools lock poisoned");
            map.remove(call_id)
        };
        let tx = tx_opt.ok_or_else(|| format!("no pending tool call {call_id}"))?;
        tx.send(ClientToolResult { ok, message, data })
            .map_err(|_| "tool result receiver dropped".to_owned())
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
