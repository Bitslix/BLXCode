//! Session facade: dispatches one user turn against the configured
//! provider (real HTTP stream) or falls back to the mock engine when no
//! key/model is available.
use crate::agent::anthropic::run_chat_turn as run_anthropic_turn;
use crate::agent::openrouter::{run_chat_turn, Endpoint};
use crate::agent::protocol::{
    AgentContextItem, AgentContextKind, AgentEvent, AgentImageContextItem, UserTurn,
};
use crate::agent::state::AgentEngineState;
use crate::agent_settings::{load_settings_pub, provider_key_pub, AgentProviderKind};
use crate::image::{self, generate as image_generate};
use crate::plans;
use crate::voice::{self, VoiceSettings};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
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

    // Image-mode branch: short-circuits the text-agent pipeline. Loads the
    // image settings + the image-provider's API key (NOT the text-agent
    // provider's key) and spawns one HTTP request.
    if turn.image_generate {
        dispatch_image_turn(app, agent, turn);
        return Ok(());
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
    let prompt = render_context_prompt(
        turn.prompt,
        &turn.context_items,
        turn.workspace_root.as_deref(),
    );
    let workspace_root = turn.workspace_root;
    let image_context_items = turn.image_context_items;
    match settings.provider {
        AgentProviderKind::Anthropic => {
            async_runtime::spawn(async move {
                run_anthropic_turn(
                    Arc::clone(&state),
                    api_key,
                    settings,
                    prompt,
                    image_context_items,
                    workspace_root,
                )
                .await;
                if voice_input {
                    maybe_emit_tts_for_last_assistant(&app_handle, &state).await;
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
                    image_context_items,
                    workspace_root,
                )
                .await;
                if voice_input {
                    maybe_emit_tts_for_last_assistant(&app_handle, &state).await;
                }
            });
        }
    }
    Ok(())
}

/// Image-mode dispatch. Does **not** touch the text conversation history;
/// emits a single `ImageGenerated` event + `Done` and (optionally) a TTS
/// confirmation when the turn originated from voice input.
fn dispatch_image_turn(app: &AppHandle, agent: &Arc<AgentEngineState>, turn: UserTurn) {
    let state = Arc::clone(agent);
    let app_handle = app.clone();
    async_runtime::spawn(async move {
        state.clear_cancel();
        state.set_busy(true);
        let result = run_image_turn(&app_handle, &state, &turn).await;
        match result {
            Ok(()) => {
                state.push(AgentEvent::Done);
            }
            Err(message) => {
                state.push(AgentEvent::Error { message });
                state.push(AgentEvent::Done);
            }
        }
        // TTS only fires when the user spoke the prompt; the confirmation
        // text is short and fixed (not the assistant transcript).
        if turn.voice_input {
            emit_tts_for_text(&app_handle, &state, tts_confirmation_text()).await;
        }
        state.set_busy(false);
    });
}

