pub(crate) mod protocol;
pub(crate) mod state;
pub mod tools;

mod anthropic;
mod environment;
mod git_agent;
mod openrouter;
mod provider;
mod session_orchestrator;
mod shell_exec;
mod subagent_prompts;
mod subagents;
mod system_prompt;
mod tool_dispatch;
mod tool_groups;
mod tools_extra;
mod web_tools;
mod workspace_agent;

pub use protocol::{AgentEvent, UserTurn};
pub use session_orchestrator::dispatch_user_turn;
pub use state::AgentEngineState;
