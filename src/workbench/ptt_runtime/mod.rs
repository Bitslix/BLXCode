//! Push-to-talk runtime: a window-level hold-to-talk handler that drives the
//! backend `ptt_*` commands and routes the finished transcript to the
//! configured target (agent composer, active terminal, active text input, or
//! clipboard).
//!
//! The hotkey itself is defined in Settings → Shortcuts as
//! [`ShortcutAction::PushToTalk`]; this module reads the bound combo from
//! [`AppPrefsService`] and applies hold (down→up) semantics. It is window-level
//! (active while the app is focused); a true OS-global shortcut would need a
//! Tauri plugin and is intentionally out of scope.
//!
//! Cross-component delivery to the agent composer goes through [`PttBus`], a
//! Leptos context the agent panel observes — no DOM `CustomEvent` plumbing.

mod view;

pub use view::PttIndicator;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, KeyboardEvent};

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    clipboard_write_text, is_tauri_shell, ptt_cancel, ptt_finalize, ptt_partial, ptt_start,
    pty_write, voice_settings_get, voice_tts_playing, PttInsertTarget, PttSettings, PttTargetMode,
    VoiceSettings,
};
use crate::workbench::app_prefs::AppPrefsService;
use crate::workbench::state::WorkbenchService;
use crate::workbench::toast::ToastService;

/// Shared push-to-talk signals, provided at the workbench root.
#[derive(Clone, Copy)]
pub struct PttBus {
    /// True while recording or transcribing (drives the indicator).
    pub recording: RwSignal<bool>,
    /// Live partial transcript (empty when idle / disabled).
    pub partial: RwSignal<String>,
    /// A finalized transcript destined for the agent composer:
    /// `(text, auto_submit)`. The agent panel consumes and clears it.
    pub agent_transcript: RwSignal<Option<(String, bool)>>,
    /// Transient status hint (e.g. "blocked while TTS is playing").
    pub hint: RwSignal<Option<String>>,
}

impl Default for PttBus {
    fn default() -> Self {
        Self {
            recording: RwSignal::new(false),
            partial: RwSignal::new(String::new()),
            agent_transcript: RwSignal::new(None),
            hint: RwSignal::new(None),
        }
    }
}

/// The insertion target resolved at the moment it is needed.
#[derive(Clone)]
enum ResolvedTarget {
    Agent,
    Terminal(u64),
    ActiveInput(HtmlElement),
    Clipboard,
}

/// True for `<input>`, `<textarea>`, or `[contenteditable]` — where a bare key
/// must be left to the field instead of intercepted as a hotkey.
fn focus_in_editable() -> bool {
    let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
    else {
        return false;
    };
    let tag = el.tag_name().to_ascii_lowercase();
    if tag == "input" || tag == "textarea" {
        return true;
    }
    matches!(
        el.get_attribute("contenteditable").as_deref(),
        Some("true") | Some("")
    )
}

fn active_html_element() -> Option<HtmlElement> {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
        .and_then(|el| el.dyn_into::<HtmlElement>().ok())
}

/// Resolve the active terminal's PTY session id (focused terminal of the active
/// workspace).
fn active_terminal_session(wb: &WorkbenchService) -> Option<u64> {
    let ws_key = wb.active_workspace_storage_key()?;
    let term_key = wb
        .focused_terminal_by_workspace()
        .get_untracked()
        .get(&ws_key)
        .cloned()?;
    wb.pty_sessions_signal()
        .get_untracked()
        .get(&term_key)
        .copied()
}

fn locale_hint(settings: &VoiceSettings, i18n: &I18nService) -> Option<String> {
    use crate::tauri_bridge::SttLanguageMode;
    match &settings.stt_language {
        SttLanguageMode::Manual { code } if !code.is_empty() => Some(code.clone()),
        SttLanguageMode::FollowApp => Some(i18n.locale().get_untracked().as_str().to_string()),
        _ => None,
    }
}

