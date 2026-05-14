//! Session facade: dispatches one user turn against the configured
//! provider (real HTTP stream) or falls back to the mock engine when no
//! key/model is available.
use crate::agent::anthropic::run_chat_turn as run_anthropic_turn;
use crate::agent::openrouter::{run_chat_turn, Endpoint};
use crate::agent::protocol::{AgentEvent, UserTurn};
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
            spawn_settings_error(Arc::clone(agent), e);
            return Ok(());
        }
    };

    // Every wired provider needs a key — bail early with a friendly UI message.
    let api_key = match provider_key_pub(app, settings.provider) {
        Ok(k) if !k.trim().is_empty() => k,
        Ok(_) | Err(_) => {
            spawn_chat_missing_key(Arc::clone(agent), settings.provider);
            return Ok(());
        }
    };

    let state = Arc::clone(agent);
    match settings.provider {
        AgentProviderKind::Anthropic => {
            async_runtime::spawn(async move {
                run_anthropic_turn(state, api_key, settings, turn.prompt, turn.workspace_root)
                    .await;
            });
        }
        AgentProviderKind::Openrouter | AgentProviderKind::Openai => {
            let endpoint = Endpoint::from_provider(settings.provider)
                .expect("openrouter/openai endpoint mapping");
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
        }
    }
    Ok(())
}

fn spawn_settings_error(state: Arc<AgentEngineState>, err: String) {
    async_runtime::spawn(async move {
        state.clear_cancel();
        state.set_busy(true);
        state.push(AgentEvent::Error {
            message: format!("Agent-Settings konnten nicht geladen werden: {err}"),
        });
        state.push(AgentEvent::Done);
        state.set_busy(false);
    });
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
