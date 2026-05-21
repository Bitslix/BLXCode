//! Disk I/O, validation, and self-healing for `.agents/{rules,skills}/`.
//!
//! - Reads/writes the two `index.json` manifests next to the markdown content.
//! - Merges on-disk state with the manifest: files present without index entry
//!   default to `enabled: true`; index entries pointing at missing files are
//!   removed at read time (self-heal).
//! - Atomic writes via `tmp` + `rename` so partial JSON never lands on disk.
//! - Path sandbox: every `name` must be a single segment under the allowed
//!   prefix and is rejected if it tries to escape via `..`, absolute paths,
//!   or extra separators.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agents_layout::{validate_workspace_cwd, AGENTS_REL};
use crate::skills_rules::types::{
    RuleEntry, RuleIndexEntry, RulesIndex, SkillEntry, SkillIndexEntry, SkillSourceKind,
    SkillSourceMeta, SkillsIndex,
};

pub const RULES_REL: &str = ".agents/rules";
pub const SKILLS_REL: &str = ".agents/skills";

/// Built-in harness skills embedded in the binary.
/// Each entry is `(name, markdown_content)`.
pub const CORE_SKILLS: &[(&str, &str)] = &[
    ("file-access", include_str!("../agent/harness_skills/file-access.md")),
    ("memory", include_str!("../agent/harness_skills/memory.md")),
    ("plans", include_str!("../agent/harness_skills/plans.md")),
    ("tasks", include_str!("../agent/harness_skills/tasks.md")),
    ("rules-skills", include_str!("../agent/harness_skills/rules-skills.md")),
    ("harness", include_str!("../agent/harness_skills/harness.md")),
];

const CORE_INSTALLED_AT: &str = "2026-01-01T00:00:00Z";

const RULES_INDEX_FILE: &str = "index.json";
const SKILLS_INDEX_FILE: &str = "index.json";
const SKILL_DOC: &str = "SKILL.md";
const SUMMARY_MAX_CHARS: usize = 200;

// ===========================================================================
// Workspace roots
// ===========================================================================

#[derive(Debug, Clone)]
pub struct SkillsRulesRoots {
    pub rules: PathBuf,
    pub skills: PathBuf,
}

/// Ensure `.agents/rules` and `.agents/skills` exist and that each has an
/// `index.json` manifest. Idempotent — on first touch, every existing
/// `rule-*.md` is recorded as `enabled: true` and every `skills/<name>/`
/// folder as `enabled: true` with `source: { kind: "local" }`. Once a
/// manifest exists, it is the source of truth and only self-healed
/// (orphan entries dropped) at read time.
pub fn ensure_skills_rules_roots(ws: &str) -> Result<SkillsRulesRoots, String> {
    let ws_path = validate_workspace_cwd(ws)?;
    let agents = ws_path.join(AGENTS_REL);
    let rules = ws_path.join(RULES_REL);
    let skills = ws_path.join(SKILLS_REL);
    fs::create_dir_all(&agents).map_err(|e| format!("create {AGENTS_REL}: {e}"))?;
    fs::create_dir_all(&rules).map_err(|e| format!("create {RULES_REL}: {e}"))?;
    fs::create_dir_all(&skills).map_err(|e| format!("create {SKILLS_REL}: {e}"))?;
    bootstrap_rules_index(&rules)?;
    bootstrap_skills_index(&skills)?;
    Ok(SkillsRulesRoots { rules, skills })
}

/// Write `.agents/rules/index.json` if it is missing. Pre-fills with every
/// existing `rule-*.md` on disk, defaulting to `enabled: true`.
fn bootstrap_rules_index(rules_dir: &Path) -> Result<(), String> {
    let path = rules_index_path(rules_dir);
    if path.exists() {
        return Ok(());
    }
    let now = now_rfc3339();
    let mut idx = RulesIndex::default();
    for name in list_rule_files(rules_dir) {
        idx.rules.insert(
            name,
            RuleIndexEntry {
                enabled: true,
                updated_at: now.clone(),
            },
        );
    }
    write_rules_index(rules_dir, &idx)
}

