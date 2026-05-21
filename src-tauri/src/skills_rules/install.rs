//! Skill installation from `git`, `npm pack`, or a local workspace path.
//!
//! Each kind populates a fresh `.agents/skills/<name>/` directory containing a
//! readable `SKILL.md`. The dispatch never executes user-controlled shell
//! strings: arguments go straight to `std::process::Command`, the
//! destination path is sandboxed via `validate_skill_name`, and partially
//! materialised directories are rolled back on any failure.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::skills_rules::store::{
    ensure_skills_rules_roots, record_installed_skill, validate_skill_name,
};
use crate::skills_rules::types::{SkillEntry, SkillSourceInput, SkillSourceKind, SkillSourceMeta};

const SKILL_DOC: &str = "SKILL.md";
const INSTALL_TIMEOUT: Duration = Duration::from_secs(90);

pub fn install_skill(ws: &str, name: &str, source: SkillSourceInput) -> Result<SkillEntry, String> {
    validate_skill_name(name)?;
    let roots = ensure_skills_rules_roots(ws)?;
    let final_dir = roots.skills.join(name);
    if final_dir.exists() {
        return Err(format!(
            "skill `{name}` already exists; remove it before reinstalling"
        ));
    }
    // Each install gets a sibling staging directory so the final path stays
    // unobserved until everything is validated.
    let staging = roots.skills.join(format!(".install.{name}.tmp"));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| format!("create staging dir: {e}"))?;

    let result = (|| -> Result<SkillSourceMeta, String> {
        match source.kind {
            SkillSourceKind::Git => install_git(&staging, &source),
            SkillSourceKind::Npm => install_npm(&staging, &source),
            SkillSourceKind::Local => install_local(&staging, ws, &source),
            SkillSourceKind::AgentCreated => {
                Err("agent-created skills must use skills_write, not skills_install".into())
            }
            SkillSourceKind::Core => {
                Err("core skills are built-in and cannot be installed".into())
            }
        }
    })();

    let meta = match result {
        Ok(meta) => meta,
        Err(err) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }
    };

    if !staging.join(SKILL_DOC).is_file() {
        let _ = fs::remove_dir_all(&staging);
        return Err("install source does not contain SKILL.md at the top level".into());
    }

    if let Err(e) = fs::rename(&staging, &final_dir) {
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("move staging -> final: {e}"));
    }

    match record_installed_skill(ws, name, meta) {
        Ok(entry) => Ok(entry),
        Err(e) => {
            let _ = fs::remove_dir_all(&final_dir);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

fn install_git(staging: &Path, src: &SkillSourceInput) -> Result<SkillSourceMeta, String> {
    let url = src
        .url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or("git source requires `url`")?;
    if !is_safe_git_url(url) {
        return Err("git url must use https://, git@, or ssh:// (no scheme tricks)".into());
    }
    let git_ref = src.git_ref.as_deref().unwrap_or("main");
    if !is_safe_ref(git_ref) {
        return Err("git ref contains unsupported characters".into());
    }

    // Step 1 — clone shallow into a fresh tempdir.
    let clone_dir = staging.join("_clone");
    fs::create_dir_all(&clone_dir).map_err(|e| format!("mkdir clone dir: {e}"))?;
    run_with_timeout(
        Command::new("git")
            .arg("clone")
            .arg("--depth=1")
            .arg("--branch")
            .arg(git_ref)
            .arg("--single-branch")
            .arg("--")
            .arg(url)
            .arg(&clone_dir),
        INSTALL_TIMEOUT,
        "git clone",
    )?;

    // Step 2 — strip `.git/`, then promote contents up into `staging`.
    let dotgit = clone_dir.join(".git");
    if dotgit.exists() {
        let _ = fs::remove_dir_all(&dotgit);
    }
    promote_dir_contents(&clone_dir, staging)?;
    let _ = fs::remove_dir_all(&clone_dir);

    Ok(SkillSourceMeta {
        kind: SkillSourceKind::Git,
        url: Some(url.to_owned()),
        git_ref: Some(git_ref.to_owned()),
        package: None,
        version: None,
        path: None,
    })
}

fn is_safe_git_url(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() || u.starts_with('-') {
        return false;
    }
    u.starts_with("https://")
        || u.starts_with("http://")
        || u.starts_with("git@")
        || u.starts_with("ssh://")
        || u.starts_with("git://")
}

fn is_safe_ref(r: &str) -> bool {
    !r.is_empty()
        && !r.starts_with('-')
        && r.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
}

// ---------------------------------------------------------------------------
// Npm
// ---------------------------------------------------------------------------

fn install_npm(staging: &Path, src: &SkillSourceInput) -> Result<SkillSourceMeta, String> {
    let package = src
        .package
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or("npm source requires `package`")?;
    if !is_safe_npm_package(package) {
        return Err("npm package name contains unsupported characters".into());
    }
    let spec = match src.version.as_deref() {
        Some(v) if !v.trim().is_empty() => {
            if !is_safe_npm_version(v) {
                return Err("npm version contains unsupported characters".into());
            }
            format!("{package}@{v}")
        }
        _ => package.to_owned(),
    };

    let pack_dir = staging.join("_pack");
    fs::create_dir_all(&pack_dir).map_err(|e| format!("mkdir pack dir: {e}"))?;

    // `npm pack <spec>` writes a `*.tgz` into the cwd and prints the filename.
    let output = Command::new("npm")
        .arg("pack")
        .arg("--silent")
        .arg("--")
        .arg(&spec)
        .current_dir(&pack_dir)
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "npm not found in PATH".to_owned(),
            _ => format!("spawn npm pack: {e}"),
        })?;
    if !output.status.success() {
        return Err(format!(
            "npm pack failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let tgz_name = String::from_utf8_lossy(&output.stdout)
        .lines()
        .last()
        .unwrap_or("")
        .trim()
        .to_owned();
    let tgz_path = if tgz_name.is_empty() {
        // Fall back to scanning the dir — some npm versions print nothing.
        fs::read_dir(&pack_dir)
            .ok()
            .and_then(|mut it| {
                it.find_map(|e| {
                    let e = e.ok()?;
                    let n = e.file_name().to_string_lossy().to_string();
                    n.ends_with(".tgz").then_some(pack_dir.join(n))
                })
            })
            .ok_or("npm pack did not produce a tarball")?
    } else {
        pack_dir.join(&tgz_name)
    };
    if !tgz_path.is_file() {
        return Err(format!("npm pack tarball missing on disk"));
    }
    extract_tarball_via_tar(&tgz_path, staging)?;
    let _ = fs::remove_dir_all(&pack_dir);

    Ok(SkillSourceMeta {
        kind: SkillSourceKind::Npm,
        url: None,
        git_ref: None,
        package: Some(package.to_owned()),
        version: src.version.clone(),
        path: None,
    })
}

fn is_safe_npm_package(p: &str) -> bool {
    let p = p.trim();
    if p.is_empty() || p.starts_with('-') {
        return false;
    }
    p.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '/' | '.' | '_' | '-'))
}

fn is_safe_npm_version(v: &str) -> bool {
    let v = v.trim();
    !v.is_empty()
        && !v.starts_with('-')
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-' | '+' | '~' | '^'))
}

/// Extracts a `*.tgz` produced by `npm pack` into `dest` by delegating to the
/// system `tar` binary (`-xzf`). The `package/` prefix that `npm pack` adds
/// is stripped via `--strip-components=1` so the staging dir ends up with
/// `SKILL.md` at the top.
///
/// Shell-out keeps us free of a flate2/tar crate dependency; `tar` ships on
/// macOS, Linux, and modern Windows (10 1803+ via the bundled bsdtar).
fn extract_tarball_via_tar(tgz: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("mkdir extract dest: {e}"))?;
    run_with_timeout(
        Command::new("tar")
            .arg("--strip-components=1")
            .arg("-xzf")
            .arg(tgz)
            .arg("-C")
            .arg(dest),
        INSTALL_TIMEOUT,
        "tar -xzf",
    )
}

