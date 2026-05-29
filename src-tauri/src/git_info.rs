use crate::proc::command;
use std::path::{Path, PathBuf};

pub fn current_branch(start: &Path) -> Option<String> {
    let git_dir = find_git_dir(start)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(rest) = head.strip_prefix("ref: ") {
        let name = rest.strip_prefix("refs/heads/").unwrap_or(rest);
        if name.is_empty() {
            return None;
        }
        return Some(name.to_string());
    }
    // Detached HEAD: short SHA
    let sha: String = head.chars().take(7).collect();
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}

pub fn head_commit(start: &Path) -> Option<String> {
    if git_cli_available() {
        let output = command("git")
            .arg("-C")
            .arg(start)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .ok()?;
        if output.status.success() {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if is_fullish_sha(&sha) {
                return Some(sha);
            }
        }
    }
    head_commit_from_files(start)
}

/// Returns true when `start` is inside a Git work tree (`.git` file or directory).
#[must_use]
pub fn is_git_repository(start: &Path) -> bool {
    find_git_dir(start).is_some()
}

/// Whether the `git` executable runs successfully (`git --version`).
#[must_use]
pub fn git_cli_available() -> bool {
    command("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn head_commit_from_files(start: &Path) -> Option<String> {
    let git_dir = find_git_dir(start)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(rest) = head.strip_prefix("ref: ") {
        let loose_ref = git_dir.join(rest);
        if let Ok(sha) = std::fs::read_to_string(&loose_ref) {
            let sha = sha.trim().to_owned();
            if is_fullish_sha(&sha) {
                return Some(sha);
            }
        }
        let packed = std::fs::read_to_string(git_dir.join("packed-refs")).ok()?;
        for line in packed.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(sha) = parts.next() else { continue };
            let Some(name) = parts.next() else { continue };
            if name == rest && is_fullish_sha(sha) {
                return Some(sha.to_owned());
            }
        }
        return None;
    }
    if is_fullish_sha(head) {
        Some(head.to_owned())
    } else {
        None
    }
}

fn is_fullish_sha(value: &str) -> bool {
    value.len() >= 7 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn is_git_repository_detects_dot_git() {
        let tmp = std::env::temp_dir().join(format!("blx_git_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        assert!(!is_git_repository(&tmp));
        fs::create_dir_all(tmp.join(".git")).unwrap();
        assert!(is_git_repository(&tmp));
        let _ = fs::remove_dir_all(&tmp);
    }
}

pub(crate) fn find_git_dir(start: &Path) -> Option<PathBuf> {
    let mut cur: Option<&Path> = Some(start);
    while let Some(p) = cur {
        let candidate = p.join(".git");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if candidate.is_file() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if let Some(rest) = content.trim().strip_prefix("gitdir: ") {
                    let pb = PathBuf::from(rest);
                    let resolved = if pb.is_absolute() { pb } else { p.join(pb) };
                    if resolved.is_dir() {
                        return Some(resolved);
                    }
                }
            }
        }
        cur = p.parent();
    }
    None
}
