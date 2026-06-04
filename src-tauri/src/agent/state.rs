use crate::agent::protocol::{AgentChatMode, AgentEvent, EventEnvelope};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

pub const DEFAULT_AGENT_SESSION_ID: &str = "default";

#[derive(Debug, Default)]
pub struct AgentEngineRegistry {
    engines: Mutex<HashMap<String, Arc<AgentEngineState>>>,
}

impl AgentEngineRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            engines: Mutex::new(HashMap::new()),
        })
    }

    pub fn engine(&self, session_id: Option<String>) -> Arc<AgentEngineState> {
        let id = normalize_session_id(session_id);
        let mut engines = self.engines.lock().expect("agent registry lock poisoned");
        engines
            .entry(id)
            .or_insert_with(AgentEngineState::new)
            .clone()
    }

    pub fn engine_for_tool_result(
        &self,
        session_id: Option<String>,
        call_id: &str,
    ) -> Arc<AgentEngineState> {
        if session_id.as_ref().is_some_and(|id| !id.trim().is_empty()) {
            return self.engine(session_id);
        }
        let matched = {
            let engines = self.engines.lock().expect("agent registry lock poisoned");
            engines
                .values()
                .find(|engine| engine.has_pending_client_tool(call_id))
                .cloned()
        };
        matched.unwrap_or_else(|| self.engine(None))
    }
}

fn normalize_session_id(session_id: Option<String>) -> String {
    session_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| DEFAULT_AGENT_SESSION_ID.to_string())
}

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
    /// Optional mode change selected during the active turn. Used when the user
    /// approves a permission prompt with "Auto-accept" so subsequent tool calls
    /// in the same turn stop prompting immediately.
    chat_mode_override: Mutex<Option<AgentChatMode>>,
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
            chat_mode_override: Mutex::new(None),
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
        self.chat_mode_override
            .lock()
            .expect("chat mode override lock poisoned")
            .take();
    }

    pub fn set_chat_mode_override(&self, mode: AgentChatMode) {
        *self
            .chat_mode_override
            .lock()
            .expect("chat mode override lock poisoned") = Some(mode);
    }

    #[must_use]
    pub fn chat_mode_override(&self) -> Option<AgentChatMode> {
        *self
            .chat_mode_override
            .lock()
            .expect("chat mode override lock poisoned")
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

    pub fn has_pending_client_tool(&self, call_id: &str) -> bool {
        self.pending_client_tools
            .lock()
            .expect("pending tools lock poisoned")
            .contains_key(call_id)
    }
}

/// Optional env-based provider config (no network in stub).
#[derive(Clone, Debug)]
pub struct ProviderEnv {
    #[allow(dead_code)]
    pub anthropic_api_key: Option<String>,
}

impl ProviderEnv {
    // Env-based key path (BLX_ANTHROPIC_API_KEY); the live path reads keys from
    // agent settings instead (see CLAUDE.md). Kept as the documented env stub.
    #[allow(dead_code)]
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
    use crate::agent::protocol::{AgentChatMode, AgentEvent};

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

    #[test]
    fn chat_mode_override_resets_on_start_turn() {
        let state = AgentEngineState::new();
        state.set_chat_mode_override(AgentChatMode::AllowAll);
        assert_eq!(state.chat_mode_override(), Some(AgentChatMode::AllowAll));

        state.start_turn();
        assert_eq!(state.chat_mode_override(), None);
    }

    #[test]
    fn registry_keeps_session_events_isolated() {
        let registry = AgentEngineRegistry::new();
        let first = registry.engine(Some("one".to_owned()));
        let second = registry.engine(Some("two".to_owned()));

        first.push(AgentEvent::AssistantDelta {
            delta: "first".to_owned(),
        });
        second.push(AgentEvent::AssistantDelta {
            delta: "second".to_owned(),
        });

        let first_events = first.drain(10);
        let second_events = second.drain(10);
        assert_eq!(first_events.len(), 1);
        assert_eq!(second_events.len(), 1);
        assert!(matches!(
            &first_events[0].event,
            AgentEvent::AssistantDelta { delta } if delta == "first"
        ));
        assert!(matches!(
            &second_events[0].event,
            AgentEvent::AssistantDelta { delta } if delta == "second"
        ));
    }

    #[test]
    fn registry_routes_omitted_session_tool_result_to_pending_engine() {
        let registry = AgentEngineRegistry::new();
        let first = registry.engine(Some("one".to_owned()));
        let second = registry.engine(Some("two".to_owned()));
        let (_tx_first, mut rx_first) = oneshot::channel();
        let (tx_second, mut rx_second) = oneshot::channel();
        first.register_client_tool("call-one".to_owned(), _tx_first);
        second.register_client_tool("call-two".to_owned(), tx_second);

        let routed = registry.engine_for_tool_result(None, "call-two");
        routed
            .deliver_client_tool_result("call-two", true, Some("ok".to_owned()), None)
            .expect("tool result delivered");

        assert!(first.has_pending_client_tool("call-one"));
        assert!(rx_first.try_recv().is_err());
        let delivered = rx_second.try_recv().expect("second receiver gets result");
        assert!(delivered.ok);
        assert_eq!(delivered.message.as_deref(), Some("ok"));
    }
}