// ---------------------------------------------------------------------------
// Local
// ---------------------------------------------------------------------------

fn install_local(
    staging: &Path,
    ws: &str,
    src: &SkillSourceInput,
) -> Result<SkillSourceMeta, String> {
    let raw = src
        .path
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or("local source requires `path`")?;
    let rel = raw.trim();
    if rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
        return Err("local path must be a workspace-relative directory".into());
    }
    let ws_path = PathBuf::from(ws);
    let src_dir = ws_path.join(rel);
    if !src_dir.is_dir() {
        return Err(format!("local source not found: {rel}"));
    }
    // Ensure src is inside workspace (canonicalise both sides; if canonicalise
    // fails we still require the literal prefix).
    let canon_ws = fs::canonicalize(&ws_path).unwrap_or(ws_path.clone());
    let canon_src = fs::canonicalize(&src_dir).unwrap_or(src_dir.clone());
    if !canon_src.starts_with(&canon_ws) {
        return Err("local source escapes workspace".into());
    }
    copy_dir_recursive(&src_dir, staging)?;
    Ok(SkillSourceMeta {
        kind: SkillSourceKind::Local,
        url: None,
        git_ref: None,
        package: None,
        version: None,
        path: Some(rel.to_owned()),
    })
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn promote_dir_contents(src: &Path, dest: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if to.exists() {
            // Conflict (e.g. `.install.foo.tmp/_clone` then `_clone` again):
            // skip to keep the operation idempotent and predictable.
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            fs::rename(&from, &to)
                .or_else(|_| copy_dir_recursive(&from, &to).and_then(|_| Ok(())))
                .map_err(|e| format!("promote dir {}: {e}", from.display()))?;
        } else {
            fs::rename(&from, &to)
                .or_else(|_| fs::copy(&from, &to).map(|_| ()))
                .map_err(|e| format!("promote file {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Process helpers
// ---------------------------------------------------------------------------

fn run_with_timeout(cmd: &mut Command, timeout: Duration, label: &str) -> Result<(), String> {
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!("{label}: command not found in PATH"),
        _ => format!("spawn {label}: {e}"),
    })?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                return Err(format!("{label} exited with status {status}"));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    return Err(format!("{label} timed out after {}s", timeout.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => return Err(format!("{label} wait: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::UNIX_EPOCH;

    fn fresh_ws(tag: &str) -> PathBuf {
        let ws = std::env::temp_dir().join(format!(
            "blxcode_install_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&ws);
        fs::create_dir_all(&ws).unwrap();
        ws
    }

    #[test]
    fn local_install_copies_dir_and_records_source() {
        let ws = fresh_ws("local_ok");
        let src = ws.join("seed");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("SKILL.md"), "# Seed\n\nhello").unwrap();
        let entry = install_skill(
            &ws.to_string_lossy(),
            "seed-skill",
            SkillSourceInput {
                kind: SkillSourceKind::Local,
                url: None,
                git_ref: None,
                package: None,
                version: None,
                path: Some("seed".into()),
            },
        )
        .unwrap();
        assert!(matches!(entry.source.kind, SkillSourceKind::Local));
        assert!(entry.enabled);
        assert!(ws.join(".agents/skills/seed-skill/SKILL.md").is_file());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn local_install_rejects_when_no_skill_md() {
        let ws = fresh_ws("local_missing");
        fs::create_dir_all(ws.join("seed")).unwrap();
        fs::write(ws.join("seed/README.md"), "no skill here").unwrap();
        let err = install_skill(
            &ws.to_string_lossy(),
            "bad",
            SkillSourceInput {
                kind: SkillSourceKind::Local,
                url: None,
                git_ref: None,
                package: None,
                version: None,
                path: Some("seed".into()),
            },
        )
        .unwrap_err();
        assert!(err.contains("SKILL.md"));
        // Staging cleaned up; final dir was never created.
        assert!(!ws.join(".agents/skills/bad").exists());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn local_install_rejects_escape_paths() {
        let ws = fresh_ws("local_escape");
        let err = install_skill(
            &ws.to_string_lossy(),
            "bad",
            SkillSourceInput {
                kind: SkillSourceKind::Local,
                url: None,
                git_ref: None,
                package: None,
                version: None,
                path: Some("../etc".into()),
            },
        )
        .unwrap_err();
        assert!(err.contains("workspace"));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn install_rejects_agent_created_kind() {
        let ws = fresh_ws("agent_created");
        let err = install_skill(
            &ws.to_string_lossy(),
            "x",
            SkillSourceInput {
                kind: SkillSourceKind::AgentCreated,
                url: None,
                git_ref: None,
                package: None,
                version: None,
                path: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("agent-created"));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn install_rejects_unsafe_git_url() {
        let ws = fresh_ws("bad_url");
        let err = install_skill(
            &ws.to_string_lossy(),
            "x",
            SkillSourceInput {
                kind: SkillSourceKind::Git,
                url: Some("--upload-pack=evil".into()),
                git_ref: None,
                package: None,
                version: None,
                path: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("git url"));
        let _ = fs::remove_dir_all(&ws);
    }
}