/// Install the window-level push-to-talk handler. Listeners are removed on
/// component cleanup.
pub fn install_ptt_runtime(
    prefs: AppPrefsService,
    wb: WorkbenchService,
    i18n: I18nService,
    bus: PttBus,
) {
    if !is_tauri_shell() {
        return;
    }
    let toast = expect_context::<ToastService>();
    let Some(window) = web_sys::window() else {
        return;
    };

    // Shared per-session state across the down/up closures.
    let turn_id: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let target: Rc<RefCell<Option<ResolvedTarget>>> = Rc::new(RefCell::new(None));
    let active = Rc::new(RefCell::new(false));
    // Backpressure: at most one partial decode in flight at a time, even if a
    // new hold starts a second poll loop before the previous one has exited.
    let partial_in_flight = Rc::new(Cell::new(false));

    // --- key down: start recording -----------------------------------------
    let down = {
        let turn_id = turn_id.clone();
        let target = target.clone();
        let active = active.clone();
        let partial_in_flight = partial_in_flight.clone();
        Closure::<dyn FnMut(KeyboardEvent)>::new(move |ev: KeyboardEvent| {
            if ev.repeat() || *active.borrow() {
                return;
            }
            let settings = match current_settings() {
                Some(s) => s,
                None => return,
            };
            if !settings.ptt.enabled {
                return;
            }
            let Some(chord) = prefs.shortcut_config().get_untracked().ptt_chord() else {
                return;
            };
            if !chord.matches(&ev) {
                return;
            }
            // A bare (modifier-less) key must not be stolen from a text field.
            let has_mod = chord.ctrl || chord.shift || chord.alt;
            if !has_mod && focus_in_editable() {
                return;
            }
            ev.prevent_default();
            *active.borrow_mut() = true;

            // Capture the target now if it must survive a focus change.
            if settings.ptt.target_mode == PttTargetMode::RememberStart {
                *target.borrow_mut() = resolve_target(&settings.ptt, &wb);
            } else {
                *target.borrow_mut() = None;
            }

            bus.recording.set(true);
            bus.partial.set(String::new());
            bus.hint.set(None);

            // FnMut: clone the shared handles per invocation for the async work.
            let turn_for_start = turn_id.clone();
            let active_for_start = active.clone();
            // Resolve localized hint strings up front (i18n is not Send).
            let hint_busy = i18n.tr(I18nKey::VoicePttMicBusy)().to_string();
            let hint_tts = i18n.tr(I18nKey::VoicePttBlockedTts)().to_string();
            let err_no_mic = i18n.tr(I18nKey::VoiceErrNoMic)().to_string();
            let toast_for_start = toast;
            spawn_local(async move {
                match ptt_start().await {
                    Ok(resp) if resp.started => {
                        if resp.decision == "stopTts" || resp.decision == "pauseTts" {
                            let _ = voice_tts_playing(false).await;
                        }
                        *turn_for_start.borrow_mut() = resp.turn_id;
                    }
                    Ok(resp) => {
                        // Rejected (busy or TTS playing): reset and hint.
                        *active_for_start.borrow_mut() = false;
                        bus.recording.set(false);
                        let hint = if resp.decision == "rejectTtsPlaying" {
                            hint_tts
                        } else {
                            hint_busy
                        };
                        toast_for_start.error(hint.clone());
                        bus.hint.set(Some(hint));
                    }
                    Err(_) => {
                        *active_for_start.borrow_mut() = false;
                        bus.recording.set(false);
                        toast_for_start.error(err_no_mic);
                    }
                }
            });

            // Start partial polling if enabled (local mode only; backend
            // returns empty otherwise).
            if settings.ptt.partial_transcript {
                start_partial_poll(
                    turn_id.clone(),
                    active.clone(),
                    partial_in_flight.clone(),
                    bus,
                    locale_hint(&settings, &i18n),
                );
            }
        })
    };

    // --- key up: finalize ----------------------------------------------------
    let up = {
        let turn_id = turn_id.clone();
        let target = target.clone();
        let active = active.clone();
        Closure::<dyn FnMut(KeyboardEvent)>::new(move |ev: KeyboardEvent| {
            if !*active.borrow() {
                return;
            }
            let Some(chord) = prefs.shortcut_config().get_untracked().ptt_chord() else {
                return;
            };
            // On release the modifiers may already be up; match the main key.
            if !chord.matches_key_only(&ev) {
                return;
            }
            ev.prevent_default();
            *active.borrow_mut() = false;
            bus.recording.set(false);

            let Some(id) = turn_id.borrow_mut().take() else {
                bus.partial.set(String::new());
                return;
            };
            let settings = match current_settings() {
                Some(s) => s,
                None => return,
            };
            // Resolve the target now for "current focus" mode.
            let resolved = target
                .borrow_mut()
                .take()
                .or_else(|| resolve_target(&settings.ptt, &wb));
            let hint = locale_hint(&settings, &i18n);
            let wb = wb;
            let bus = bus;
            let i18n = i18n;
            let toast = toast;
            spawn_local(async move {
                bus.partial.set(String::new());
                let text = match ptt_finalize(id, hint).await {
                    Ok(text) => text,
                    Err(err) => {
                        toast.error(ptt_finalize_error_message(&err, &i18n));
                        return;
                    }
                };
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return;
                }
                route_transcript(
                    trimmed.to_string(),
                    settings.ptt.auto_submit,
                    resolved,
                    &wb,
                    bus,
                    &i18n,
                    toast,
                )
                .await;
            });
        })
    };

    let _ = window.add_event_listener_with_callback("keydown", down.as_ref().unchecked_ref());
    let _ = window.add_event_listener_with_callback("keyup", up.as_ref().unchecked_ref());

    let down = send_wrapper::SendWrapper::new(down);
    let up = send_wrapper::SendWrapper::new(up);
    let win = send_wrapper::SendWrapper::new(window);
    let turn_cleanup = send_wrapper::SendWrapper::new(turn_id.clone());
    on_cleanup(move || {
        let d = down.take();
        let u = up.take();
        let w = win.take();
        let _ = w.remove_event_listener_with_callback("keydown", d.as_ref().unchecked_ref());
        let _ = w.remove_event_listener_with_callback("keyup", u.as_ref().unchecked_ref());
        // Abandon any in-flight recording.
        if let Some(id) = turn_cleanup.take().borrow_mut().take() {
            spawn_local(async move {
                let _ = ptt_cancel(id).await;
            });
        }
    });
}