/// Write `.agents/skills/index.json` if it is missing. Pre-fills with every
/// existing `<name>/` folder, defaulting to `enabled: true` and
/// `source.kind = "local"` so the user can later edit the manifest if a
/// folder came in via git/npm but predates this bootstrap.
fn bootstrap_skills_index(skills_dir: &Path) -> Result<(), String> {
    let path = skills_index_path(skills_dir);
    if path.exists() {
        return Ok(());
    }
    let now = now_rfc3339();
    let mut idx = SkillsIndex::default();
    for name in list_skill_dirs(skills_dir) {
        idx.skills.insert(
            name,
            SkillIndexEntry {
                enabled: true,
                source: SkillSourceMeta {
                    kind: SkillSourceKind::Local,
                    url: None,
                    git_ref: None,
                    package: None,
                    version: None,
                    path: None,
                },
                installed_at: now.clone(),
                updated_at: now.clone(),
            },
        );
    }
    write_skills_index(skills_dir, &idx)
}

// ===========================================================================
// Validation
// ===========================================================================

/// Allowed: `rule-foo.md` / `rule-foo-bar_baz.md`. No path separators, no `..`.
pub fn validate_rule_name(name: &str) -> Result<(), String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("rule name is empty".into());
    }
    if n.len() > 120 {
        return Err("rule name too long".into());
    }
    if n.contains('/') || n.contains('\\') || n.contains("..") {
        return Err("rule name must not contain path separators".into());
    }
    if !n.starts_with("rule-") {
        return Err("rule file name must start with `rule-`".into());
    }
    if !n.ends_with(".md") {
        return Err("rule file name must end with `.md`".into());
    }
    let stem = &n[5..n.len() - 3];
    if stem.is_empty() {
        return Err("rule name must include a stem after `rule-`".into());
    }
    if !stem
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("rule name may only contain a-z, 0-9, `-`, `_`".into());
    }
    Ok(())
}

/// Allowed: `[a-z0-9][a-z0-9-_]{0,40}`. No path separators, no `..`.
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("skill name is empty".into());
    }
    if n.len() > 41 {
        return Err("skill name too long".into());
    }
    if n.contains('/') || n.contains('\\') || n.contains("..") {
        return Err("skill name must not contain path separators".into());
    }
    let mut chars = n.chars();
    let first = chars.next().ok_or("empty skill name")?;
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err("skill name must start with a lowercase letter or digit".into());
    }
    for c in chars {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
            return Err("skill name may only contain a-z, 0-9, `-`, `_`".into());
        }
    }
    Ok(())
}

// ===========================================================================
// Helpers
// ===========================================================================

fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Lightweight RFC3339 UTC formatter without bringing in chrono.
    rfc3339_from_unix(secs as i64)
}

fn rfc3339_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);
    let hh = (time_of_day / 3600) as u32;
    let mm = ((time_of_day % 3600) / 60) as u32;
    let ss = (time_of_day % 60) as u32;
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Howard Hinnant's `civil_from_days` for the proleptic Gregorian calendar.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mo = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if mo <= 2 { y + 1 } else { y } as i32;
    (y, mo, d)
}

fn modified_rfc3339(meta: &fs::Metadata) -> String {
    let secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| format!("encode json: {e}"))?;
    let parent = path.parent().ok_or("invalid path")?;
    fs::create_dir_all(parent).map_err(|e| format!("mkdir parent: {e}"))?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("index"),
        std::process::id()
    ));
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create tmp: {e}"))?;
        f.write_all(&json).map_err(|e| format!("write tmp: {e}"))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(|e| format!("rename tmp -> final: {e}"))
}

fn read_or_default_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<T>(&s).ok())
        .unwrap_or_default()
}

/// Strip leading `# Heading` line if present and shrink to `SUMMARY_MAX_CHARS`.
fn extract_title_and_summary(body: &str, fallback_title: &str) -> (String, String) {
    let mut title = fallback_title.to_owned();
    let mut summary = String::new();
    let mut seen_title = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !summary.is_empty() {
                break;
            }
            continue;
        }
        if !seen_title && trimmed.starts_with('#') {
            title = trimmed.trim_start_matches('#').trim().to_owned();
            seen_title = true;
            continue;
        }
        if !summary.is_empty() {
            summary.push(' ');
        }
        summary.push_str(trimmed);
        if summary.chars().count() >= SUMMARY_MAX_CHARS {
            break;
        }
    }
    if summary.chars().count() > SUMMARY_MAX_CHARS {
        summary = summary.chars().take(SUMMARY_MAX_CHARS).collect::<String>();
        summary.push('…');
    }
    if title.is_empty() {
        title = fallback_title.to_owned();
    }
    (title, summary)
}

