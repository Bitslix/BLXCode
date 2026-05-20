//! Workspace `.agents/` bootstrap, legacy memory migration, and learnings wikilink upgrades.

use std::fs;
use std::path::{Path, PathBuf};

pub const AGENTS_REL: &str = ".agents";
pub const MEMORY_REL: &str = ".agents/memory";
pub const LEARNINGS_REL: &str = ".agents/learnings";
pub const PLANS_REL: &str = ".agents/plans";
pub const LEGACY_MEMORY_REL: &str = ".blxcode/memory";
pub const LEARNINGS_API_PREFIX: &str = "learnings/";
pub const PLANS_INDEX: &str = "PLANS.md";
const TEMPLATES_DIRNAME: &str = "_templates";

const LEARNINGS_INDEX: &str = "LEARNINGS.md";
const LEARNINGS_INDEX_TYPO: &str = "LEARNIGS.md";

const PLANS_SEED: &str = r#"# Plans

This directory holds durable Markdown plans for AI coding agents working on
this repository. Each plan lives in its own Markdown file under
`.agents/plans/`. Plans are the structured Markdown counterpart to the
short-lived task list — they are checked into git and survive across
sessions.

Keep this file as the overview and index. Reference each plan with a
relative Markdown link, one line per plan.

## Index

_(Add plans here as `[Short title](plan-filename.md)` — one line per plan.)_
"#;

const LEARNINGS_SEED: &str = r#"# Learnings

This directory is the persistent knowledge base for AI coding agents working on
this repository. Use it to capture facts, decisions, conventions, pitfalls, and
resolved mistakes that are useful beyond a single task.

Keep this file as the overview and index. Store individual learnings in separate
Markdown files inside `.agents/learnings/`.

## Index

_(Add learnings here as `[[learnings/topic-filename|Short title]]` — one line per topic.)_
"#;

#[derive(Debug, Clone)]
pub struct WorkspaceRoots {
    pub memory: PathBuf,
    pub learnings: PathBuf,
    pub plans: PathBuf,
}

