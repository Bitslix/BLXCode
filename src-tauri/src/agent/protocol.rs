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
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "voice_ready")]
    VoiceReady { audio_b64: String, mime: String },
}
