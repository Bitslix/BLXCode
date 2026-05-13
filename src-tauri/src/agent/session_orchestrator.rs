//! Session-Fassade: Orchestrierung eines User-Turns an die eingebaute Engine.
use crate::agent::protocol::UserTurn;
use crate::agent::spawn_mock_turn;
use crate::agent::state::AgentEngineState;
use std::sync::Arc;

pub fn dispatch_user_turn(agent: &Arc<AgentEngineState>, turn: UserTurn) -> Result<(), String> {
    if agent.busy() {
        return Err("Agent ist noch beschäftigt.".into());
    }
    spawn_mock_turn(Arc::clone(agent), turn.prompt, turn.workspace_root);
    Ok(())
}
