//! Whisper model manager: catalog, installed-state, and a resumable,
//! progress-reporting downloader.
//!
//! The download logic is decoupled from Tauri so it stays unit-testable: it
//! takes a target directory and a progress callback. The command layer (P2)
//! supplies the app-data models dir and turns progress into frontend events.

pub mod catalog;
pub mod commands;

pub use catalog::{catalog, find, ModelFamily, WhisperModel};
pub use commands::{
    whisper_model_cancel, whisper_model_delete, whisper_model_download, whisper_models_list,
};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

/// Tracks in-flight downloads so they can be cancelled. Registered as Tauri
/// state.
#[derive(Default)]
pub struct WhisperDownloadState {
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl WhisperDownloadState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a cancel flag for `id`, replacing any previous one.
    fn register(&self, id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut g) = self.active.lock() {
            g.insert(id.to_owned(), flag.clone());
        }
        flag
    }

    fn unregister(&self, id: &str) {
        if let Ok(mut g) = self.active.lock() {
            g.remove(id);
        }
    }

    /// Request cancellation of an in-flight download. Returns true if one was
    /// active.
    pub fn cancel(&self, id: &str) -> bool {
        if let Ok(g) = self.active.lock() {
            if let Some(flag) = g.get(id) {
                flag.store(true, Ordering::SeqCst);
                return true;
            }
        }
        false
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.active
            .lock()
            .map(|g| g.contains_key(id))
            .unwrap_or(false)
    }
}

/// Final installed model path for an id (e.g. `<dir>/base.bin`).
pub fn model_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.bin"))
}

/// In-progress `.part` path used for resumable downloads.
pub fn part_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.bin.part"))
}

/// Runtime view of a catalog model for the frontend list.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelView {
    pub id: String,
    pub label: String,
    pub family: ModelFamily,
    pub multilingual: bool,
    pub size_bytes: u64,
    pub speed_rating: u8,
    pub accuracy_rating: u8,
    pub best_for: String,
    pub installed: bool,
    pub installed_path: Option<String>,
    /// Size of a partially downloaded `.part`, if any (drives Resume UI).
    pub partial_bytes: Option<u64>,
}

/// Build the model list with installed / partial state resolved against `dir`.
pub fn list(dir: &Path) -> Vec<WhisperModelView> {
    catalog()
        .iter()
        .map(|m| {
            let installed_path = model_path(dir, m.id);
            let installed = installed_path.exists();
            let partial = part_path(dir, m.id);
            let partial_bytes = std::fs::metadata(&partial).ok().map(|md| md.len());
            WhisperModelView {
                id: m.id.to_string(),
                label: m.label.to_string(),
                family: m.family,
                multilingual: m.multilingual,
                size_bytes: m.size_bytes,
                speed_rating: m.speed_rating,
                accuracy_rating: m.accuracy_rating,
                best_for: m.best_for.to_string(),
                installed,
                installed_path: installed.then(|| installed_path.to_string_lossy().to_string()),
                partial_bytes,
            }
        })
        .collect()
}

/// Delete an installed model (and any stray `.part`).
pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    let path = model_path(dir, id);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("delete {}: {e}", path.display()))?;
    }
    let part = part_path(dir, id);
    let _ = std::fs::remove_file(part);
    Ok(())
}

/// Download (or resume) a model into `dir`, reporting progress as
/// `(received, total, speed_bps)`. Resumes from an existing `.part` via an
/// HTTP Range request; falls back to a full download if the server ignores it.
///
/// Cancellation: a cancel flag is registered in `state` for the duration; on
/// cancel the `.part` is **kept** so a later call resumes.
pub async fn download<F>(
    state: &WhisperDownloadState,
    dir: &Path,
    id: &str,
    mut on_progress: F,
) -> Result<PathBuf, String>
where
    F: FnMut(u64, u64, f64),
{
    let model = find(id).ok_or_else(|| format!("Unbekanntes Modell: {id}"))?;
    let final_path = model_path(dir, id);
    if final_path.exists() {
        return Ok(final_path);
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;

    let part = part_path(dir, id);
    let mut downloaded = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);

    let cancel = state.register(id);
    let result = download_inner(
        model,
        &part,
        &final_path,
        &mut downloaded,
        &cancel,
        &mut on_progress,
    )
    .await;
    state.unregister(id);
    result
}