// ===========================================================================
// Rules
// ===========================================================================

fn rules_index_path(rules_dir: &Path) -> PathBuf {
    rules_dir.join(RULES_INDEX_FILE)
}

fn read_rules_index(rules_dir: &Path) -> RulesIndex {
    read_or_default_json(&rules_index_path(rules_dir))
}

fn write_rules_index(rules_dir: &Path, idx: &RulesIndex) -> Result<(), String> {
    atomic_write_json(&rules_index_path(rules_dir), idx)
}

fn list_rule_files(rules_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(read) = fs::read_dir(rules_dir) else {
        return out;
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if validate_rule_name(&name_str).is_ok() {
            out.push(name_str);
        }
    }
    out.sort();
    out
}

pub fn list_rules(ws: &str) -> Result<Vec<RuleEntry>, String> {
    let roots = ensure_skills_rules_roots(ws)?;
    let files = list_rule_files(&roots.rules);
    let mut idx = read_rules_index(&roots.rules);

    // Self-heal: drop index entries with no corresponding file.
    let known: std::collections::BTreeSet<_> = files.iter().cloned().collect();
    let stale: Vec<_> = idx
        .rules
        .keys()
        .filter(|k| !known.contains(*k))
        .cloned()
        .collect();
    let dirty = !stale.is_empty();
    for k in stale {
        idx.rules.remove(&k);
    }
    if dirty {
        let _ = write_rules_index(&roots.rules, &idx);
    }

    let mut entries = Vec::with_capacity(files.len());
    for name in files {
        let path = roots.rules.join(&name);
        let body = fs::read_to_string(&path).unwrap_or_default();
        let meta = fs::metadata(&path).ok();
        let (title, summary) =
            extract_title_and_summary(&body, name.trim_end_matches(".md"));
        let enabled = idx.rules.get(&name).map(|e| e.enabled).unwrap_or(true);
        entries.push(RuleEntry {
            name,
            title,
            summary,
            enabled,
            size_bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            updated_at: meta.map(|m| modified_rfc3339(&m)).unwrap_or_default(),
        });
    }
    Ok(entries)
}

pub fn read_rule(ws: &str, name: &str) -> Result<String, String> {
    let roots = ensure_skills_rules_roots(ws)?;
    validate_rule_name(name)?;
    let path = roots.rules.join(name);
    fs::read_to_string(&path).map_err(|e| format!("read rule: {e}"))
}

pub fn write_rule(ws: &str, name: &str, content: &str) -> Result<RuleEntry, String> {
    let roots = ensure_skills_rules_roots(ws)?;
    validate_rule_name(name)?;
    let path = roots.rules.join(name);
    fs::write(&path, content.as_bytes()).map_err(|e| format!("write rule: {e}"))?;

    let mut idx = read_rules_index(&roots.rules);
    let now = now_rfc3339();
    idx.rules
        .entry(name.to_owned())
        .and_modify(|e| e.updated_at = now.clone())
        .or_insert(RuleIndexEntry {
            enabled: true,
            updated_at: now.clone(),
        });
    write_rules_index(&roots.rules, &idx)?;
    list_rules(ws)?
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| "rule disappeared after write".into())
}

pub fn set_rule_enabled(ws: &str, name: &str, enabled: bool) -> Result<RuleEntry, String> {
    let roots = ensure_skills_rules_roots(ws)?;
    validate_rule_name(name)?;
    if !roots.rules.join(name).is_file() {
        return Err(format!("rule not found: {name}"));
    }
    let mut idx = read_rules_index(&roots.rules);
    let now = now_rfc3339();
    idx.rules
        .entry(name.to_owned())
        .and_modify(|e| {
            e.enabled = enabled;
            e.updated_at = now.clone();
        })
        .or_insert(RuleIndexEntry {
            enabled,
            updated_at: now.clone(),
        });
    write_rules_index(&roots.rules, &idx)?;
    list_rules(ws)?
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| "rule disappeared after toggle".into())
}

pub fn remove_rule(ws: &str, name: &str) -> Result<(), String> {
    let roots = ensure_skills_rules_roots(ws)?;
    validate_rule_name(name)?;
    let path = roots.rules.join(name);
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("remove rule: {e}"))?;
    }
    let mut idx = read_rules_index(&roots.rules);
    idx.rules.remove(name);
    write_rules_index(&roots.rules, &idx)
}

