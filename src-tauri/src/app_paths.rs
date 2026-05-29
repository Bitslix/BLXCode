//! Per-installation paths derived from Tauri's `app_data_dir`.
//!
//! `app_data_dir` is only reachable through an `AppHandle`, which modules
//! deep in the call graph (e.g. agent tools) do not own. We capture the
//! path once in the Tauri `setup` hook (see `lib.rs`) and serve it via a
//! `OnceLock` so any module can request stable subdirectories without
//! passing the handle around.
//!
//! For tests and ad-hoc tooling either the env var
//! `BLX_APP_DATA_DIR_OVERRIDE` or a thread-local override (set via
//! `test_support::with_app_data_dir`) takes precedence over the captured
//! value.

use std::cell::RefCell;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const ENV_OVERRIDE: &str = "BLX_APP_DATA_DIR_OVERRIDE";
const TASKS_DIR: &str = "tasks";

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

thread_local! {
    static TL_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn init(app_data_dir: PathBuf) {
    let _ = APP_DATA_DIR.set(app_data_dir);
}

pub fn app_data_dir() -> Result<PathBuf, String> {
    if let Some(p) = TL_OVERRIDE.with(|cell| cell.borrow().clone()) {
        return Ok(p);
    }
    if let Ok(raw) = env::var(ENV_OVERRIDE) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    APP_DATA_DIR
        .get()
        .cloned()
        .ok_or_else(|| "app data dir not initialised".to_owned())
}

#[cfg(test)]
pub mod test_support {
    use super::*;

    /// RAII guard that swaps the thread-local app-data dir for the
    /// duration of a test and restores it on drop.
    pub struct AppDataDirGuard {
        previous: Option<PathBuf>,
    }

    impl AppDataDirGuard {
        pub fn new(path: PathBuf) -> Self {
            let previous = TL_OVERRIDE.with(|cell| cell.borrow_mut().replace(path));
            Self { previous }
        }
    }

    impl Drop for AppDataDirGuard {
        fn drop(&mut self) {
            let prev = self.previous.take();
            TL_OVERRIDE.with(|cell| *cell.borrow_mut() = prev);
        }
    }
}

/// Returns (and creates) `{app_data_dir}/tasks/<workspace_hash>` for the
/// given workspace path. The hash is derived from the canonicalised
/// absolute path so symlinks and `..` segments do not produce separate
/// task stores for the same workspace.
pub fn tasks_root_for(workspace_cwd: &str) -> Result<PathBuf, String> {
    let trimmed = workspace_cwd.trim();
    if trimmed.is_empty() {
        return Err("workspace cwd is empty".into());
    }
    let raw = PathBuf::from(trimmed);
    if !raw.is_absolute() {
        return Err("workspace cwd must be absolute".into());
    }
    let canonical = fs::canonicalize(&raw).unwrap_or(raw);
    let hash = workspace_hash(&canonical);
    let root = app_data_dir()?.join(TASKS_DIR).join(hash);
    fs::create_dir_all(&root).map_err(|e| format!("create tasks root {}: {e}", root.display()))?;
    Ok(root)
}

/// 64-bit FNV-1a digest of the path bytes, rendered as 16 lowercase hex
/// characters. Stable across runs and platforms; no extra dependency.
pub fn workspace_hash(path: &Path) -> String {
    let bytes = path.as_os_str().to_string_lossy();
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in bytes.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic_and_16_hex_chars() {
        let a = workspace_hash(Path::new("/home/x/repo"));
        let b = workspace_hash(Path::new("/home/x/repo"));
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_differs_for_different_paths() {
        let a = workspace_hash(Path::new("/home/x/repo"));
        let b = workspace_hash(Path::new("/home/x/other"));
        assert_ne!(a, b);
    }
}
