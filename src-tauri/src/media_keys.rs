//! Keyring-backed API keys for image/video and voice media providers (fal.ai,
//! Amazon Polly). Surfaced in Settings → API Keys and consumed by the image
//! and (future) voice subsystems.

use serde::Serialize;
use tauri::AppHandle;

use crate::agent_settings;

const KEYRING_SERVICE: &str = "BLXCode";

const KEY_FAL: &str = "agent:media:fal";
const KEY_POLLY: &str = "agent:media:polly";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKeyKind {
    Fal,
    AwsPolly,
}

impl MediaKeyKind {
    pub fn kind_id(self) -> &'static str {
        match self {
            Self::Fal => "fal",
            Self::AwsPolly => "aws_polly",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Fal => "fal.ai",
            Self::AwsPolly => "Amazon Polly (TTS/STT)",
        }
    }

    fn account(self) -> &'static str {
        match self {
            Self::Fal => KEY_FAL,
            Self::AwsPolly => KEY_POLLY,
        }
    }

    pub fn env_var(self) -> &'static str {
        match self {
            Self::Fal => "BLX_FAL_API_KEY",
            Self::AwsPolly => "BLX_AWS_POLLY_API_KEY",
        }
    }
}

pub const MEDIA_KEY_KINDS: [MediaKeyKind; 2] = [MediaKeyKind::Fal, MediaKeyKind::AwsPolly];

pub fn kind_from_id(kind: &str) -> Option<MediaKeyKind> {
    MEDIA_KEY_KINDS
        .iter()
        .copied()
        .find(|k| k.kind_id() == kind)
}

fn keyring_entry(kind: MediaKeyKind) -> Result<keyring_core::Entry, String> {
    keyring_core::Entry::new(KEYRING_SERVICE, kind.account())
        .map_err(|e| format!("keyring init {}: {e}", kind.account()))
}

fn env_key(kind: MediaKeyKind) -> Option<String> {
    std::env::var(kind.env_var())
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// (masked_value, via_env). Order: keyring → env.
pub fn key_with_source(kind: MediaKeyKind) -> Result<(Option<String>, bool), String> {
    let entry = keyring_entry(kind)?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => {
            Ok((agent_settings::mask_secret_pub(&secret), false))
        }
        Ok(_) | Err(keyring_core::Error::NoEntry) => match env_key(kind) {
            Some(secret) => Ok((agent_settings::mask_secret_pub(&secret), true)),
            None => Ok((None, false)),
        },
        Err(_) if cfg!(target_os = "linux") => match env_key(kind) {
            Some(secret) => Ok((agent_settings::mask_secret_pub(&secret), true)),
            None => Ok((None, false)),
        },
        Err(e) => Err(format!("keyring get {}: {e}", kind.account())),
    }
}

pub fn resolve_key(kind: MediaKeyKind) -> Option<String> {
    env_key(kind).or_else(|| {
        keyring_entry(kind)
            .ok()?
            .get_password()
            .ok()
            .filter(|s| !s.trim().is_empty())
    })
}

pub fn set_key(_app: &AppHandle, kind: MediaKeyKind, api_key: &str) -> Result<(), String> {
    let secret = api_key.trim();
    if secret.is_empty() {
        return Err("API key is empty".into());
    }
    let entry = keyring_entry(kind)?;
    entry
        .set_password(secret)
        .map_err(|e| format!("keyring set {}: {e}", kind.account()))
}

pub fn delete_key(_app: &AppHandle, kind: MediaKeyKind) -> Result<(), String> {
    let entry = keyring_entry(kind)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keyring delete {}: {e}", kind.account())),
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaKeyCatalogEntry {
    pub kind: String,
    pub label: String,
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked_value: Option<String>,
    pub via_env: bool,
    pub env_var: String,
}

pub fn catalog_entry(kind: MediaKeyKind) -> Result<MediaKeyCatalogEntry, String> {
    let (masked, via_env) = key_with_source(kind)?;
    Ok(MediaKeyCatalogEntry {
        kind: kind.kind_id().into(),
        label: kind.label().into(),
        configured: masked.is_some(),
        masked_value: masked,
        via_env,
        env_var: kind.env_var().into(),
    })
}