async fn download_inner<F>(
    model: &WhisperModel,
    part: &Path,
    final_path: &Path,
    downloaded: &mut u64,
    cancel: &AtomicBool,
    on_progress: &mut F,
) -> Result<PathBuf, String>
where
    F: FnMut(u64, u64, f64),
{
    let url = catalog::model_url(model);
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    let mut req = client.get(&url);
    if *downloaded > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={downloaded}-"));
    }
    let res = req
        .send()
        .await
        .map_err(|e| format!("download request: {e}"))?;
    let status = res.status();

    // Open the `.part`: append on a successful resume (206), truncate otherwise.
    let resuming = status == reqwest::StatusCode::PARTIAL_CONTENT && *downloaded > 0;
    if !resuming {
        *downloaded = 0;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resuming)
        .truncate(!resuming)
        .open(part)
        .await
        .map_err(|e| format!("open {}: {e}", part.display()))?;

    if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(format!("download {status} für {}", model.id));
    }

    let total = res
        .content_length()
        .map(|len| *downloaded + len)
        .unwrap_or(model.size_bytes);

    let mut res = res;
    let mut window_start = Instant::now();
    let mut window_bytes = 0u64;
    let mut last_emit = Instant::now();
    on_progress(*downloaded, total, 0.0);

    while let Some(chunk) = res
        .chunk()
        .await
        .map_err(|e| format!("download chunk: {e}"))?
    {
        if cancel.load(Ordering::SeqCst) {
            let _ = file.flush().await;
            return Err("cancelled".into());
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("write {}: {e}", part.display()))?;
        *downloaded += chunk.len() as u64;
        window_bytes += chunk.len() as u64;

        // Throttle progress emission to ~200 ms; compute speed over the window.
        if last_emit.elapsed().as_millis() >= 200 {
            let secs = window_start.elapsed().as_secs_f64().max(1e-6);
            let speed_bps = window_bytes as f64 / secs;
            on_progress(*downloaded, total.max(*downloaded), speed_bps);
            last_emit = Instant::now();
            window_start = Instant::now();
            window_bytes = 0;
        }
    }
    file.flush().await.map_err(|e| format!("flush: {e}"))?;
    drop(file);
    on_progress(*downloaded, total.max(*downloaded), 0.0);

    // Verify integrity when a checksum is published. Hashing reads the whole
    // file (models up to ~1.5 GB), so keep it off the async worker.
    if !model.sha256.is_empty() {
        let part_buf = part.to_path_buf();
        let actual = tauri::async_runtime::spawn_blocking(move || sha256_file(&part_buf))
            .await
            .map_err(|e| format!("sha256 task join: {e}"))??;
        if !actual.eq_ignore_ascii_case(model.sha256) {
            let _ = std::fs::remove_file(part);
            return Err(format!(
                "Prüfsumme stimmt nicht für {} (erwartet {}, erhalten {actual}).",
                model.id, model.sha256
            ));
        }
    }

    std::fs::rename(part, final_path)
        .map_err(|e| format!("rename {} → {}: {e}", part.display(), final_path.display()))?;
    Ok(final_path.to_path_buf())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| format!("hash {}: {e}", path.display()))?;
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_derived_from_id() {
        let dir = Path::new("/tmp/models");
        assert_eq!(model_path(dir, "base"), dir.join("base.bin"));
        assert_eq!(part_path(dir, "base"), dir.join("base.bin.part"));
    }

    #[test]
    fn list_reports_installed_and_partial() {
        let tmp = std::env::temp_dir().join(format!("blx-models-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(model_path(&tmp, "base"), b"x").unwrap();
        std::fs::write(part_path(&tmp, "small"), b"abcd").unwrap();

        let views = list(&tmp);
        let base = views.iter().find(|v| v.id == "base").unwrap();
        assert!(base.installed);
        assert!(base.installed_path.is_some());
        let small = views.iter().find(|v| v.id == "small").unwrap();
        assert!(!small.installed);
        assert_eq!(small.partial_bytes, Some(4));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn cancel_flag_roundtrip() {
        let st = WhisperDownloadState::new();
        assert!(!st.cancel("base")); // nothing active
        let flag = st.register("base");
        assert!(st.is_active("base"));
        assert!(st.cancel("base"));
        assert!(flag.load(Ordering::SeqCst));
        st.unregister("base");
        assert!(!st.is_active("base"));
    }
}