// ===========================================================================
// Skills
// ===========================================================================

fn skills_index_path(skills_dir: &Path) -> PathBuf {
    skills_dir.join(SKILLS_INDEX_FILE)
}

fn read_skills_index(skills_dir: &Path) -> SkillsIndex {
    read_or_default_json(&skills_index_path(skills_dir))
}

fn write_skills_index(skills_dir: &Path, idx: &SkillsIndex) -> Result<(), String> {
    atomic_write_json(&skills_index_path(skills_dir), idx)
}

fn list_skill_dirs(skills_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(read) = fs::read_dir(skills_dir) else {
        return out;
    };
    for entry in read.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if validate_skill_name(&name).is_ok() {
            out.push(name);
        }
    }
    out.sort();
    out
}

pub fn list_skills(ws: &str) -> Result<Vec<SkillEntry>, String> {
    let roots = ensure_skills_rules_roots(ws)?;
    let dirs = list_skill_dirs(&roots.skills);
    let mut idx = read_skills_index(&roots.skills);

    // Self-heal: drop user-skill entries with no corresponding directory.
    // Core skill names are excluded from the self-heal check.
    let core_names: std::collections::BTreeSet<&str> =
        CORE_SKILLS.iter().map(|(n, _)| *n).collect();
    let known: std::collections::BTreeSet<_> = dirs.iter().cloned().collect();
    let stale: Vec<_> = idx
        .skills
        .keys()
        .filter(|k| !known.contains(*k) && !core_names.contains(k.as_str()))
        .cloned()
        .collect();
    let dirty = !stale.is_empty();
    for k in stale {
        idx.skills.remove(&k);
    }
    if dirty {
        let _ = write_skills_index(&roots.skills, &idx);
    }

    // Prepend core skills.
    let core_source = SkillSourceMeta {
        kind: SkillSourceKind::Core,
        url: None,
        git_ref: None,
        package: None,
        version: None,
        path: None,
    };
    let mut entries: Vec<SkillEntry> = CORE_SKILLS
        .iter()
        .map(|(name, content)| {
            let (title, summary) = extract_title_and_summary(content, name);
            let enabled = idx
                .skills
                .get(*name)
                .map(|e| e.enabled)
                .unwrap_or(true);
            SkillEntry {
                name: name.to_string(),
                title,
                summary,
                enabled,
                source: core_source.clone(),
                installed_at: CORE_INSTALLED_AT.to_string(),
                updated_at: CORE_INSTALLED_AT.to_string(),
                missing_skill_md: false,
            }
        })
        .collect();

    // Append user-installed skills.
    for name in dirs {
        let dir = roots.skills.join(&name);
        let doc_path = dir.join(SKILL_DOC);
        let body = fs::read_to_string(&doc_path).unwrap_or_default();
        let missing = body.is_empty() && !doc_path.is_file();
        let (title, summary) = extract_title_and_summary(&body, &name);

        let (source, installed_at, updated_at, enabled) = match idx.skills.get(&name) {
            Some(e) => (
                e.source.clone(),
                e.installed_at.clone(),
                e.updated_at.clone(),
                e.enabled,
            ),
            None => (
                SkillSourceMeta {
                    kind: SkillSourceKind::Local,
                    url: None,
                    git_ref: None,
                    package: None,
                    version: None,
                    path: None,
                },
                fs::metadata(&dir)
                    .ok()
                    .map(|m| modified_rfc3339(&m))
                    .unwrap_or_default(),
                fs::metadata(&doc_path)
                    .or_else(|_| fs::metadata(&dir))
                    .ok()
                    .map(|m| modified_rfc3339(&m))
                    .unwrap_or_default(),
                true,
            ),
        };

        entries.push(SkillEntry {
            name,
            title,
            summary,
            enabled,
            source,
            installed_at,
            updated_at,
            missing_skill_md: missing,
        });
    }
    Ok(entries)
}

