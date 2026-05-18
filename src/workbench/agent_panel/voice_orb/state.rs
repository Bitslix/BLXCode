//! Voice-orb state machine + hotkey spec used by the agent panel.

use crate::tauri_bridge::PttHotkey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceOrbState {
    Idle,
    /// Recording started by hold (PTT) — stop on key/mouse release.
    RecordingHold,
    /// Recording started by toggle click — stop on next click.
    RecordingToggle,
    Transcribing,
}

impl VoiceOrbState {
    pub fn is_recording(self) -> bool {
        matches!(self, Self::RecordingHold | Self::RecordingToggle)
    }
}

/// Classify a `mousedown` → `mouseup` interaction.
pub const HOLD_THRESHOLD_MS: f64 = 250.0;

pub fn hotkey_matches(spec: &PttHotkey, ev: &web_sys::KeyboardEvent) -> bool {
    if !spec.enabled {
        return false;
    }
    if ev.code() != spec.code {
        return false;
    }
    ev.ctrl_key() == spec.ctrl
        && ev.shift_key() == spec.shift
        && ev.alt_key() == spec.alt
        && ev.meta_key() == spec.meta
}

/// True for `<input>`, `<textarea>`, `[contenteditable]` etc., where typing
/// should be allowed without intercepting the key as a hotkey.
pub fn focus_in_editable() -> bool {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    let Some(el) = doc.active_element() else {
        return false;
    };
    let tag = el.tag_name().to_ascii_lowercase();
    if tag == "input" || tag == "textarea" {
        return true;
    }
    matches!(el.get_attribute("contenteditable").as_deref(), Some("true") | Some(""))
}
