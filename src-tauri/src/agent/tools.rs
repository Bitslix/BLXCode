//! Read-only file access scoped to workspace root.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceRootGuard {
    /// Canonical filesystem path to project root (UTF-8 for MVP).
    path: PathBuf,
}

impl WorkspaceRootGuard {
    pub fn parse(user_input: &str) -> Result<Option<Self>, String> {
        let p = PathBuf::from(user_input.trim());
        if user_input.trim().is_empty() {
            return Ok(None);
        }
        let canon = fs::canonicalize(&p).map_err(|e| format!("canonicalize workspace: {e}"))?;
        Ok(Some(Self { path: canon }))
    }

    pub fn contains(&self, abs: &Path) -> bool {
        abs.strip_prefix(&self.path).is_ok() || abs == self.path
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScopedReadOps;

#[derive(Clone, Debug, thiserror::Error)]
pub enum ReadToolError {
    #[error("no workspace configured")]
    NoWorkspace,
    #[error("path escapes workspace root")]
    PathEscape,
    #[error("invalid path")]
    InvalidPath,
    #[error("{0}")]
    Io(String),
}

impl ScopedReadOps {
    pub fn read_text(
        root: Option<&WorkspaceRootGuard>,
        relative: &str,
    ) -> Result<String, ReadToolError> {
        let guard = root.ok_or(ReadToolError::NoWorkspace)?;
        let rel = RelativePath::normalize(relative).ok_or(ReadToolError::InvalidPath)?;
        let full = guard.path.join(&rel);

        guard
            .contains(&full)
            .then_some(())
            .ok_or(ReadToolError::PathEscape)?;

        fs::read_to_string(&full).map_err(|e| ReadToolError::Io(e.to_string()))
    }
}

struct RelativePath;

impl RelativePath {
    fn normalize(rel: &str) -> Option<PathBuf> {
        let trimmed = rel.trim_start_matches('/');
        if trimmed.contains("..") {
            return None;
        }
        if trimmed.is_empty() {
            return None;
        }
        Some(PathBuf::from(trimmed))
    }
}
