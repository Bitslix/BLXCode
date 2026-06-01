//! Voice subsystem: mic recording (cpal) + STT/TTS + push-to-talk.
//!
//! Layout:
//! - `recorder` – cpal capture into WAV files (cloud path) or in-memory PCM (PTT).
//! - `stt`      – `SttBackend` trait with `cloud` (OpenAI/OpenRouter) + local whisper.
//! - `tts`      – JSON POST to OpenAI speech endpoint, returns MP3 bytes.
//! - `models`   – downloadable whisper model catalog + resumable downloader.
//! - `ptt`      – push-to-talk runtime state + collision machine.
//! - `settings` – `VoiceSettings` persisted alongside `agent_provider_settings.json`.
//! - `commands` – Tauri command handlers.

pub mod commands;
pub mod models;
pub mod ptt;
pub mod recorder;
pub mod settings;
pub mod stt;
pub mod tts;

pub use commands::*;
pub use models::{
    whisper_model_cancel, whisper_model_delete, whisper_model_download, whisper_models_list,
    WhisperDownloadState,
};
pub use ptt::{
    ptt_cancel, ptt_finalize, ptt_partial, ptt_start, voice_agent_input_active, voice_tts_playing,
    VoiceRuntimeStateHandle,
};
pub use recorder::VoiceRecorderState;
pub use stt::WhisperEngine;
#[allow(unused_imports)]
pub use settings::{
    PostSttFlow, PttHotkey, PttSettings, SttLanguageMode, SttSettings, TtsSettings,
    VoiceProviderKind, VoiceSettings,
};