pub fn validate_workspace_cwd(ws: &str) -> Result<PathBuf, String> {
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

/// Creates `.agents/memory`, `.agents/learnings`, migrates legacy memory, seeds index, upgrades wikilinks.
pub fn ensure_agents_layout(ws: &str) -> Result<WorkspaceRoots, String> {
    let ws_path = validate_workspace_cwd(ws)?;
    let agents = ws_path.join(AGENTS_REL);
    let memory = ws_path.join(MEMORY_REL);
    let learnings = ws_path.join(LEARNINGS_REL);
    let plans = ws_path.join(PLANS_REL);
    let templates = memory.join(TEMPLATES_DIRNAME);

    fs::create_dir_all(&agents).map_err(|e| format!("create {AGENTS_REL}: {e}"))?;
    fs::create_dir_all(&memory).map_err(|e| format!("create {MEMORY_REL}: {e}"))?;
    fs::create_dir_all(&templates).map_err(|e| format!("create templates: {e}"))?;
    fs::create_dir_all(&learnings).map_err(|e| format!("create {LEARNINGS_REL}: {e}"))?;
    fs::create_dir_all(&plans).map_err(|e| format!("create {PLANS_REL}: {e}"))?;

    migrate_legacy_memory(&ws_path, &memory)?;
    seed_learnings_index_if_empty(&learnings)?;
    fix_learnings_index_typo(&learnings)?;
    upgrade_learnings_graph_links(&learnings)?;
    seed_plans_index_if_missing(&plans)?;

    Ok(WorkspaceRoots {
        memory,
        learnings,
        plans,
    })
}

fn seed_plans_index_if_missing(plans: &Path) -> Result<(), String> {
    let index = plans.join(PLANS_INDEX);
    if index.exists() {
        return Ok(());
    }
    fs::write(&index, PLANS_SEED.as_bytes()).map_err(|e| format!("write {PLANS_INDEX}: {e}"))
}

fn dir_has_any_md(dir: &Path) -> bool {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(read) = fs::read_dir(&d) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                stack.push(path);
                continue;
            }
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
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

fn migrate_legacy_memory(ws: &Path, memory: &Path) -> Result<(), String> {
    let legacy = ws.join(LEGACY_MEMORY_REL);
    if !legacy.is_dir() {
        return Ok(());
    }
    if dir_has_any_md(memory) {
        return Ok(());
    }
    if !dir_has_any_md(&legacy) {
        return Ok(());
    }
    copy_dir_recursive(&legacy, memory)
}

fn seed_learnings_index_if_empty(learnings: &Path) -> Result<(), String> {
    if dir_has_any_md(learnings) {
        return Ok(());
    }
    let index = learnings.join(LEARNINGS_INDEX);
    fs::write(&index, LEARNINGS_SEED.as_bytes())
        .map_err(|e| format!("write {LEARNINGS_INDEX}: {e}"))
}

fn fix_learnings_index_typo(learnings: &Path) -> Result<(), String> {
    let typo = learnings.join(LEARNINGS_INDEX_TYPO);
    let correct = learnings.join(LEARNINGS_INDEX);
    if typo.is_file() && !correct.exists() {
        fs::rename(&typo, &correct).map_err(|e| format!("rename index typo: {e}"))?;
    }
    Ok(())
}

fn learnings_index_stem(learnings: &Path) -> Option<String> {
    if learnings.join(LEARNINGS_INDEX).is_file() {
        return Some("LEARNINGS".to_owned());
    }
    if learnings.join(LEARNINGS_INDEX_TYPO).is_file() {
        return Some("LEARNIGS".to_owned());
    }
    None
}

fn is_index_file(name: &str) -> bool {
    name.eq_ignore_ascii_case(LEARNINGS_INDEX) || name.eq_ignore_ascii_case(LEARNINGS_INDEX_TYPO)
}

/// Idempotently converts `[label](topic.md)` to `[[learnings/topic|label]]` outside fenced code.
pub fn upgrade_learnings_graph_links(learnings: &Path) -> Result<(), String> {
    let index_stem = learnings_index_stem(learnings);
    let mut files = Vec::new();
    collect_md_files(learnings, &mut files);
    for path in &files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        let upgraded = upgrade_markdown_body(&body, is_index_file(name));
        if upgraded != body {
            fs::write(path, upgraded.as_bytes())
                .map_err(|e| format!("write {}: {e}", path.display()))?;
        }
    }
    if let Some(ref stem) = index_stem {
        for path in files {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if is_index_file(name) {
                continue;
            }
            ensure_topic_backlink_to_index(&path, stem)?;
        }
    }
    Ok(())
}

fn collect_md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            collect_md_files(&path, out);
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

fn md_link_target_to_wikilink(target: &str) -> Option<String> {
    let t = target.trim();
    if t.is_empty() || t.contains("://") || t.starts_with('#') {
        return None;
    }
    let path_part = t.split('#').next().unwrap_or(t).trim();
    if !path_part.to_ascii_lowercase().ends_with(".md") {
        return None;
    }
    let stem = path_part
        .trim_end_matches(".md")
        .trim_end_matches(".MD");
    if stem.is_empty() || stem.contains("..") || stem.starts_with('/') {
        return None;
    }
    let api_stem = if stem.contains('/') {
        format!("{LEARNINGS_API_PREFIX}{}", stem.trim_start_matches('/'))
    } else {
        format!("{LEARNINGS_API_PREFIX}{stem}")
    };
    Some(api_stem)
}

fn line_has_wikilink_to(line: &str, api_stem: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let needle = format!("[[{api_stem}");
    lower.contains(&needle.to_ascii_lowercase())
        || lower.contains(&format!("[[{}/", api_stem.to_ascii_lowercase()))
}

fn upgrade_markdown_body(body: &str, _is_index: bool) -> String {
    let mut out = String::with_capacity(body.len() + 32);
    let mut fenced = false;
    for line in body.split_inclusive('\n') {
        let (line_body, nl) = if let Some(b) = line.strip_suffix('\n') {
            (b, true)
        } else {
            (line, false)
        };
        let trim = line_body.trim_start();
        if trim.starts_with("```") {
            fenced = !fenced;
            out.push_str(line_body);
            if nl {
                out.push('\n');
            }
            continue;
        }
        if fenced {
            out.push_str(line_body);
            if nl {
                out.push('\n');
            }
            continue;
        }
        out.push_str(&upgrade_markdown_links_in_line(line_body));
        if nl {
            out.push('\n');
        }
    }
    out
}

