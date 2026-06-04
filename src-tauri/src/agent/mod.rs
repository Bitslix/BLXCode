pub(crate) mod protocol;
pub(crate) mod state;
pub mod tools;

mod anthropic;
pub(crate) mod badwords;
pub(crate) mod compaction;
pub(crate) mod context_window;
mod environment;
mod git_agent;
pub mod mermaid;
pub(crate) mod nickname;
pub(crate) mod oneshot;
mod openrouter;
pub mod plan_ai;
pub(crate) mod pricing;
mod project_docs;
pub mod prompt_enhance;
pub(crate) mod provider;
mod session_orchestrator;
pub(crate) mod session_roles;
mod shell_exec;
mod subagent_prompts;
mod subagent_runner;
mod subagents;
mod system_prompt;
mod tool_dispatch;
mod tool_groups;
mod tools_extra;
mod web_commands;
pub(crate) mod web_settings;
mod web_tools;
mod workspace_agent;

pub use compaction::agent_compact_conversation;
pub use web_commands::{
    agent_environment_invalidate, agent_web_settings_get, agent_web_settings_save,
};

pub use protocol::{EventEnvelope, UserTurn};
pub use session_orchestrator::dispatch_user_turn;
pub use state::AgentEngineRegistry;
