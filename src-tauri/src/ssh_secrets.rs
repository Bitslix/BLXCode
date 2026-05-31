//! Encrypted storage for SSH connection secrets (login passwords + private
//! key passphrases). Mirrors the keyring + Linux file-fallback pattern from
//! `agent_settings` so secrets land in the OS credential store (Windows
//! Credential Manager / macOS Keychain / Linux secret service) and never in
//! the plaintext `remote_connections.json` preset store.
//!
//! Secrets are keyed by `ssh:<connection_id>:<kind>` where `connection_id`
//! is the immutable uuid of a `RemoteConnection`.

use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const SECRETS_DIR: &str = "secrets";
const KEYRING_SERVICE: &str = "BLXCode";

/// The two kinds of secret a connection can carry. A connection never needs
/// both (password auth → `Password`, key auth → optional `Passphrase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SshSecretKind {
    Password,
    Passphrase,
}

impl SshSecretKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Passphrase => "passphrase",
        }
    }
}

fn keyring_account(id: &str, kind: SshSecretKind) -> String {
    format!("ssh:{id}:{}", kind.as_str())
}

fn keyring_entry(id: &str, kind: SshSecretKind) -> Result<keyring_core::Entry, String> {
    keyring_core::Entry::new(KEYRING_SERVICE, &keyring_account(id, kind))
        .map_err(|e| format!("keyring init ssh {id}: {e}"))
}

fn secrets_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(SECRETS_DIR))
}

fn fallback_path(app: &AppHandle, id: &str, kind: SshSecretKind) -> Result<PathBuf, String> {
    Ok(secrets_dir(app)?.join(format!("ssh-{id}-{}.secret", kind.as_str())))
}

#[cfg(unix)]
fn ensure_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("mkdir {}: {e}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("chmod 700 {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn ensure_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("mkdir {}: {e}", path.display()))
}

fn read_fallback(app: &AppHandle, id: &str, kind: SshSecretKind) -> Result<Option<String>, String> {
    let path = fallback_path(app, id, kind)?;
    match fs::read_to_string(&path) {
        Ok(raw) => {
            let trimmed = raw.trim().to_string();
            Ok(if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read fallback {}: {e}", path.display())),
    }
}

fn write_fallback(app: &AppHandle, id: &str, kind: SshSecretKind, secret: &str) -> Result<(), String> {
    let dir = secrets_dir(app)?;
    ensure_private_dir(&dir)?;
    let path = fallback_path(app, id, kind)?;
    #[cfg(unix)]
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("create fallback {}: {e}", path.display()))?;
    #[cfg(not(unix))]
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .map_err(|e| format!("create fallback {}: {e}", path.display()))?;
    file.write_all(secret.as_bytes())
        .map_err(|e| format!("write fallback {}: {e}", path.display()))?;
    file.sync_all().ok();
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("chmod 600 {}: {e}", path.display()))?;
    Ok(())
}

fn delete_fallback(app: &AppHandle, id: &str, kind: SshSecretKind) -> Result<(), String> {
    let path = fallback_path(app, id, kind)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("delete fallback {}: {e}", path.display())),
    }
}

/// Store a secret (keyring first, Linux file fallback on failure). Empty
/// input deletes the secret instead of storing a blank value.
pub fn set_secret(
    app: &AppHandle,
    id: &str,
    kind: SshSecretKind,
    secret: &str,
) -> Result<(), String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return delete_secret(app, id, kind);
    }
    let entry = keyring_entry(id, kind)?;
    match entry.set_password(trimmed) {
        Ok(()) => {}
        Err(_e) if cfg!(target_os = "linux") => {
            return write_fallback(app, id, kind, trimmed);
        }
        Err(e) => return Err(format!("keyring set ssh {id}: {e}")),
    }
    // Verify the readback so a silently-empty store falls back on Linux.
    match entry.get_password() {
        Ok(saved) if !saved.trim().is_empty() => {
            let _ = delete_fallback(app, id, kind);
            Ok(())
        }
        _ if cfg!(target_os = "linux") => write_fallback(app, id, kind, trimmed),
        Ok(_) => Err(format!(
            "keyring verify ssh {id}: secret written but readback was empty"
        )),
        Err(e) => Err(format!("keyring verify ssh {id}: readback failed: {e}")),
    }
}

/// Read the raw secret (used backend-side when spawning ssh). `None` if not
/// configured.
pub fn get_secret(
    app: &AppHandle,
    id: &str,
    kind: SshSecretKind,
) -> Result<Option<String>, String> {
    let entry = keyring_entry(id, kind)?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => Ok(Some(secret)),
        Ok(_) | Err(keyring_core::Error::NoEntry) => read_fallback(app, id, kind),
        Err(_) if cfg!(target_os = "linux") => read_fallback(app, id, kind),
        Err(e) => Err(format!("keyring get ssh {id}: {e}")),
    }
}

/// Whether a secret is configured — for UI status without exposing the value.
pub fn has_secret(app: &AppHandle, id: &str, kind: SshSecretKind) -> bool {
    matches!(get_secret(app, id, kind), Ok(Some(_)))
}

/// Delete a single secret from keyring + fallback. Idempotent.
pub fn delete_secret(app: &AppHandle, id: &str, kind: SshSecretKind) -> Result<(), String> {
    let entry = keyring_entry(id, kind)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => {}
        Err(_) if cfg!(target_os = "linux") => {}
        Err(e) => return Err(format!("keyring delete ssh {id}: {e}")),
    }
    delete_fallback(app, id, kind)
}

/// Delete every secret tied to a connection (called when a preset is removed).
pub fn delete_all(app: &AppHandle, id: &str) -> Result<(), String> {
    delete_secret(app, id, SshSecretKind::Password)?;
    delete_secret(app, id, SshSecretKind::Passphrase)?;
    Ok(())
}
