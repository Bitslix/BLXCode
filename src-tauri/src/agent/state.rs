use crate::agent::protocol::{AgentEvent, EventEnvelope};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    events: Mutex<VecDeque<EventEnvelope>>,
    busy: AtomicBool,
    cancel: AtomicBool,
    next_seq: AtomicU64,
    parent_stack: Mutex<Vec<String>>,
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
            next_seq: AtomicU64::new(0),
            parent_stack: Mutex::new(Vec::new()),
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

    pub fn start_turn(&self) {
        self.next_seq.store(0, Ordering::SeqCst);
        self.parent_stack
            .lock()
            .expect("parent stack lock poisoned")
            .clear();
    }

    pub fn push_parent(&self, call_id: String) {
        self.parent_stack
            .lock()
            .expect("parent stack lock poisoned")
            .push(call_id);
    }

    pub fn pop_parent(&self) {
        let _ = self
            .parent_stack
            .lock()
            .expect("parent stack lock poisoned")
            .pop();
    }

    pub fn push_batch(&self, evs: impl IntoIterator<Item = AgentEvent>) {
        let mut q = self.events.lock().expect("agent queue lock poisoned");
        for e in evs {
            let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
            let parent_call_id = self
                .parent_stack
                .lock()
                .expect("parent stack lock poisoned")
                .last()
                .cloned();
            q.push_back(EventEnvelope {
                seq,
                parent_call_id,
                event: e,
            });
        }
    }

    pub fn push(&self, ev: AgentEvent) {
        self.push_batch(std::iter::once(ev));
    }

    pub fn drain(&self, max: usize) -> Vec<EventEnvelope> {
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::protocol::AgentEvent;

    #[test]
    fn seq_monotonic_per_turn() {
        let state = AgentEngineState::new();
        state.start_turn();
        for idx in 0..100 {
            state.push(AgentEvent::AssistantDelta {
                delta: idx.to_string(),
            });
        }
        let events = state.drain(200);
        assert_eq!(events.len(), 100);
        for (idx, env) in events.iter().enumerate() {
            assert_eq!(env.seq, idx as u64);
        }

        state.start_turn();
        state.push(AgentEvent::AssistantDelta {
            delta: "next".to_owned(),
        });
        let events = state.drain(1);
        assert_eq!(events[0].seq, 0);
    }

    #[test]
    fn pushed_events_inherit_current_parent_call_id() {
        let state = AgentEngineState::new();
        state.start_turn();
        state.push(AgentEvent::ToolCall {
            tool: "subagents.run".to_owned(),
            call_id: Some("cid-outer".to_owned()),
            args: None,
        });
        state.push_parent("cid-outer".to_owned());
        state.push(AgentEvent::SubagentStarted {
            agent_id: "sa-1".to_owned(),
            role: "scout".to_owned(),
            display_name: "Scout".to_owned(),
        });
        state.push(AgentEvent::SubagentToolCall {
            agent_id: "sa-1".to_owned(),
            tool: "read_workspace_file".to_owned(),
            call_id: Some("cid-inner".to_owned()),
            args: None,
        });
        state.pop_parent();

        let events = state.drain(10);
        assert_eq!(events[0].parent_call_id, None);
        assert_eq!(events[1].parent_call_id.as_deref(), Some("cid-outer"));
        assert_eq!(events[2].parent_call_id.as_deref(), Some("cid-outer"));
    }
}
