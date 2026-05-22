//! Entspricht `src-tauri/src/agent/protocol.rs` (Serde-JSON zwischen Engine und Harness).
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTurn {
    pub prompt: String,
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub voice_input: bool,
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
    PlanIndex,
    PlanFile,
    PlanTaskGroup,
    /// Inline file snippet (line range) attached from the file preview's
    /// right-click menu. `content` carries the fenced markdown block.
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Blocked,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub position: u32,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub parent_id: Option<String>,
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_task_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub tasks: Vec<AgentTask>,
    pub active_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_plan_path: Option<String>,
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
        #[serde(default)]
        call_id: Option<String>,
        #[serde(default)]
        args: Option<Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool: String,
        ok: bool,
        #[serde(default)]
        message: Option<String>,
    },
    #[serde(rename = "task_snapshot")]
    TaskSnapshot { snapshot: TaskSnapshot },
    #[serde(rename = "image_context_consumed")]
    ImageContextConsumed { ids: Vec<String> },
    #[serde(rename = "image_generated")]
    ImageGenerated {
        prompt: String,
        mime: String,
        #[serde(default)]
        saved_path: Option<String>,
        #[serde(default)]
        filename: Option<String>,
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
        #[serde(default)]
        note: Option<String>,
    },
    #[serde(rename = "subagent_tool_call")]
    SubagentToolCall {
        agent_id: String,
        tool: String,
        #[serde(default)]
        call_id: Option<String>,
        #[serde(default)]
        args: Option<Value>,
    },
    #[serde(rename = "subagent_assistant_delta")]
    SubagentAssistantDelta { agent_id: String, delta: String },
    #[serde(rename = "subagent_thinking_delta")]
    SubagentThinkingDelta { agent_id: String, delta: String },
    #[serde(rename = "subagent_thinking_done")]
    SubagentThinkingDone { agent_id: String },
    #[serde(rename = "subagent_finished")]
    SubagentFinished {
        agent_id: String,
        status: String,
        summary: String,
    },
    #[serde(rename = "turn_usage")]
    TurnUsage {
        kind: TurnUsageKind,
        #[serde(default)]
        agent_id: Option<String>,
        #[serde(default)]
        call_id: Option<String>,
        #[serde(default)]
        round_index: Option<u32>,
        #[serde(default)]
        turn_generation: u64,
        #[serde(default)]
        input_tokens: Option<u64>,
        #[serde(default)]
        output_tokens: Option<u64>,
        #[serde(default)]
        ttft_ms: Option<u64>,
        elapsed_ms: u64,
        #[serde(default)]
        cost_usd: Option<f64>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnUsageKind {
    ModelRound,
    ToolExec,
}

/// Per-row metric bundle (mirror of backend `TurnMetrics`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnMetrics {
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub ttft_ms: Option<u64>,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default)]
    pub cost_usd: Option<f64>,
}

impl TurnMetrics {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.ttft_ms.is_none()
            && self.elapsed_ms == 0
            && self.cost_usd.is_none()
    }

    pub fn merge(&mut self, other: &TurnMetrics) {
        if let Some(v) = other.input_tokens {
            self.input_tokens = Some(self.input_tokens.unwrap_or(0).saturating_add(v));
        }
        if let Some(v) = other.output_tokens {
            self.output_tokens = Some(self.output_tokens.unwrap_or(0).saturating_add(v));
        }
        if let Some(v) = other.ttft_ms {
            self.ttft_ms.get_or_insert(v);
        }
        self.elapsed_ms = self.elapsed_ms.saturating_add(other.elapsed_ms);
        if let Some(v) = other.cost_usd {
            self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + v);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBoundsPayload {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub visible: bool,
}
