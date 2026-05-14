//! Entspricht `src-tauri/src/agent/protocol.rs` (Serde-JSON zwischen Engine und Harness).
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTurn {
    pub prompt: String,
    pub workspace_root: Option<String>,
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
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "error")]
    Error { message: String },
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
