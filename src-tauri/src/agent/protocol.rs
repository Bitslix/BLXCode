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
    /// Per-turn usage stats emitted just before `Done`. The UI accumulates
    /// these across the conversation and renders a footer line with
    /// totals, decode speed (output_tokens/s) and time-to-first-token.
    #[serde(rename = "turn_usage")]
    TurnUsage {
        /// Tokens billed for the prompt (system + history + this user turn,
        /// including any tool results). When the provider doesn't emit
        /// usage, this is `None` and the UI shows `—`.
        #[serde(skip_serializing_if = "Option::is_none")]
        input_tokens: Option<u64>,
        /// Tokens billed for the model's output across all rounds of this
        /// turn.
        #[serde(skip_serializing_if = "Option::is_none")]
        output_tokens: Option<u64>,
        /// Wall-clock milliseconds from request send to first streamed
        /// content/thinking delta of the first round.
        #[serde(skip_serializing_if = "Option::is_none")]
        ttft_ms: Option<u64>,
        /// Wall-clock milliseconds for the whole turn (all rounds, all
        /// tool calls, until `Done`).
        elapsed_ms: u64,
    },
}