fn current_settings() -> Option<VoiceSettings> {
    // Synchronous best-effort read isn't possible across the async bridge, so
    // callers fetch fresh settings; this helper exists for symmetry and is
    // replaced by the cached value below.
    PTT_SETTINGS_CACHE.with(|c| c.borrow().clone())
}

thread_local! {
    static PTT_SETTINGS_CACHE: RefCell<Option<VoiceSettings>> = const { RefCell::new(None) };
}

/// Refresh the cached voice settings (called on mount and whenever settings are
/// saved). Keeps the synchronous keydown path from needing an await.
pub fn refresh_ptt_settings_cache() {
    if !is_tauri_shell() {
        return;
    }
    spawn_local(async move {
        if let Ok(s) = voice_settings_get().await {
            PTT_SETTINGS_CACHE.with(|c| *c.borrow_mut() = Some(s));
        }
    });
}

fn resolve_target(ptt: &PttSettings, wb: &WorkbenchService) -> Option<ResolvedTarget> {
    match ptt.insert_target {
        PttInsertTarget::Agent => Some(ResolvedTarget::Agent),
        PttInsertTarget::Clipboard => Some(ResolvedTarget::Clipboard),
        PttInsertTarget::Terminal => active_terminal_session(wb).map(ResolvedTarget::Terminal),
        PttInsertTarget::ActiveInput => active_html_element().map(ResolvedTarget::ActiveInput),
    }
}

