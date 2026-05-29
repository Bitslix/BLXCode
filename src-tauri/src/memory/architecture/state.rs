use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agents_layout::MEMORY_REL;

const STATE_FILE: &str = "architecture-state.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureState {
    pub git_rev: Option<String>,
    pub generated_at: String,
    pub crate_count: u32,
    pub module_count: u32,
}

pub fn state_path(workspace_root: &Path) -> PathBuf {
    workspace_root
        .join(MEMORY_REL)
        .join(crate::memory::paths::META_DIRNAME)
        .join(STATE_FILE)
}

pub fn read_state(workspace_root: &Path) -> Option<ArchitectureState> {
    let body = fs::read_to_string(state_path(workspace_root)).ok()?;
    serde_json::from_str(&body).ok()
}

pub fn write_state(workspace_root: &Path, state: &ArchitectureState) -> Result<(), String> {
    let path = state_path(workspace_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(state).map_err(|e| format!("serialize state: {e}"))?;
    fs::write(&path, format!("{body}\n")).map_err(|e| format!("write {}: {e}", path.display()))
}

pub fn now_unix_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}
