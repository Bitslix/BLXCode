//! Sandboxed directory listing for the sidebar project explorer.

use std::fs;
use std::path::{Path, PathBuf};

const MAX_TEXT_PREVIEW_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntryBrief {
    pub name: String,
    pub is_dir: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFilePreview {
    pub content: String,
    pub truncated: bool,
    pub byte_len: u64,
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
pub fn list_path_entries(
    workspace_root: String,
    path: String,
) -> Result<Vec<FsEntryBrief>, String> {
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
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase()),
    });
    Ok(out)
}

/// Reads a UTF-8 text file under `workspace_root` for the center preview tab.
#[tauri::command]
pub fn read_workspace_text_file(
    workspace_root: String,
    path: String,
) -> Result<TextFilePreview, String> {
    let root = canonical_root(&workspace_root)?;
    let file = resolve_under_root(&root, &path)?;
    if !file.is_file() {
        return Err("not a file".into());
    }
    let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
    let byte_len = meta.len();
    let mut bytes = fs::read(&file).map_err(|e| e.to_string())?;
    let truncated = byte_len > MAX_TEXT_PREVIEW_BYTES;
    if truncated {
        bytes.truncate(MAX_TEXT_PREVIEW_BYTES as usize);
    }
    let content =
        String::from_utf8(bytes).map_err(|_| "file is not valid UTF-8 text".to_string())?;
    Ok(TextFilePreview {
        content,
        truncated,
        byte_len,
    })
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

    #[test]
    fn read_workspace_text_file_reads_under_root() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("hello.txt"), b"hello").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let preview = read_workspace_text_file(root, "hello.txt".into()).unwrap();
        assert_eq!(preview.content, "hello");
        assert!(!preview.truncated);
        assert_eq!(preview.byte_len, 5);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_text_file_rejects_outside_root() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let outside = std::env::temp_dir().join(format!("blx_fs_out_{}", uuid::Uuid::new_v4()));
        fs::write(&outside, b"outside").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = read_workspace_text_file(root, outside.to_string_lossy().into_owned())
            .expect_err("outside path should fail");
        assert!(err.contains("outside workspace"));
        let _ = fs::remove_dir_all(tmp);
        let _ = fs::remove_file(outside);
    }

    #[test]
    fn read_workspace_text_file_rejects_directories() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(tmp.join("dir")).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = read_workspace_text_file(root, "dir".into()).expect_err("directory should fail");
        assert_eq!(err, "not a file");
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_text_file_handles_missing_files() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = read_workspace_text_file(root, "missing.txt".into()).expect_err("missing");
        assert!(err.contains("path not found"));
        let _ = fs::remove_dir_all(tmp);
    }
}