fn start_partial_poll(
    turn_id: Rc<RefCell<Option<String>>>,
    active: Rc<RefCell<bool>>,
    in_flight: Rc<Cell<bool>>,
    bus: PttBus,
    hint: Option<String>,
) {
    spawn_local(async move {
        // Throttle to ~400 ms between re-decodes.
        loop {
            gloo_timers::future::TimeoutFuture::new(400).await;
            if !*active.borrow() {
                break;
            }
            // Skip this tick if a decode from a previous tick (or another poll
            // loop) is still running — never stack overlapping decodes.
            if in_flight.get() {
                continue;
            }
            let id = match turn_id.borrow().clone() {
                Some(id) => id,
                None => continue,
            };
            in_flight.set(true);
            let result = ptt_partial(id, hint.clone()).await;
            in_flight.set(false);
            if let Ok(text) = result {
                if *active.borrow() && !text.trim().is_empty() {
                    bus.partial.set(text);
                }
            }
        }
    });
}

async fn route_transcript(
    text: String,
    auto_submit: bool,
    target: Option<ResolvedTarget>,
    _wb: &WorkbenchService,
    bus: PttBus,
    i18n: &I18nService,
    toast: ToastService,
) {
    match target {
        Some(ResolvedTarget::Agent) | None => {
            bus.agent_transcript.set(Some((text, auto_submit)));
        }
        Some(ResolvedTarget::Clipboard) => {
            if clipboard_write_text(text).await.is_err() {
                toast.error(i18n.tr(I18nKey::VoicePttInsertFailed)());
            }
        }
        Some(ResolvedTarget::Terminal(session)) => {
            let payload = if auto_submit {
                format!("{text}\r")
            } else {
                text
            };
            if pty_write(session, BASE64.encode(payload.as_bytes()))
                .await
                .is_err()
            {
                toast.error(i18n.tr(I18nKey::VoicePttInsertFailed)());
            }
        }
        Some(ResolvedTarget::ActiveInput(el)) => {
            if !insert_into_input(&el, &text) {
                toast.error(i18n.tr(I18nKey::VoicePttInsertFailed)());
            }
        }
    }
}

fn ptt_finalize_error_message(err: &str, i18n: &I18nService) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("kein lokales whisper-modell")
        || lower.contains("no local whisper model")
        || lower.contains("no local model")
    {
        return i18n.tr(I18nKey::VoicePttNoModel)().to_string();
    }
    if lower.contains("whisper")
        || lower.contains("model")
        || lower.contains("load")
        || lower.contains("ggml")
    {
        return i18n.tr(I18nKey::VoicePttModelLoadFailed)().to_string();
    }
    fill_first_placeholder(i18n.tr(I18nKey::VoiceErrStt)(), err)
}

fn fill_first_placeholder(template: &str, value: &str) -> String {
    let Some(start) = template.find('{') else {
        return format!("{template}: {value}");
    };
    let Some(end_offset) = template[start..].find('}') else {
        return format!("{template}: {value}");
    };
    let end = start + end_offset + 1;
    format!("{}{}{}", &template[..start], value, &template[end..])
}

/// Insert `text` into an `<input>`/`<textarea>` at the caret, or append to a
/// contenteditable. Fires an `input` event so frameworks observe the change.
fn insert_into_input(el: &HtmlElement, text: &str) -> bool {
    use web_sys::{HtmlInputElement, HtmlTextAreaElement};
    if let Some(input) = el.dyn_ref::<HtmlInputElement>() {
        let cur = input.value();
        input.set_value(&format!("{cur}{text}"));
    } else if let Some(area) = el.dyn_ref::<HtmlTextAreaElement>() {
        let cur = area.value();
        area.set_value(&format!("{cur}{text}"));
    } else {
        // contenteditable
        let cur = el.inner_text();
        el.set_inner_text(&format!("{cur}{text}"));
    }
    // Best-effort input event.
    if let Ok(ev) = web_sys::Event::new("input") {
        return el.dispatch_event(&ev).is_ok();
    }
    false
}
