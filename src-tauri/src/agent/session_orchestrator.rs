//! Session facade: dispatches one user turn against the configured
//! provider (real HTTP stream) or falls back to the mock engine when no
//! key/model is available.
use crate::agent::anthropic::run_chat_turn as run_anthropic_turn;
use crate::agent::openrouter::{run_chat_turn, Endpoint};
use crate::agent::protocol::{AgentContextItem, AgentEvent, UserTurn};
use crate::agent::state::AgentEngineState;
use crate::agent_settings::{load_settings_pub, provider_key_pub, AgentProviderKind};
use crate::voice::{self, VoiceSettings};
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
    let app_handle = app.clone();
    let voice_input = turn.voice_input;
    let prompt = render_context_prompt(turn.prompt, &turn.context_items);
    let workspace_root = turn.workspace_root;
    match settings.provider {
        AgentProviderKind::Anthropic => {
            async_runtime::spawn(async move {
                run_anthropic_turn(
                    Arc::clone(&state),
                    api_key,
                    settings,
                    prompt,
                    workspace_root,
                )
                .await;
                if voice_input {
                    maybe_emit_tts(&app_handle, &state).await;
                }
            });
        }
        AgentProviderKind::Openrouter | AgentProviderKind::Openai => {
            let endpoint = Endpoint::from_provider(settings.provider)
                .expect("openrouter/openai endpoint mapping");
            async_runtime::spawn(async move {
                run_chat_turn(
                    Arc::clone(&state),
                    endpoint,
                    api_key,
                    settings,
                    prompt,
                    workspace_root,
                )
                .await;
                if voice_input {
                    maybe_emit_tts(&app_handle, &state).await;
                }
            });
        }
    }
    Ok(())
}

fn render_context_prompt(prompt: String, context_items: &[AgentContextItem]) -> String {
    if context_items.is_empty() {
        return prompt;
    }

    let mut out = String::from("Attached BLXCode context (paths only; read files if needed):\n");
    for item in context_items {
        let paths = if item.paths.is_empty() {
            item.source.clone()
        } else {
            item.paths.join(", ")
        };
        out.push_str(&format!("- {}: {}\n", item.label.trim(), paths));
    }
    out.push_str("\nUser prompt:\n");
    out.push_str(&prompt);
    out
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

/// After a turn finishes, run TTS over the final assistant text if voice
/// output is configured. Errors are surfaced as `AgentEvent::Error` rather
/// than failing the whole turn (the text answer already reached the user).
async fn maybe_emit_tts(app: &AppHandle, state: &Arc<AgentEngineState>) {
    let voice_settings: VoiceSettings = match voice::settings::load(app) {
        Ok(v) => v,
        Err(e) => {
            state.push(AgentEvent::Error {
                message: format!("Voice-Settings konnten nicht geladen werden: {e}"),
            });
            return;
        }
    };
    if !voice_settings.tts.enabled {
        return;
    }
    let Some(text) = last_assistant_text(state) else {
        return;
    };
    if text.trim().is_empty() {
        return;
    }
    let api_key = match voice::settings::provider_key(app, voice_settings.tts.provider) {
        Ok(k) => k,
        Err(e) => {
            state.push(AgentEvent::Error {
                message: format!("Voice-Key fehlt: {e}"),
            });
            return;
        }
    };
    match voice::tts::synthesize(
        voice_settings.tts.provider,
        &voice_settings.tts.model_id,
        &voice_settings.tts.voice,
        &text,
        &api_key,
    )
    .await
    {
        Ok(bytes) => {
            use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
            state.push(AgentEvent::VoiceReady {
                audio_b64: BASE64.encode(&bytes),
                mime: "audio/mpeg".into(),
            });
        }
        Err(e) => {
            state.push(AgentEvent::Error {
                message: format!("TTS-Fehler: {e}"),
            });
        }
    }
}

fn last_assistant_text(state: &Arc<AgentEngineState>) -> Option<String> {
    let convo = state.conversation_snapshot();
    for msg in convo.iter().rev() {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role != "assistant" {
            continue;
        }
        if let Some(content) = msg.get("content") {
            if let Some(s) = content.as_str() {
                return Some(s.to_string());
            }
            // Anthropic-style content array: [{ "type":"text", "text":"..."}, ...]
            if let Some(arr) = content.as_array() {
                let mut buf = String::new();
                for block in arr {
                    if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                        if let Some(s) = block.get("text").and_then(|v| v.as_str()) {
                            if !buf.is_empty() {
                                buf.push('\n');
                            }
                            buf.push_str(s);
                        }
                    }
                }
                if !buf.is_empty() {
                    return Some(buf);
                }
            }
        }
    }
    None
}
