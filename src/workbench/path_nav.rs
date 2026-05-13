//! Path navigation for the wizard: Tauri uses real FS; CSR uses string-only `PathBuf` joins.

use std::path::{Path, PathBuf};

/// When not in Tauri, resolve `cd` without `canonicalize` (no FS verification).
#[must_use]
pub fn path_nav_wasm_string(base: &str, line: &str) -> Result<(String, String), String> {
    let base_pb = if base.trim().is_empty() {
        PathBuf::from("/")
    } else {
        PathBuf::from(base.trim())
    };
    let line = line.trim();
    if line.is_empty() {
        let s = base_pb.to_string_lossy().into_owned();
        return Ok((s.clone(), s));
    }
    let lower = line.to_ascii_lowercase();
    let rest = if lower == "cd" {
        ""
    } else if lower.starts_with("cd ") {
        line[3..].trim()
    } else {
        return Err("only 'cd' is supported".into());
    };
    let target = push_cd_path(&base_pb, rest)?;
    let cwd = target.to_string_lossy().into_owned();
    Ok((cwd.clone(), format!("cd -> {cwd}")))
}

fn push_cd_path(base: &Path, arg: &str) -> Result<PathBuf, String> {
    let arg = arg.trim();
    if arg.is_empty() {
        return Ok(base.to_path_buf());
    }
    if arg == "~" || arg.starts_with("~/") {
        return Err("HOME is not available in the browser build; use an absolute path.".into());
    }
    let joined = if arg.starts_with('/') {
        PathBuf::from(arg)
    } else {
        base.join(arg)
    };
    Ok(normalize_path(&joined))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::Prefix(p) => out.push(p.as_os_str()),
            std::path::Component::RootDir => {
                out = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::Normal(s) => out.push(s),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        out
    }
}
