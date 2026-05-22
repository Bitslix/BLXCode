//! Voice subsystem: mic recording (cpal) + STT/TTS HTTP calls.
//!
//! Layout:
//! - `recorder` – cpal capture into WAV files inside `app_cache_dir/voice/`.
//! - `stt`      – multipart POST to OpenAI/OpenRouter audio transcription.
//! - `tts`      – JSON POST to OpenAI speech endpoint, returns MP3 bytes.
//! - `settings` – `VoiceSettings` persisted alongside `agent_provider_settings.json`.
//! - `commands` – Tauri command handlers.

pub mod commands;
pub mod recorder;
pub mod settings;
pub mod stt;
pub mod tts;

pub use commands::*;
pub use recorder::VoiceRecorderState;
#[allow(unused_imports)]
pub use settings::{
    PostSttFlow, PttHotkey, SttLanguageMode, SttSettings, TtsSettings, VoiceProviderKind,
    VoiceSettings,
};