pub fn read_skill(ws: &str, name: &str) -> Result<String, String> {
    validate_skill_name(name)?;
    // Serve core skill content from embedded binary.
    if let Some((_, content)) = CORE_SKILLS.iter().find(|(n, _)| *n == name) {
        return Ok(content.to_string());
    }
    let roots = ensure_skills_rules_roots(ws)?;
    let path = roots.skills.join(name).join(SKILL_DOC);
    fs::read_to_string(&path).map_err(|e| format!("read skill: {e}"))
}

pub fn write_skill(ws: &str, name: &str, content: &str) -> Result<SkillEntry, String> {
    let roots = ensure_skills_rules_roots(ws)?;
    validate_skill_name(name)?;
    let dir = roots.skills.join(name);
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir skill: {e}"))?;
    let doc_path = dir.join(SKILL_DOC);
    let existed = doc_path.is_file();
    fs::write(&doc_path, content.as_bytes()).map_err(|e| format!("write SKILL.md: {e}"))?;

    let mut idx = read_skills_index(&roots.skills);
    let now = now_rfc3339();
    idx.skills
        .entry(name.to_owned())
        .and_modify(|e| {
            e.updated_at = now.clone();
        })
        .or_insert(SkillIndexEntry {
            enabled: true,
            source: SkillSourceMeta {
                kind: SkillSourceKind::AgentCreated,
                url: None,
                git_ref: None,
                package: None,
                version: None,
                path: None,
            },
            installed_at: now.clone(),
            updated_at: now.clone(),
        });
    write_skills_index(&roots.skills, &idx)?;

    let _ = existed;
    list_skills(ws)?
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| "skill disappeared after write".into())
}

pub fn set_skill_enabled(ws: &str, name: &str, enabled: bool) -> Result<SkillEntry, String> {
    validate_skill_name(name)?;
    let roots = ensure_skills_rules_roots(ws)?;
    let is_core = CORE_SKILLS.iter().any(|(n, _)| *n == name);
    if !is_core && !roots.skills.join(name).is_dir() {
        return Err(format!("skill not found: {name}"));
    }
    let mut idx = read_skills_index(&roots.skills);
    let now = now_rfc3339();
    idx.skills
        .entry(name.to_owned())
        .and_modify(|e| {
            e.enabled = enabled;
            e.updated_at = now.clone();
        })
        .or_insert(SkillIndexEntry {
            enabled,
            source: SkillSourceMeta {
                kind: SkillSourceKind::Local,
                url: None,
                git_ref: None,
                package: None,
                version: None,
                path: None,
            },
            installed_at: now.clone(),
            updated_at: now.clone(),
        });
    write_skills_index(&roots.skills, &idx)?;
    list_skills(ws)?
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| "skill disappeared after toggle".into())
}

pub fn remove_skill(ws: &str, name: &str) -> Result<(), String> {
    validate_skill_name(name)?;
    if CORE_SKILLS.iter().any(|(n, _)| *n == name) {
        return Err(format!("core skill '{name}' cannot be removed"));
    }
    let roots = ensure_skills_rules_roots(ws)?;
    let dir = roots.skills.join(name);
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|e| format!("remove skill dir: {e}"))?;
    }
    let mut idx = read_skills_index(&roots.skills);
    idx.skills.remove(name);
    write_skills_index(&roots.skills, &idx)
}

