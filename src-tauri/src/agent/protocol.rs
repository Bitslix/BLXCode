//! JSON-serialisierbare Events zwischen Agent-Engine (Tauri) und Harness (Webview).
use crate::tasks::TaskSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTurn {
    pub prompt: String,
    /// Sandbox root for read-only tools; must be canonical if set (caller responsibility).
    pub workspace_root: Option<String>,
    /// When true, the orchestrator runs the configured TTS engine on the
    /// final assistant text and emits an `AgentEvent::VoiceReady`.
    #[serde(default)]
    pub voice_input: bool,
    /// When true, the orchestrator runs the configured image provider on
    /// the prompt (plus any `image_context_items` as references) instead of
    /// dispatching a text-agent turn.
    #[serde(default)]
    pub image_generate: bool,
    #[serde(default)]
    pub context_items: Vec<AgentContextItem>,
    #[serde(default)]
    pub image_context_items: Vec<AgentImageContextItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentContextKind {
    MemoryCategory,
    LearningCategory,
    MemoryNote,
    LearningNote,
    TerminalSession,
    /// Aggregate pointer to `PLANS.md` (plans index).
    PlanIndex,
    /// One specific plan Markdown file in `.agents/plans/`.
    PlanFile,
    /// Plan-linked task subset (no on-disk file).
    PlanTaskGroup,
    /// Inline snippet of a file (line range) attached from the file preview's
    /// right-click context menu. The `content` field carries the fenced
    /// markdown block; `paths` holds the workspace-relative file path.
    FileSnippet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextItem {
    pub id: String,
    pub kind: AgentContextKind,
    pub label: String,
    pub source: String,
    pub paths: Vec<String>,
    pub added_at: i64,
    /// Inline content for items that ship their payload alongside the
    /// metadata (currently only `FileSnippet`). Older items lack this field,
    /// so it must be optional and skipped on serialization when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentImageContextItem {
    pub id: String,
    pub label: String,
    pub mime: String,
    pub bytes_b64: String,
    pub size_bytes: u64,
    pub added_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all_fields = "camelCase",
    rename_all = "camelCase"
)]
pub enum AgentEvent {
    #[serde(rename = "assistant_delta")]
    AssistantDelta { delta: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { delta: String },
    #[serde(rename = "thinking_done")]
    ThinkingDone,
    #[serde(rename = "tool_call")]
    ToolCall {
        tool: String,
        /// Provider-issued call id. Required for client-side tools so the
        /// UI can route results back via `agent_submit_tool_result`.
        /// Optional for legacy / mock events.
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    #[serde(rename = "task_snapshot")]
    TaskSnapshot { snapshot: TaskSnapshot },
    #[serde(rename = "image_context_consumed")]
    ImageContextConsumed { ids: Vec<String> },
    /// Emitted after a successful image-generation turn. `savedPath` is the
    /// absolute file path under `.blxcode/generated/` when a workspace root
    /// was provided; otherwise the image lives only in-memory and the
    /// `previewSrc` data URL carries the bytes.
    #[serde(rename = "image_generated")]
    ImageGenerated {
        prompt: String,
        mime: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        saved_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
        /// Data URL (`data:<mime>;base64,...`) suitable for `<img src>`
        /// directly. Always present so the UI can render immediately.
        preview_src: String,
    },
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "voice_ready")]
    VoiceReady { audio_b64: String, mime: String },
    #[serde(rename = "subagent_started")]
    SubagentStarted {
        agent_id: String,
        role: String,
        display_name: String,
    },
    #[serde(rename = "subagent_step")]
    SubagentStep {
        agent_id: String,
        step_id: String,
        title: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    #[serde(rename = "subagent_tool_call")]
    SubagentToolCall {
        agent_id: String,
        tool: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
    },
    /// Streamed assistant text from a subagent's current round. The UI
    /// appends successive deltas to a per-agent buffer so the operator can
    /// watch the subagent's reasoning in real time.
    #[serde(rename = "subagent_assistant_delta")]
    SubagentAssistantDelta { agent_id: String, delta: String },
    /// Streamed reasoning text from a subagent (provider's "thinking" /
    /// "reasoning" channel). Treated the same as `AssistantDelta` but kept
    /// separate so the UI can style it as collapsed thinking.
    #[serde(rename = "subagent_thinking_delta")]
    SubagentThinkingDelta { agent_id: String, delta: String },
    /// Marks the end of a thinking burst so the UI can collapse the
    /// thinking block once the subagent moves on to tool calls or text.
    #[serde(rename = "subagent_thinking_done")]
    SubagentThinkingDone { agent_id: String },
    #[serde(rename = "subagent_finished")]
    SubagentFinished {
        agent_id: String,
        status: String,
        summary: String,
    },
    /// Per-round / per-tool usage stats. One event per `ModelRound`
    /// (provider call) and one per `ToolExec` (server tool dispatch).
    /// The UI attaches metrics to the matching timeline row and updates
    /// the session cost total in the chat header.
    #[serde(rename = "turn_usage")]
    TurnUsage {
        /// Whether this event reports a provider round or a tool execution.
        kind: TurnUsageKind,
        /// `None` for the main agent; `Some(id)` for events emitted from
        /// inside a subagent run (routed to the subagent card).
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
        /// Provider call id for `ToolExec` events — used to correlate
        /// the metric back to the originating tool row.
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        /// Zero-based round index within this user turn for `ModelRound`.
        #[serde(skip_serializing_if = "Option::is_none")]
        round_index: Option<u32>,
        /// Monotonic counter incremented on `agent_clear_conversation`.
        /// The frontend drops `TurnUsage` events whose generation is
        /// older than the current one to avoid late events from a
        /// cancelled turn polluting a fresh chat.
        turn_generation: u64,
        /// Prompt / input tokens reported for this round (or this tool's
        /// internal usage, if applicable). `None` when not available.
        #[serde(skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
        /// Completion / output tokens reported for this round.
        #[serde(skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
        /// Wall-clock ms from round start to first streamed delta.
        /// `None` for tool-only rounds or `ToolExec` events.
        #[serde(skip_serializing_if = "Option::is_none")]
        ttft_ms: Option<u64>,
        /// Wall-clock ms spent in this round or tool execution.
        elapsed_ms: u64,
        /// Resolved USD cost for this round / tool. `None` when no
        /// pricing data is available; the UI then shows `—`.
        #[serde(skip_serializing_if = "Option::is_none")]
        cost_usd: Option<f64>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnUsageKind {
    /// One provider round (one HTTP request → streamed reply, possibly
    /// followed by tool calls in the next round).
    ModelRound,
    /// One in-process tool dispatch invoked by the model.
    ToolExec,
}

/// Per-row metric bundle attached to assistant / tool / subagent rows in
/// the timeline. Mirrored on the frontend in `agent_wire.rs` — the
/// active consumer lives there, so the Tauri-side definition exists only
/// as a wire-format spec / future-server hook.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u64>,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

#[allow(dead_code)]
impl TurnMetrics {
    /// Returns `true` when no field carries meaningful data — the UI uses
    /// this to skip rendering an empty metrics bar.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.ttft_ms.is_none()
            && self.elapsed_ms == 0
            && self.cost_usd.is_none()
    }

    /// Accumulate another metric burst into this one (used for subagent
    /// cards where multiple `ModelRound` events fold into one header).
    pub fn merge(&mut self, other: &TurnMetrics) {
        if let Some(v) = other.input_tokens {
            self.input_tokens = Some(self.input_tokens.unwrap_or(0).saturating_add(v));
        }
        if let Some(v) = other.output_tokens {
            self.output_tokens = Some(self.output_tokens.unwrap_or(0).saturating_add(v));
        }
        if let Some(v) = other.ttft_ms {
            // For aggregated metrics we keep the first TTFT sample —
            // averaging across rounds would mislead.
            self.ttft_ms.get_or_insert(v);
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(other.elapsed_ms);
        if let Some(v) = other.cost_usd {
            self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + v);
        }
    }
}
