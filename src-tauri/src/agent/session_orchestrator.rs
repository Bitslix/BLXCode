//! Session facade: dispatches one user turn against the configured
//! provider (real HTTP stream) or falls back to the mock engine when no
//! key/model is available.
use crate::agent::openrouter::{run_chat_turn, Endpoint};
use crate::agent::protocol::{AgentEvent, UserTurn};
use crate::agent::spawn_mock_turn;
use crate::agent::state::AgentEngineState;
use crate::agent_settings::{load_settings_pub, provider_key_pub, AgentProviderKind};
use std::sync::Arc;
use tauri::{async_runtime, AppHandle};

pub fn dispatch_user_turn(
    app: &AppHandle,
    agent: &Arc<AgentEngineState>,
    turn: UserTurn,
) -> Result<(), String> {
    if agent.busy() {
        return Err("Agent ist noch beschäftigt.".into());
    }

    let settings = match load_settings_pub(app) {
        Ok(s) => s,
        Err(e) => {
            // Couldn't even read settings — fall back to mock for offline dev.
            eprintln!("[agent] settings load failed, using mock: {e}");
            spawn_mock_turn(Arc::clone(agent), turn.prompt, turn.workspace_root);
            return Ok(());
        }
    };

    let endpoint = match Endpoint::from_provider(settings.provider) {
        Some(e) => e,
        None => {
            // Anthropic native is not wired yet (Phase E). Fall back to mock so
            // the user still sees output instead of a stalled UI.
            spawn_mock_turn(Arc::clone(agent), turn.prompt, turn.workspace_root);
            return Ok(());
        }
    };

    // OpenRouter is open-access for some models but most providers require
    // a key — only Openrouter happens to also accept no-auth for a tiny
    // free tier. We still always require a key for predictability.
    let api_key = match provider_key_pub(app, settings.provider) {
        Ok(k) if !k.trim().is_empty() => k,
        Ok(_) | Err(_) => {
            spawn_chat_missing_key(Arc::clone(agent), settings.provider);
            return Ok(());
        }
    };

    let state = Arc::clone(agent);
    async_runtime::spawn(async move {
        run_chat_turn(
            state,
            endpoint,
            api_key,
            settings,
            turn.prompt,
            turn.workspace_root,
        )
        .await;
    });
    Ok(())
}

fn spawn_chat_missing_key(state: Arc<AgentEngineState>, provider: AgentProviderKind) {
    async_runtime::spawn(async move {
        state.clear_cancel();
        state.set_busy(true);
        state.push(AgentEvent::Error {
            message: format!(
                "Kein API-Key für {} hinterlegt. In den Harness-Einstellungen setzen.",
                provider.as_str()
            ),
        });
        state.push(AgentEvent::Done);
        state.set_busy(false);
    });
}
