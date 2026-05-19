//! Append BLXCode workspace entries to a project `.gitignore`.

use std::fs;
use std::path::PathBuf;

const BLXCODE_ENTRY: &str = ".blxcode/";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitignoreAppendResult {
    pub path: String,
    pub appended: bool,
    pub already_present: bool,
}

fn validate_workspace_cwd(ws: &str) -> Result<PathBuf, String> {
    let trimmed = ws.trim();
    if trimmed.is_empty() {
        return Err("workspace cwd is empty".into());
    }
    let p = PathBuf::from(trimmed);
    if !p.is_absolute() {
        return Err("workspace cwd must be absolute".into());
    }
    if !p.exists() {
        return Err(format!("workspace cwd does not exist: {trimmed}"));
    }
    Ok(p)
}

fn entry_present(body: &str) -> bool {
    body.lines().any(|line| {
        let t = line.trim();
        t == ".blxcode" || t == ".blxcode/" || t == "/.blxcode" || t == "/.blxcode/"
    })
}

/// Appends `.blxcode/` to `<workspace>/.gitignore` when not already listed.
#[tauri::command]
pub fn gitignore_append_blxcode(workspace_cwd: String) -> Result<GitignoreAppendResult, String> {
    let root = validate_workspace_cwd(&workspace_cwd)?;
    let gitignore_path = root.join(".gitignore");
    let path_display = gitignore_path.to_string_lossy().into_owned();

    if gitignore_path.is_file() {
        let body = fs::read_to_string(&gitignore_path)
            .map_err(|e| format!("read .gitignore: {e}"))?;
        if entry_present(&body) {
            return Ok(GitignoreAppendResult {
                path: path_display,
                appended: false,
                already_present: true,
            });
        }
        let mut next = body;
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        if !next.is_empty() {
            next.push('\n');
        }
        next.push_str("# BLXCode local workspace data\n");
        next.push_str(BLXCODE_ENTRY);
        next.push('\n');
        fs::write(&gitignore_path, next.as_bytes())
            .map_err(|e| format!("write .gitignore: {e}"))?;
        return Ok(GitignoreAppendResult {
            path: path_display,
            appended: true,
            already_present: false,
        });
    }

    let body = format!("# BLXCode local workspace data\n{BLXCODE_ENTRY}\n");
    fs::write(&gitignore_path, body.as_bytes()).map_err(|e| format!("create .gitignore: {e}"))?;
    Ok(GitignoreAppendResult {
        path: path_display,
        appended: true,
        already_present: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_creates_and_dedupes() {
        let ws = std::env::temp_dir().join(format!(
            "blxcode_gitignore_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&ws);
        fs::create_dir_all(&ws).unwrap();
        let cwd = ws.to_string_lossy().into_owned();

        let r1 = gitignore_append_blxcode(cwd.clone()).unwrap();
        assert!(r1.appended);
        let body = fs::read_to_string(ws.join(".gitignore")).unwrap();
        assert!(body.contains(".blxcode/"));

        let r2 = gitignore_append_blxcode(cwd).unwrap();
        assert!(!r2.appended);
        assert!(r2.already_present);

        let _ = fs::remove_dir_all(&ws);
    }
}