fn upgrade_markdown_links_in_line(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len() + 16);
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some(close_bracket) = line[i + 1..].find(']') {
                let label_end = i + 1 + close_bracket;
                if label_end + 1 < bytes.len() && bytes[label_end + 1] == b'(' {
                    if let Some(close_paren) = line[label_end + 2..].find(')') {
                        let label = &line[i + 1..label_end];
                        let target = &line[label_end + 2..label_end + 2 + close_paren];
                        if let Some(api_stem) = md_link_target_to_wikilink(target) {
                            if !line_has_wikilink_to(line, &api_stem) {
                                out.push_str("[[");
                                out.push_str(&api_stem);
                                out.push('|');
                                out.push_str(label);
                                out.push_str("]]");
                                i = label_end + 2 + close_paren + 1;
                                continue;
                            }
                        }
                    }
                }
            }
        }
        let ch = line[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn body_links_to_index(body: &str, index_stem: &str) -> bool {
    let index_api = format!("{LEARNINGS_API_PREFIX}{index_stem}");
    for line in body.lines() {
        if line_has_wikilink_to(line, &index_api) || line_has_wikilink_to(line, index_stem) {
            return true;
        }
    }
    false
}

fn ensure_topic_backlink_to_index(path: &Path, index_stem: &str) -> Result<(), String> {
    let Ok(mut body) = fs::read_to_string(path) else {
        return Ok(());
    };
    if body_links_to_index(&body, index_stem) {
        return Ok(());
    }
    let index_api = format!("{LEARNINGS_API_PREFIX}{index_stem}");
    if !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str("\n## Related\n\n");
    body.push_str(&format!("[[{index_api}]]\n"));
    fs::write(path, body.as_bytes()).map_err(|e| format!("write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md_link_converts_to_wikilink() {
        let line = "- [shadcn Tailwind Setup](shadcn-tailwind.md) — note";
        let out = upgrade_markdown_links_in_line(line);
        assert!(out.contains("[[learnings/shadcn-tailwind|shadcn Tailwind Setup]]"));
    }

    #[test]
    fn md_link_upgrade_idempotent() {
        let line = "- [[learnings/foo|Foo]] and [Foo](foo.md)";
        let out = upgrade_markdown_links_in_line(line);
        assert_eq!(out, line);
    }

    #[test]
    fn migrate_legacy_only_when_new_empty() {
        let ws = std::env::temp_dir().join(format!(
            "blxcode_agents_migrate_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&ws);
        fs::create_dir_all(ws.join(LEGACY_MEMORY_REL)).unwrap();
        fs::write(ws.join(LEGACY_MEMORY_REL).join("note.md"), "legacy").unwrap();

        let roots = ensure_agents_layout(&ws.to_string_lossy()).unwrap();
        assert_eq!(
            fs::read_to_string(roots.memory.join("note.md")).unwrap(),
            "legacy"
        );

        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn wikilink_upgrade_creates_graph_edge_fixture() {
        let ws = std::env::temp_dir().join(format!(
            "blxcode_agents_wiki_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&ws);
        let learnings = ws.join(LEARNINGS_REL);
        fs::create_dir_all(&learnings).unwrap();
        fs::write(
            learnings.join(LEARNINGS_INDEX),
            "## Index\n\n- [Foo](foo.md) — test\n",
        )
        .unwrap();
        fs::write(learnings.join("foo.md"), "# Foo\n").unwrap();

        upgrade_learnings_graph_links(&learnings).unwrap();
        let index = fs::read_to_string(learnings.join(LEARNINGS_INDEX)).unwrap();
        assert!(index.contains("[[learnings/foo|Foo]]"));
        let topic = fs::read_to_string(learnings.join("foo.md")).unwrap();
        assert!(topic.contains("[[learnings/LEARNINGS]]"));

        let _ = fs::remove_dir_all(&ws);
    }
}
