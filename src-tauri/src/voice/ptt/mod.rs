//! Push-to-talk runtime: shared voice-runtime state + collision policy.
//!
//! The transcript routing (agent composer / terminal / active input /
//! clipboard) and the live re-decode worker live on the frontend and in
//! `commands`; this module owns only the process-wide arbitration state so the
//! microphone and speaker are never double-booked.

pub mod collision;
pub mod commands;

pub use collision::{PttStartDecision, VoiceRuntimeState};
pub use commands::{
    ptt_cancel, ptt_finalize, ptt_partial, ptt_start, voice_agent_input_active, voice_tts_playing,
};

use std::sync::Mutex;

use crate::voice::settings::TtsCollision;

/// Process-wide voice-runtime state, registered as Tauri state.
#[derive(Default)]
pub struct VoiceRuntimeStateHandle {
    state: Mutex<VoiceRuntimeState>,
}

impl VoiceRuntimeStateHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> VoiceRuntimeState {
        self.state.lock().map(|g| *g).unwrap_or_default()
    }

    pub fn set(&self, next: VoiceRuntimeState) {
        if let Ok(mut g) = self.state.lock() {
            *g = next;
        }
    }

    /// Resolve a push-to-talk start request against the current state and the
    /// configured TTS-collision policy. The caller acts on the decision (e.g.
    /// stop/pause TTS) and only then transitions to `RecordingPtt`.
    pub fn decide_ptt_start(&self, policy: TtsCollision) -> PttStartDecision {
        self.current().decide_ptt_start(policy)
    }
}