/// Used by `install::*` to commit a freshly populated skill folder.
pub fn record_installed_skill(
    ws: &str,
    name: &str,
    source: SkillSourceMeta,
) -> Result<SkillEntry, String> {
    let roots = ensure_skills_rules_roots(ws)?;
    validate_skill_name(name)?;
    let dir = roots.skills.join(name);
    if !dir.is_dir() {
        return Err("install completed but skill folder is missing".into());
    }
    let now = now_rfc3339();
    let mut idx = read_skills_index(&roots.skills);
    idx.skills.insert(
        name.to_owned(),
        SkillIndexEntry {
            enabled: true,
            source,
            installed_at: now.clone(),
            updated_at: now,
        },
    );
    write_skills_index(&roots.skills, &idx)?;
    list_skills(ws)?
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| "skill missing from listing after install".into())
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents_layout::AGENTS_REL;
    use crate::skills_rules::types::SkillSourceKind;

    fn fresh_ws(tag: &str) -> PathBuf {
        let ws = std::env::temp_dir().join(format!(
            "blxcode_sr_{}_{}_{}",
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

    fn ws_str(p: &Path) -> String {
        p.to_string_lossy().to_string()
    }

    #[test]
    fn rule_name_validation() {
        assert!(validate_rule_name("rule-foo.md").is_ok());
        assert!(validate_rule_name("rule-foo_bar-baz.md").is_ok());
        assert!(validate_rule_name("foo.md").is_err());
        assert!(validate_rule_name("rule-.md").is_err());
        assert!(validate_rule_name("rule-foo").is_err());
        assert!(validate_rule_name("rule-foo.md/").is_err());
        assert!(validate_rule_name("../rule-foo.md").is_err());
        assert!(validate_rule_name("rule-foo bar.md").is_err());
    }

    #[test]
    fn skill_name_validation() {
        assert!(validate_skill_name("leptos-guide").is_ok());
        assert!(validate_skill_name("a").is_ok());
        assert!(validate_skill_name("9skill").is_ok());
        assert!(validate_skill_name("Skill").is_err()); // uppercase
        assert!(validate_skill_name("-bad").is_err()); // leading dash
        assert!(validate_skill_name("../escape").is_err());
        assert!(validate_skill_name("with/slash").is_err());
    }

    #[test]
    fn empty_workspace_lists_empty() {
        let ws = fresh_ws("empty_list");
        let rules = list_rules(&ws_str(&ws)).unwrap();
        let skills = list_skills(&ws_str(&ws)).unwrap();
        assert!(rules.is_empty());
        // Only core skills are present; no user-installed skills.
        let user_skills: Vec<_> = skills
            .iter()
            .filter(|s| !matches!(s.source.kind, SkillSourceKind::Core))
            .collect();
        assert!(user_skills.is_empty());
        assert_eq!(skills.len(), CORE_SKILLS.len());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn rule_default_enabled_when_no_index() {
        let ws = fresh_ws("rule_default");
        let roots = ensure_skills_rules_roots(&ws_str(&ws)).unwrap();
        fs::write(roots.rules.join("rule-foo.md"), "# Foo\n\nBody").unwrap();
        let list = list_rules(&ws_str(&ws)).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].enabled);
        assert_eq!(list[0].name, "rule-foo.md");
        assert_eq!(list[0].title, "Foo");
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn rule_toggle_persists_and_is_idempotent() {
        let ws = fresh_ws("rule_toggle");
        let roots = ensure_skills_rules_roots(&ws_str(&ws)).unwrap();
        fs::write(roots.rules.join("rule-foo.md"), "body").unwrap();
        let e1 = set_rule_enabled(&ws_str(&ws), "rule-foo.md", false).unwrap();
        assert!(!e1.enabled);
        let e2 = set_rule_enabled(&ws_str(&ws), "rule-foo.md", false).unwrap();
        assert!(!e2.enabled);
        let e3 = set_rule_enabled(&ws_str(&ws), "rule-foo.md", true).unwrap();
        assert!(e3.enabled);
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn rule_index_self_heal_drops_missing_files() {
        let ws = fresh_ws("rule_selfheal");
        let roots = ensure_skills_rules_roots(&ws_str(&ws)).unwrap();
        // Fake an index entry for a file that does not exist.
        let mut idx = RulesIndex::default();
        idx.rules.insert(
            "rule-ghost.md".to_owned(),
            RuleIndexEntry {
                enabled: true,
                updated_at: now_rfc3339(),
            },
        );
        write_rules_index(&roots.rules, &idx).unwrap();

        let list = list_rules(&ws_str(&ws)).unwrap();
        assert!(list.is_empty());
        let after = read_rules_index(&roots.rules);
        assert!(after.rules.is_empty());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn skill_default_enabled_with_source_local() {
        let ws = fresh_ws("skill_default");
        let roots = ensure_skills_rules_roots(&ws_str(&ws)).unwrap();
        let dir = roots.skills.join("leptos-guide");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "# Leptos Guide\n\nhints").unwrap();
        let list = list_skills(&ws_str(&ws)).unwrap();
        assert_eq!(list.len(), CORE_SKILLS.len() + 1);
        let user = list.iter().find(|s| s.name == "leptos-guide").unwrap();
        assert!(user.enabled);
        assert!(matches!(user.source.kind, SkillSourceKind::Local));
        assert!(!user.missing_skill_md);
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn skill_missing_skill_md_flagged() {
        let ws = fresh_ws("skill_missing");
        let roots = ensure_skills_rules_roots(&ws_str(&ws)).unwrap();
        fs::create_dir_all(roots.skills.join("foo")).unwrap();
        let list = list_skills(&ws_str(&ws)).unwrap();
        assert_eq!(list.len(), CORE_SKILLS.len() + 1);
        let user = list.iter().find(|s| s.name == "foo").unwrap();
        assert!(user.missing_skill_md);
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn skill_write_marks_agent_created_source() {
        let ws = fresh_ws("skill_write");
        let e = write_skill(&ws_str(&ws), "new-skill", "# Title\nbody").unwrap();
        assert!(matches!(e.source.kind, SkillSourceKind::AgentCreated));
        assert!(e.enabled);
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn remove_skill_clears_index() {
        let ws = fresh_ws("skill_remove");
        write_skill(&ws_str(&ws), "skill-a", "# a").unwrap();
        remove_skill(&ws_str(&ws), "skill-a").unwrap();
        let list = list_skills(&ws_str(&ws)).unwrap();
        // Only core skills remain after removing the user skill.
        let user_skills: Vec<_> = list
            .iter()
            .filter(|s| !matches!(s.source.kind, SkillSourceKind::Core))
            .collect();
        assert!(user_skills.is_empty());
        assert_eq!(list.len(), CORE_SKILLS.len());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn pathlike_names_rejected() {
        let ws = fresh_ws("sandbox");
        assert!(read_rule(&ws_str(&ws), "../escape").is_err());
        assert!(write_skill(&ws_str(&ws), "../etc", "x").is_err());
        assert!(set_rule_enabled(&ws_str(&ws), "rule-a/b.md", true).is_err());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn bootstrap_writes_index_files_with_existing_content_enabled() {
        let ws = fresh_ws("bootstrap");
        // Seed `.agents/{rules,skills}/` BEFORE the first `ensure_*` call so
        // the bootstrap step can pick them up.
        let agents = ws.join(AGENTS_REL);
        let rules_dir = ws.join(RULES_REL);
        let skills_dir = ws.join(SKILLS_REL);
        fs::create_dir_all(&agents).unwrap();
        fs::create_dir_all(&rules_dir).unwrap();
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(rules_dir.join("rule-alpha.md"), "# Alpha").unwrap();
        fs::write(rules_dir.join("rule-beta.md"), "# Beta").unwrap();
        fs::create_dir_all(skills_dir.join("seed-a")).unwrap();
        fs::write(skills_dir.join("seed-a/SKILL.md"), "# Seed A").unwrap();
        fs::create_dir_all(skills_dir.join("seed-b")).unwrap();

        // First touch — must materialise the manifests.
        let _roots = ensure_skills_rules_roots(&ws_str(&ws)).unwrap();
        assert!(rules_dir.join("index.json").is_file());
        assert!(skills_dir.join("index.json").is_file());

        let rules = list_rules(&ws_str(&ws)).unwrap();
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().all(|r| r.enabled));
        let skills = list_skills(&ws_str(&ws)).unwrap();
        // Core skills are always prepended; user-installed skills follow.
        let user_skills: Vec<_> = skills
            .iter()
            .filter(|s| !matches!(s.source.kind, SkillSourceKind::Core))
            .collect();
        assert_eq!(user_skills.len(), 2);
        assert!(user_skills.iter().all(|s| s.enabled));
        assert!(user_skills
            .iter()
            .all(|s| matches!(s.source.kind, SkillSourceKind::Local)));

        // A second touch must NOT clobber a manually-edited manifest:
        // disable one, re-bootstrap, expect the disabled flag to survive.
        set_rule_enabled(&ws_str(&ws), "rule-beta.md", false).unwrap();
        let _ = ensure_skills_rules_roots(&ws_str(&ws)).unwrap();
        let rules_after = list_rules(&ws_str(&ws)).unwrap();
        let beta = rules_after.iter().find(|r| r.name == "rule-beta.md").unwrap();
        assert!(!beta.enabled, "bootstrap must be a no-op when index exists");
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn extract_title_strips_h1() {
        let (t, s) = extract_title_and_summary("# Foo\n\nbar baz", "fallback");
        assert_eq!(t, "Foo");
        assert_eq!(s, "bar baz");
        let (t2, _s2) = extract_title_and_summary("no heading text", "fallback");
        assert_eq!(t2, "fallback");
    }
}
