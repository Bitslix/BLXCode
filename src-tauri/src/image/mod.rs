//! Image generation subsystem.
//!
//! Layout:
//! - `settings`  – `ImageSettings` persisted as the `image` envelope key in
//!   `agent_provider_settings.json`.
//! - `generate`  – HTTP calls to OpenAI `/v1/images/generations` (and
//!   `/v1/images/edits` for img2img) and OpenRouter chat-completions with
//!   `modalities: ["image"]`.
//! - `commands`  – Tauri command handlers.

pub mod commands;
pub mod generate;
pub mod settings;

pub use commands::*;
#[allow(unused_imports)]
pub use settings::{ImageProviderKind, ImageQualityLevel, ImageSettings};