async fn run_image_turn(
    app: &AppHandle,
    state: &Arc<AgentEngineState>,
    turn: &UserTurn,
) -> Result<(), String> {
    let settings = image::settings::load(app)
        .map_err(|e| format!("Image-Settings konnten nicht geladen werden: {e}"))?;
    let api_key = match image::settings::provider_key(app, settings.provider) {
        Ok(k) if !k.trim().is_empty() => k,
        Ok(_) | Err(_) => {
            return Err(format!(
                "Kein API-Key für Bild-Provider {} hinterlegt. In den Image-Einstellungen setzen.",
                settings.provider.as_str()
            ));
        }
    };

    let refs: &[AgentImageContextItem] = &turn.image_context_items;
    let generated = image_generate::generate(state, &settings, &api_key, &turn.prompt, refs)
        .await
        .map_err(|e| e.into_message())?;

    // Only mark refs as consumed after a successful 2xx (i.e. now).
    if !refs.is_empty() {
        state.push(AgentEvent::ImageContextConsumed {
            ids: refs.iter().map(|r| r.id.clone()).collect(),
        });
    }

    // Persist when we have a workspace root, otherwise inline only.
    let (saved_path, filename) = if let Some(root) = turn
        .workspace_root
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        match image_generate::save_to_workspace(
            std::path::Path::new(root),
            &generated,
            &turn.prompt,
        ) {
            Ok(saved) => (
                Some(saved.abs_path.to_string_lossy().into_owned()),
                Some(saved.filename),
            ),
            Err(e) => {
                state.push(AgentEvent::Error {
                    message: format!("Bild speichern fehlgeschlagen: {e}"),
                });
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    let preview_src = format!(
        "data:{};base64,{}",
        generated.mime,
        BASE64.encode(&generated.bytes)
    );
    state.push(AgentEvent::ImageGenerated {
        prompt: turn.prompt.clone(),
        mime: generated.mime,
        saved_path,
        filename,
        preview_src,
    });
    Ok(())
}

fn tts_confirmation_text() -> String {
    // Locale-neutral: short German phrase matching the UI's primary locale
    // hint. The string itself is short enough that any TTS engine handles
    // it cleanly even when set to follow-app locale.
    "Bild erstellt.".to_owned()
}

fn render_context_prompt(
    prompt: String,
    context_items: &[AgentContextItem],
    workspace_root: Option<&str>,
) -> String {
    if context_items.is_empty() {
        return prompt;
    }

    let (plans, memory_like): (Vec<_>, Vec<_>) = context_items.iter().partition(|item| {
        matches!(
            item.kind,
            AgentContextKind::PlanIndex
                | AgentContextKind::PlanFile
                | AgentContextKind::PlanTaskGroup
        )
    });

    let mut out = String::new();

    if !memory_like.is_empty() {
        out.push_str("Attached BLXCode context (paths only; read files if needed):\n");
        for item in &memory_like {
            let paths = if item.paths.is_empty() {
                item.source.clone()
            } else {
                item.paths.join(", ")
            };
            out.push_str(&format!("- {}: {}\n", item.label.trim(), paths));
        }
    }

    if !plans.is_empty() {
        if !memory_like.is_empty() {
            out.push('\n');
        }
        out.push_str("Attached plans (call `plan_read` for details, `plan_load` to sync tasks):\n");
        for item in &plans {
            match item.kind {
                AgentContextKind::PlanIndex => {
                    out.push_str(&format!(
                        "- plan index: {} — see `.agents/plans/PLANS.md`\n",
                        item.label.trim()
                    ));
                }
                AgentContextKind::PlanFile => {
                    let plan_path = item
                        .paths
                        .first()
                        .cloned()
                        .unwrap_or_else(|| item.source.clone());
                    let mut line = format!(
                        "- plan `{plan_path}`: {label}",
                        label = item.label.trim()
                    );
                    if let Some(ws) = workspace_root {
                        if let Some(meta) = plans::plan_meta_for(ws, &plan_path) {
                            line.push_str(&format!(
                                " — tasks total {}, pending {}, in_progress {}, blocked {}, completed {}, cancelled {}",
                                meta.task_summary.total,
                                meta.task_summary.pending,
                                meta.task_summary.in_progress,
                                meta.task_summary.blocked,
                                meta.task_summary.completed,
                                meta.task_summary.cancelled
                            ));
                        }
                    }
                    line.push('\n');
                    out.push_str(&line);
                }
                AgentContextKind::PlanTaskGroup => {
                    out.push_str(&format!(
                        "- plan tasks: {} ({})\n",
                        item.label.trim(),
                        item.source
                    ));
                }
                _ => {}
            }
        }
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

/// After a text turn finishes, run TTS over the final assistant text if
/// voice output is configured. Used by the Anthropic / OpenAI / OpenRouter
/// chat paths.
async fn maybe_emit_tts_for_last_assistant(app: &AppHandle, state: &Arc<AgentEngineState>) {
    let Some(text) = last_assistant_text(state) else {
        return;
    };
    emit_tts_for_text(app, state, text).await;
}

/// Run TTS over a caller-provided text. Used by the image branch for
/// short confirmation phrases, and indirectly by chat turns via
/// `maybe_emit_tts_for_last_assistant`.
async fn emit_tts_for_text(app: &AppHandle, state: &Arc<AgentEngineState>, text: String) {
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
