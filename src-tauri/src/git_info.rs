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

fn find_git_dir(start: &Path) -> Option<PathBuf> {
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
