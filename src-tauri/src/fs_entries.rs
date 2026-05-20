//! Sandboxed directory listing for the sidebar project explorer.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntryBrief {
    pub name: String,
    pub is_dir: bool,
    pub hidden: bool,
}

fn canonical_root(workspace_root: &str) -> Result<PathBuf, String> {
    let trimmed = workspace_root.trim();
    if trimmed.is_empty() {
        return Err("workspace root is empty".into());
    }
    let p = PathBuf::from(trimmed);
    if !p.is_dir() {
        return Err("workspace root is not a directory".into());
    }
    fs::canonicalize(&p).map_err(|e| format!("canonicalize workspace: {e}"))
}

fn resolve_under_root(root: &Path, rel_or_abs: &str) -> Result<PathBuf, String> {
    let target = if rel_or_abs.trim().is_empty() {
        root.to_path_buf()
    } else {
        let p = PathBuf::from(rel_or_abs);
        if p.is_absolute() {
            p
        } else {
            root.join(p)
        }
    };
    let canon = fs::canonicalize(&target).map_err(|e| format!("path not found: {e}"))?;
    if !canon.starts_with(root) {
        return Err("path outside workspace".into());
    }
    Ok(canon)
}

/// Lists files and directories under `path`, constrained to `workspace_root`.
#[tauri::command]
pub fn list_path_entries(workspace_root: String, path: String) -> Result<Vec<FsEntryBrief>, String> {
    let root = canonical_root(&workspace_root)?;
    let dir = resolve_under_root(&root, &path)?;
    if !dir.is_dir() {
        return Err("not a directory".into());
    }
    let read = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    let mut out: Vec<FsEntryBrief> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let ft = e.file_type().ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "." || name == ".." {
                return None;
            }
            let hidden = name.starts_with('.');
            Some(FsEntryBrief {
                name,
                is_dir: ft.is_dir(),
                hidden,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a
                .name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase()),
        }
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn list_path_entries_sorts_dirs_first() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("z.txt"), b"").unwrap();
        fs::create_dir_all(tmp.join("a_dir")).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let entries = list_path_entries(root.clone(), root.clone()).unwrap();
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "a_dir");
        let _ = fs::remove_dir_all(tmp);
    }
}
