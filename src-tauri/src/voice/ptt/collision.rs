//! Collision state machine that keeps push-to-talk from clashing with the
//! existing agent voice input and TTS playback. A single source of truth for
//! "who owns the microphone / speaker right now".

use crate::voice::settings::TtsCollision;

/// What the voice subsystem is currently doing. Exactly one capture or playback
/// activity may be active at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VoiceRuntimeState {
    #[default]
    Idle,
    RecordingPtt,
    TranscribingPtt,
    PlayingTts,
    AgentVoiceInputActive,
}

/// The action a PTT-start request resolves to, given the current state and the
/// configured TTS-collision policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PttStartDecision {
    /// Begin recording immediately.
    Start,
    /// Stop the currently playing TTS, then record.
    StopTtsThenStart,
    /// Pause the currently playing TTS, then record.
    PauseTtsThenStart,
    /// Reject: another mic session is active.
    RejectBusy,
    /// Reject: TTS is playing and the policy is `Block`.
    RejectTtsPlaying,
}

impl VoiceRuntimeState {
    /// Decide what should happen when the user presses push-to-talk.
    pub fn decide_ptt_start(self, policy: TtsCollision) -> PttStartDecision {
        match self {
            VoiceRuntimeState::Idle => PttStartDecision::Start,
            VoiceRuntimeState::PlayingTts => match policy {
                TtsCollision::Stop => PttStartDecision::StopTtsThenStart,
                TtsCollision::Pause => PttStartDecision::PauseTtsThenStart,
                TtsCollision::Block => PttStartDecision::RejectTtsPlaying,
            },
            // Any active capture (PTT mid-flight, transcribing, or the agent's
            // own voice input) blocks a second mic session.
            VoiceRuntimeState::RecordingPtt
            | VoiceRuntimeState::TranscribingPtt
            | VoiceRuntimeState::AgentVoiceInputActive => PttStartDecision::RejectBusy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_always_starts() {
        for p in [TtsCollision::Stop, TtsCollision::Pause, TtsCollision::Block] {
            assert_eq!(
                VoiceRuntimeState::Idle.decide_ptt_start(p),
                PttStartDecision::Start
            );
        }
    }

    #[test]
    fn tts_playing_respects_policy() {
        assert_eq!(
            VoiceRuntimeState::PlayingTts.decide_ptt_start(TtsCollision::Stop),
            PttStartDecision::StopTtsThenStart
        );
        assert_eq!(
            VoiceRuntimeState::PlayingTts.decide_ptt_start(TtsCollision::Pause),
            PttStartDecision::PauseTtsThenStart
        );
        assert_eq!(
            VoiceRuntimeState::PlayingTts.decide_ptt_start(TtsCollision::Block),
            PttStartDecision::RejectTtsPlaying
        );
    }

    #[test]
    fn active_capture_is_rejected() {
        for s in [
            VoiceRuntimeState::RecordingPtt,
            VoiceRuntimeState::TranscribingPtt,
            VoiceRuntimeState::AgentVoiceInputActive,
        ] {
            assert_eq!(
                s.decide_ptt_start(TtsCollision::Stop),
                PttStartDecision::RejectBusy
            );
        }
    }
}
