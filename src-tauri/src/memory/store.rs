//! Core CRUD helpers — called from #[tauri::command] wrappers in mod.rs.

use crate::agents_layout::{ensure_agents_layout, LEARNINGS_API_PREFIX, LEARNINGS_REL, MEMORY_REL};
use std::collections::HashMap;
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Component, Path, PathBuf};

use super::frontmatter::{
    parse_frontmatter, serialize_frontmatter, strip_frontmatter, MemoryFrontmatter,
};
use super::graph::{build_graph, CategoryHubInput, ScopeNote};
use super::paths::{
    folder_exists, get_global_roots, get_roots_for_scope, graph_category_for,
    list_memory_subcategories, node_id, parse_node_id, validate_category_name, MemoryRoots,
    CATEGORY_PLACEHOLDER, TEMPLATES_DIRNAME,
};
use super::types::{
    BacklinkRef, GraphData, MemoryFolderStatus, MemoryListResponse, MemoryScope,
    MemoryStatusResponse, MemorySubcategories, NoteContent, NoteMeta, PointerResult, RenameReport,
    SearchHit,
};
use super::wikilinks::{parse_links_and_tags, rewrite_wikilinks};

// ── Bootstrap seed text ────────────────────────────────────────────────────────

const MEMORY_OVERVIEW_BODY: &str = r#"# Memory

This folder stores **durable, project-wide notes** that should survive across coding sessions and be available to every AI agent or teammate working in this repo.

## What belongs here

- Architectural decisions and the reasoning behind them
- Conventions, patterns and "house style" that aren't obvious from the code
- Reference material: external systems, where to find logs / dashboards
- Known-good workflows ("how we deploy", "how we cut a release")
- User-specific preferences for collaboration

## What does **not** belong here

- Ephemeral todos for the current task (use a Plan or task list instead)
- Things already documented in code, `CLAUDE.md`, or commit messages
- Time-sensitive snapshots (`git log` is authoritative for history)

## Tips for AI agents

- Read existing notes before answering questions about this project.
- When you learn something non-obvious, propose adding it here.
- Prefer updating an existing note over creating a near-duplicate.
"#;

const LEARNINGS_OVERVIEW_BODY: &str = r#"# Learnings

This folder is a **growing knowledge base of resolved problems** — concrete things you discovered the hard way and want future agents (human or AI) to find.

Each learning is a separate markdown file named `learning-<slug>.md`. Together they form a searchable log of "the thing we now know that we didn't know before."

## When to record a learning

- A bug you fixed that wasn't obvious from the symptom
- A workaround for a quirk in a dependency, runtime, or environment
- A migration step that required out-of-band knowledge

## Tips for AI agents

- Search here before debugging — someone may have hit this already.
- When you fix a non-trivial bug, propose a new learning entry.
- Keep one learning per file; cross-link related ones via `[[wikilinks]]`.
"#;

// ── Path helpers ───────────────────────────────────────────────────────────────

fn err<T>(s: impl Into<String>) -> Result<T, String> {
    Err(s.into())
}

fn mtime_secs(p: &Path) -> i64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn walk_md(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(root) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk_md(&path, out);
            continue;
        }
        if ft.is_file() {
            let is_md = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if is_md {
                out.push(path);
            }
        }
    }
}

fn rel_from_root(root: &Path, p: &Path) -> Option<String> {
    p.strip_prefix(root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

/// Returns (is_learnings, rel_within_root).
fn resolve_api_path(api_path: &str) -> Result<(bool, String), String> {
    let p = api_path.trim().replace('\\', "/");
    if p.is_empty() {
        return err("empty path");
    }
    if p.starts_with(LEARNINGS_API_PREFIX) {
        let rel = p
            .strip_prefix(LEARNINGS_API_PREFIX)
            .unwrap_or_default()
            .trim();
        if rel.is_empty() || rel.contains("..") {
            return err("invalid learnings path");
        }
        return Ok((true, rel.to_owned()));
    }
    if p.contains("..") {
        return err("invalid path");
    }
    Ok((false, p))
}

fn note_abs(roots: &MemoryRoots, api_path: &str) -> Result<PathBuf, String> {
    let (is_learnings, rel) = resolve_api_path(api_path)?;
    let root = if is_learnings {
        roots.learnings.as_path()
    } else {
        roots.memory.as_path()
    };
    safe_join(root, &rel, true)
}

fn safe_join(root: &Path, rel: &str, enforce_md: bool) -> Result<PathBuf, String> {
    let rel = rel.trim().trim_start_matches('/').trim_start_matches('\\');
    if rel.is_empty() {
        return err("empty relative path");
    }
    let normalized = rel.replace('\\', "/");
    let candidate = PathBuf::from(&normalized);
    for c in candidate.components() {
        match c {
            Component::Normal(_) => {}
            _ => return err(format!("disallowed path component in {rel}")),
        }
    }
    let abs = root.join(&candidate);
    let canon_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let canon_abs = abs
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        .map(|p| p.join(abs.file_name().unwrap_or_default()))
        .unwrap_or(abs.clone());
    if !canon_abs.starts_with(&canon_root) {
        return err("path escapes memory root");
    }
    if enforce_md {
        let is_md = canon_abs
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if !is_md {
            return err("only .md files allowed");
        }
    }
    Ok(abs)
}

// ── Note metadata collection ───────────────────────────────────────────────────

fn meta_from_file(scope: &MemoryScope, api_path: &str, abs: &Path) -> Option<NoteMeta> {
    let content = fs::read_to_string(abs).ok()?;
    let (fm, body_rest) = parse_frontmatter(&content);
    let (is_learnings, _) = resolve_api_path(api_path).ok()?;
    let file_name = Path::new(api_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();
    let basename = Path::new(api_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();
    let is_overview = basename.eq_ignore_ascii_case("README.md");
    let is_template = !is_learnings && api_path.starts_with(&format!("{TEMPLATES_DIRNAME}/"));
    let title = fm
        .title
        .clone()
        .unwrap_or_else(|| extract_h1_title(&body_rest).unwrap_or_else(|| file_name.clone()));
    let meta = fs::metadata(abs).ok();
    Some(NoteMeta {
        scope: scope.clone(),
        path: api_path.to_owned(),
        name: file_name,
        title,
        enabled: fm.enabled.unwrap_or(true),
        tags: fm.tags.unwrap_or_default(),
        size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        modified: mtime_secs(abs),
        is_template,
        is_learning: is_learnings,
        is_overview,
        category: graph_category_for(api_path),
    })
}

fn extract_h1_title(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let t = rest.trim();
            if !t.is_empty() {
                return Some(t.to_owned());
            }
        }
    }
    None
}

pub fn collect_notes(scope: &MemoryScope, workspace_cwd: &str) -> Vec<NoteMeta> {
    let Ok(roots) = get_roots_for_scope(scope, workspace_cwd) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (is_learnings, root) in [
        (false, roots.memory.as_path()),
        (true, roots.learnings.as_path()),
    ] {
        let mut files = Vec::new();
        walk_md(root, &mut files);
        for abs in files {
            let Some(rel) = rel_from_root(root, &abs) else {
                continue;
            };
            let api = if is_learnings {
                format!("{LEARNINGS_API_PREFIX}{rel}")
            } else {
                rel
            };
            if let Some(meta) = meta_from_file(scope, &api, &abs) {
                out.push(meta);
            }
        }
    }
    out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    out
}

// ── Status & Bootstrap ────────────────────────────────────────────────────────

pub fn memory_status_impl(workspace_cwd: &str) -> MemoryStatusResponse {
    let global = get_global_roots();
    let ws = get_roots_for_scope(&MemoryScope::Workspace, workspace_cwd).ok();
    MemoryStatusResponse {
        workspace: MemoryFolderStatus {
            memory: ws
                .as_ref()
                .map(|r| folder_exists(&r.memory))
                .unwrap_or(false),
            learnings: ws
                .as_ref()
                .map(|r| folder_exists(&r.learnings))
                .unwrap_or(false),
        },
        global: MemoryFolderStatus {
            memory: folder_exists(&global.memory),
            learnings: folder_exists(&global.learnings),
        },
    }
}

fn seed_memory(memory_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(memory_dir).map_err(|e| format!("mkdir: {e}"))?;
    let readme = memory_dir.join("README.md");
    if !readme.exists() {
        let fm = MemoryFrontmatter {
            title: Some("Memory".into()),
            enabled: Some(true),
            tags: Some(Vec::new()),
        };
        let content = serialize_frontmatter(&fm, MEMORY_OVERVIEW_BODY);
        fs::write(&readme, content.as_bytes()).map_err(|e| format!("write README: {e}"))?;
    }
    Ok(())
}

fn seed_learnings(learnings_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(learnings_dir).map_err(|e| format!("mkdir: {e}"))?;
    let readme = learnings_dir.join("README.md");
    if !readme.exists() {
        let fm = MemoryFrontmatter {
            title: Some("Learnings".into()),
            enabled: Some(true),
            tags: Some(Vec::new()),
        };
        let content = serialize_frontmatter(&fm, LEARNINGS_OVERVIEW_BODY);
        fs::write(&readme, content.as_bytes()).map_err(|e| format!("write README: {e}"))?;
    }
    Ok(())
}

pub fn memory_bootstrap_impl(target: &str, workspace_cwd: &str) -> Result<(), String> {
    match target {
        "workspace" | "all" => {
            let roots = get_roots_for_scope(&MemoryScope::Workspace, workspace_cwd)?;
            seed_memory(&roots.memory)?;
            seed_learnings(&roots.learnings)?;
            if target == "workspace" {
                return Ok(());
            }
            // fall through for "all"
            let global = get_global_roots();
            if let Some(parent) = global.memory.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("mkdir global base: {e}"))?;
            }
            seed_memory(&global.memory)?;
            seed_learnings(&global.learnings)?;
        }
        "global" => {
            let global = get_global_roots();
            if let Some(parent) = global.memory.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("mkdir global base: {e}"))?;
            }
            seed_memory(&global.memory)?;
            seed_learnings(&global.learnings)?;
        }
        _ => return err(format!("unknown target: {target}")),
    }
    Ok(())
}

// ── memory_list ────────────────────────────────────────────────────────────────

pub fn memory_list_impl(workspace_cwd: &str) -> MemoryListResponse {
    // Workspace scope
    let ws_notes = collect_notes(&MemoryScope::Workspace, workspace_cwd);
    let ws_cats = get_roots_for_scope(&MemoryScope::Workspace, workspace_cwd)
        .map(|r| list_memory_subcategories(&r.memory))
        .unwrap_or_default();

    // Global scope
    let global_roots = get_global_roots();
    let global_notes =
        if folder_exists(&global_roots.memory) || folder_exists(&global_roots.learnings) {
            // collect_notes for global doesn't need workspace_cwd but the function signature takes it;
            // for global we can pass an empty string and use get_global_roots directly
            collect_notes_global()
        } else {
            Vec::new()
        };
    let global_cats = list_memory_subcategories(&global_roots.memory);

    let mut all_notes = ws_notes;
    all_notes.extend(global_notes);

    MemoryListResponse {
        notes: all_notes,
        memory_subcategories: MemorySubcategories {
            workspace: ws_cats,
            global: global_cats,
        },
    }
}

fn collect_notes_global() -> Vec<NoteMeta> {
    let roots = get_global_roots();
    let mut out = Vec::new();
    for (is_learnings, root) in [
        (false, roots.memory.as_path()),
        (true, roots.learnings.as_path()),
    ] {
        let mut files = Vec::new();
        walk_md(root, &mut files);
        for abs in files {
            let Some(rel) = rel_from_root(root, &abs) else {
                continue;
            };
            let api = if is_learnings {
                format!("{LEARNINGS_API_PREFIX}{rel}")
            } else {
                rel
            };
            if let Some(meta) = meta_from_file(&MemoryScope::Global, &api, &abs) {
                out.push(meta);
            }
        }
    }
    out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    out
}

// ── CRUD ──────────────────────────────────────────────────────────────────────

pub fn memory_read_impl(
    scope: &MemoryScope,
    workspace_cwd: &str,
    path: &str,
) -> Result<NoteContent, String> {
    let roots = get_roots_for_scope(scope, workspace_cwd)?;
    let abs = note_abs(&roots, path)?;
    let content = fs::read_to_string(&abs).map_err(|e| format!("read {path}: {e}"))?;
    Ok(NoteContent {
        scope: scope.clone(),
        path: path.to_owned(),
        content,
        modified: mtime_secs(&abs),
    })
}

pub fn memory_write_impl(
    scope: &MemoryScope,
    workspace_cwd: &str,
    path: &str,
    content: &str,
) -> Result<NoteContent, String> {
    let roots = get_roots_for_scope(scope, workspace_cwd)?;
    let abs = note_abs(&roots, path)?;
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let mut file = fs::File::create(&abs).map_err(|e| format!("create {path}: {e}"))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("write {path}: {e}"))?;
    Ok(NoteContent {
        scope: scope.clone(),
        path: path.to_owned(),
        content: content.to_owned(),
        modified: mtime_secs(&abs),
    })
}

pub fn memory_create_impl(
    scope: &MemoryScope,
    workspace_cwd: &str,
    path: &str,
    content: Option<String>,
) -> Result<NoteMeta, String> {
    let roots = get_roots_for_scope(scope, workspace_cwd)?;
    let abs = note_abs(&roots, path)?;
    if abs.exists() {
        return err(format!("already exists: {path}"));
    }
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let body = content.unwrap_or_default();
    fs::write(&abs, body.as_bytes()).map_err(|e| format!("write {path}: {e}"))?;
    meta_from_file(scope, path, &abs).ok_or_else(|| format!("failed to read meta for {path}"))
}

pub fn memory_delete_impl(
    scope: &MemoryScope,
    workspace_cwd: &str,
    path: &str,
) -> Result<(), String> {
    let roots = get_roots_for_scope(scope, workspace_cwd)?;
    let (is_learnings, _) = resolve_api_path(path)?;
    let root = if is_learnings {
        &roots.learnings
    } else {
        &roots.memory
    };
    let abs = note_abs(&roots, path)?;
    if !abs.exists() {
        return err(format!("not found: {path}"));
    }
    fs::remove_file(&abs).map_err(|e| format!("delete {path}: {e}"))?;
    // Remove empty parent dirs up to root
    if let Some(mut parent) = abs.parent() {
        while parent != root.as_path() {
            if fs::read_dir(parent)
                .map(|mut r| r.next().is_none())
                .unwrap_or(false)
            {
                let _ = fs::remove_dir(parent);
                if let Some(grand) = parent.parent() {
                    parent = grand;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
    Ok(())
}

pub fn memory_rename_impl(
    scope: &MemoryScope,
    workspace_cwd: &str,
    old_path: &str,
    new_path: &str,
    rewrite_links: bool,
) -> Result<RenameReport, String> {
    let roots = get_roots_for_scope(scope, workspace_cwd)?;
    let (old_learn, _) = resolve_api_path(old_path)?;
    let (new_learn, _) = resolve_api_path(new_path)?;
    if old_learn != new_learn {
        return err("cannot rename across memory and learnings roots");
    }
    let abs_old = note_abs(&roots, old_path)?;
    let abs_new = note_abs(&roots, new_path)?;
    if !abs_old.exists() {
        return err(format!("not found: {old_path}"));
    }
    if abs_new.exists() {
        return err(format!("already exists: {new_path}"));
    }
    if let Some(parent) = abs_new.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    fs::rename(&abs_old, &abs_new).map_err(|e| format!("rename: {e}"))?;

    let old_basename = Path::new(old_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();
    let new_basename = Path::new(new_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();

    let mut total_rewrites = 0u32;
    let mut files_changed = 0u32;

    if rewrite_links && !old_basename.is_empty() && !new_basename.is_empty() {
        let mut all_files = Vec::new();
        walk_md(&roots.memory, &mut all_files);
        walk_md(&roots.learnings, &mut all_files);
        for f in all_files {
            let Ok(content) = fs::read_to_string(&f) else {
                continue;
            };
            let (updated, n) =
                rewrite_wikilinks(&content, &old_basename, &new_basename, old_path, new_path);
            if n > 0 {
                if fs::write(&f, updated.as_bytes()).is_ok() {
                    total_rewrites += n;
                    files_changed += 1;
                }
            }
        }
    }
    Ok(RenameReport {
        old_path: old_path.to_owned(),
        new_path: new_path.to_owned(),
        link_rewrites: total_rewrites,
        files_changed,
    })
}

pub fn memory_create_category_impl(
    scope: &MemoryScope,
    workspace_cwd: &str,
    name: &str,
) -> Result<String, String> {
    let roots = get_roots_for_scope(scope, workspace_cwd)?;
    let clean = validate_category_name(name)?;
    let dir = roots.memory.join(&clean);
    if dir.exists() {
        return err(format!("already exists: {clean}"));
    }
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir: {e}"))?;
    let _ = fs::write(dir.join(CATEGORY_PLACEHOLDER), b"");
    Ok(clean)
}

// ── Graph ─────────────────────────────────────────────────────────────────────

pub fn memory_graph_impl(workspace_cwd: &str) -> Result<GraphData, String> {
    let ws_roots = get_roots_for_scope(&MemoryScope::Workspace, workspace_cwd).ok();
    let global_roots = get_global_roots();

    let ws_notes = collect_notes(&MemoryScope::Workspace, workspace_cwd);
    let global_notes = collect_notes_global();

    let mut scope_notes: Vec<ScopeNote> = Vec::new();

    for meta in &ws_notes {
        if meta.is_template || meta.is_overview {
            continue;
        }
        let body = ws_roots
            .as_ref()
            .and_then(|roots| note_abs(roots, &meta.path).ok())
            .and_then(|abs| fs::read_to_string(&abs).ok())
            .unwrap_or_default();
        scope_notes.push(ScopeNote {
            scope: MemoryScope::Workspace,
            path: meta.path.clone(),
            body,
            label: meta.name.clone(),
        });
    }

    for meta in &global_notes {
        if meta.is_template || meta.is_overview {
            continue;
        }
        let body = note_abs(&global_roots, &meta.path)
            .ok()
            .and_then(|abs| fs::read_to_string(&abs).ok())
            .unwrap_or_default();
        scope_notes.push(ScopeNote {
            scope: MemoryScope::Global,
            path: meta.path.clone(),
            body,
            label: meta.name.clone(),
        });
    }

    // Build category hubs from unique categories across both scopes
    let mut cat_scopes: HashMap<String, Vec<MemoryScope>> = HashMap::new();
    for sn in &scope_notes {
        let cat = graph_category_for(&sn.path);
        if cat != "memory" {
            let entry = cat_scopes.entry(cat).or_default();
            if !entry.contains(&sn.scope) {
                entry.push(sn.scope.clone());
            }
        }
    }
    let category_hubs: Vec<CategoryHubInput> = cat_scopes
        .into_iter()
        .map(|(cat, scopes)| CategoryHubInput {
            label: capitalize_first(&cat),
            category: cat,
            scopes,
        })
        .collect();

    Ok(build_graph(scope_notes, category_hubs))
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

// ── Backlinks & Search ────────────────────────────────────────────────────────

pub fn memory_backlinks_impl(
    scope: &MemoryScope,
    workspace_cwd: &str,
    path: &str,
) -> Result<Vec<BacklinkRef>, String> {
    let graph = memory_graph_impl(workspace_cwd)?;
    let target_id = node_id(scope, path);
    let mut out: Vec<BacklinkRef> = graph
        .edges
        .into_iter()
        .filter_map(|e| {
            if e.target == target_id {
                super::paths::parse_node_id(&e.source)
                    .map(|(s, p)| BacklinkRef { scope: s, path: p })
            } else {
                None
            }
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out.dedup_by(|a, b| a.path == b.path && a.scope == b.scope);
    Ok(out)
}

pub fn memory_search_impl(workspace_cwd: &str, query: &str) -> Result<Vec<SearchHit>, String> {
    let needle = query.trim();
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let needle_l = needle.to_ascii_lowercase();
    let mut hits: Vec<SearchHit> = Vec::new();

    for scope in [MemoryScope::Workspace, MemoryScope::Global] {
        let roots = match get_roots_for_scope(&scope, workspace_cwd) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for (is_learnings, root) in [
            (false, roots.memory.as_path()),
            (true, roots.learnings.as_path()),
        ] {
            let mut files = Vec::new();
            walk_md(root, &mut files);
            for abs in files {
                let Some(rel) = rel_from_root(root, &abs) else {
                    continue;
                };
                let api = if is_learnings {
                    format!("{LEARNINGS_API_PREFIX}{rel}")
                } else {
                    rel
                };
                let Ok(body) = fs::read_to_string(&abs) else {
                    continue;
                };
                for (idx, line) in body.lines().enumerate() {
                    if line.to_ascii_lowercase().contains(&needle_l) {
                        let snip = if line.len() > 200 { &line[..200] } else { line };
                        hits.push(SearchHit {
                            scope: scope.clone(),
                            path: api.clone(),
                            line: (idx + 1) as u32,
                            snippet: snip.to_owned(),
                            category: graph_category_for(&api),
                        });
                        if hits.len() >= 500 {
                            return Ok(hits);
                        }
                    }
                }
            }
        }
    }
    Ok(hits)
}

// ── Export / Import ───────────────────────────────────────────────────────────

pub fn memory_export_impl(workspace_cwd: &str, dest_dir: &str) -> Result<u32, String> {
    let roots = get_roots_for_scope(&MemoryScope::Workspace, workspace_cwd)?;
    let dest = PathBuf::from(dest_dir.trim());
    if !dest.is_absolute() {
        return err("dest_dir must be absolute");
    }
    fs::create_dir_all(&dest).map_err(|e| format!("mkdir dest: {e}"))?;
    let mut n = 0u32;
    for (root, sub) in [
        (roots.memory.as_path(), "memory"),
        (roots.learnings.as_path(), "learnings"),
    ] {
        let mut files = Vec::new();
        walk_md(root, &mut files);
        for f in files {
            let Some(rel) = rel_from_root(root, &f) else {
                continue;
            };
            let target = dest.join(sub).join(&rel);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
            }
            fs::copy(&f, &target).map_err(|e| format!("copy {rel}: {e}"))?;
            n += 1;
        }
    }
    Ok(n)
}

pub fn memory_import_impl(workspace_cwd: &str, src_dir: &str) -> Result<u32, String> {
    let roots = get_roots_for_scope(&MemoryScope::Workspace, workspace_cwd)?;
    let src = PathBuf::from(src_dir.trim());
    if !src.is_absolute() {
        return err("src_dir must be absolute");
    }
    if !src.exists() {
        return err("src_dir does not exist");
    }
    let mut n = 0u32;
    let learnings_src = src.join("learnings");
    let memory_src = src.join("memory");
    if learnings_src.is_dir() {
        n += import_tree(&learnings_src, &roots.learnings)?;
    }
    if memory_src.is_dir() {
        n += import_tree(&memory_src, &roots.memory)?;
    } else if !learnings_src.is_dir() {
        n += import_tree(&src, &roots.memory)?;
    }
    Ok(n)
}

fn import_tree(src: &Path, dest_root: &Path) -> Result<u32, String> {
    let mut files = Vec::new();
    walk_md(src, &mut files);
    let mut n = 0u32;
    for f in files {
        let Ok(rel_pb) = f.strip_prefix(src) else {
            continue;
        };
        let rel = rel_pb.to_string_lossy().replace('\\', "/");
        let abs = match safe_join(dest_root, &rel, true) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if abs.exists() {
            continue;
        }
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        fs::copy(&f, &abs).map_err(|e| format!("copy {rel}: {e}"))?;
        n += 1;
    }
    Ok(n)
}

// ── Pointer files ─────────────────────────────────────────────────────────────

const POINTER_BEGIN: &str = "<!-- blxcode-memory:begin -->";
const POINTER_END: &str = "<!-- blxcode-memory:end -->";
const POINTER_BEGIN_CURSOR: &str = "# blxcode-memory:begin";
const POINTER_END_CURSOR: &str = "# blxcode-memory:end";

fn pointer_filename(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("CLAUDE.md"),
        "codex" => Some("AGENTS.md"),
        "gemini" => Some("GEMINI.md"),
        "cursor" => Some(".cursorrules"),
        "opencode" => Some("AGENTS.md"),
        _ => None,
    }
}

fn pointer_body(workspace_cwd: &Path, notes: &[NoteMeta], cursor_style: bool) -> String {
    let memory_dir = workspace_cwd.join(MEMORY_REL);
    let learnings_dir = workspace_cwd.join(LEARNINGS_REL);
    let mut s = String::new();
    if cursor_style {
        s.push_str("blxcode tracks per-workspace memory and learnings at the paths below.\n");
    } else {
        s.push_str("## blxcode workspace memory\n\n");
        s.push_str(
            "This workspace uses **blxcode** to maintain Markdown notes and learnings \
shared across all agent sessions. Treat the directories below as authoritative \
context: read notes that are relevant to the task, and propose new notes \
or edits when you learn something the team should remember.\n\n",
        );
    }
    s.push_str(&format!("Memory root: `{}`\n", memory_dir.display()));
    s.push_str(&format!(
        "Learnings root: `{}` (API paths: `learnings/…`)\n\n",
        learnings_dir.display()
    ));
    let non_template: Vec<_> = notes.iter().filter(|n| !n.is_template).collect();
    if non_template.is_empty() {
        s.push_str("(no notes yet)\n");
    } else {
        s.push_str("Current notes:\n");
        let mut shown = 0;
        for n in &non_template {
            s.push_str(&format!("- `{}`\n", n.path));
            shown += 1;
            if shown >= 80 {
                s.push_str(&format!("- … and {} more\n", non_template.len() - shown));
                break;
            }
        }
    }
    s.push('\n');
    s
}

fn splice_block(existing: &str, begin: &str, end: &str, new_body: &str) -> String {
    let block = format!("{begin}\n{new_body}{end}\n");
    if let (Some(bi), Some(ei)) = (existing.find(begin), existing.find(end)) {
        let ei_end = ei + end.len();
        let tail_start = if ei_end < existing.len() && existing.as_bytes()[ei_end] == b'\n' {
            ei_end + 1
        } else {
            ei_end
        };
        let mut out = String::with_capacity(bi + block.len() + existing.len() - tail_start);
        out.push_str(&existing[..bi]);
        out.push_str(&block);
        out.push_str(&existing[tail_start..]);
        return out;
    }
    let mut out = String::new();
    out.push_str(existing);
    if !existing.is_empty() && !existing.ends_with('\n') {
        out.push('\n');
    }
    if !existing.is_empty() {
        out.push('\n');
    }
    out.push_str(&block);
    out
}

fn strip_block(existing: &str, begin: &str, end: &str) -> String {
    if let (Some(bi), Some(ei)) = (existing.find(begin), existing.find(end)) {
        let ei_end = ei + end.len();
        let tail_start = if ei_end < existing.len() && existing.as_bytes()[ei_end] == b'\n' {
            ei_end + 1
        } else {
            ei_end
        };
        let mut out = String::new();
        out.push_str(&existing[..bi]);
        out.push_str(&existing[tail_start..]);
        return out.trim_end_matches('\n').to_owned() + "\n";
    }
    existing.to_owned()
}

pub fn memory_install_pointers_impl(
    workspace_cwd: &str,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    use crate::agents_layout::validate_workspace_cwd;
    let ws = validate_workspace_cwd(workspace_cwd)?;
    let notes = collect_notes(&MemoryScope::Workspace, workspace_cwd);
    let mut results: Vec<PointerResult> = Vec::new();
    let mut written: std::collections::BTreeSet<String> = Default::default();
    for agent in agents {
        let Some(fname) = pointer_filename(&agent) else {
            results.push(PointerResult {
                agent,
                path: String::new(),
                installed: false,
                note: Some("unknown agent".into()),
            });
            continue;
        };
        let path = ws.join(fname);
        if written.contains(fname) {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some("shared file already handled".into()),
            });
            continue;
        }
        if !path.exists() {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some(
                    "skipped: file absent; blxcode does not auto-create root pointer files".into(),
                ),
            });
            written.insert(fname.to_owned());
            continue;
        }
        let cursor_style = agent == "cursor";
        let body = pointer_body(&ws, &notes, cursor_style);
        let (begin, end) = if cursor_style {
            (POINTER_BEGIN_CURSOR, POINTER_END_CURSOR)
        } else {
            (POINTER_BEGIN, POINTER_END)
        };
        let existing = fs::read_to_string(&path).unwrap_or_default();
        if !existing.contains(begin) {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some(
                    "skipped: existing file is user-owned and has no blxcode managed block".into(),
                ),
            });
            written.insert(fname.to_owned());
            continue;
        }
        let updated = splice_block(&existing, begin, end, &body);
        match fs::write(&path, updated.as_bytes()) {
            Ok(()) => {
                written.insert(fname.to_owned());
                results.push(PointerResult {
                    agent,
                    path: path.to_string_lossy().into_owned(),
                    installed: true,
                    note: None,
                });
            }
            Err(e) => results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some(format!("write failed: {e}")),
            }),
        }
    }
    Ok(results)
}

pub fn memory_uninstall_pointers_impl(
    workspace_cwd: &str,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    use crate::agents_layout::validate_workspace_cwd;
    let ws = validate_workspace_cwd(workspace_cwd)?;
    let mut results = Vec::new();
    let mut handled: std::collections::BTreeSet<String> = Default::default();
    for agent in agents {
        let Some(fname) = pointer_filename(&agent) else {
            results.push(PointerResult {
                agent,
                path: String::new(),
                installed: false,
                note: Some("unknown agent".into()),
            });
            continue;
        };
        if handled.contains(fname) {
            results.push(PointerResult {
                agent,
                path: ws.join(fname).to_string_lossy().into_owned(),
                installed: false,
                note: Some("shared file already cleaned".into()),
            });
            continue;
        }
        let path = ws.join(fname);
        let cursor_style = agent == "cursor";
        let (begin, end) = if cursor_style {
            (POINTER_BEGIN_CURSOR, POINTER_END_CURSOR)
        } else {
            (POINTER_BEGIN, POINTER_END)
        };
        let existing = fs::read_to_string(&path).unwrap_or_default();
        if existing.is_empty() {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some("no file".into()),
            });
            continue;
        }
        let stripped = strip_block(&existing, begin, end);
        if stripped.trim().is_empty() {
            let _ = fs::remove_file(&path);
        } else if let Err(e) = fs::write(&path, stripped.as_bytes()) {
            results.push(PointerResult {
                agent,
                path: path.to_string_lossy().into_owned(),
                installed: false,
                note: Some(format!("write failed: {e}")),
            });
            continue;
        }
        handled.insert(fname.to_owned());
        results.push(PointerResult {
            agent,
            path: path.to_string_lossy().into_owned(),
            installed: false,
            note: Some("removed".into()),
        });
    }
    Ok(results)
}

pub fn memory_pointer_status_impl(workspace_cwd: &str) -> Result<Vec<PointerResult>, String> {
    use crate::agents_layout::validate_workspace_cwd;
    let ws = validate_workspace_cwd(workspace_cwd)?;
    let agents = ["claude", "codex", "gemini", "cursor", "opencode"];
    let mut out = Vec::new();
    for a in agents {
        let Some(fname) = pointer_filename(a) else {
            continue;
        };
        let path = ws.join(fname);
        let body = fs::read_to_string(&path).unwrap_or_default();
        let cursor_style = a == "cursor";
        let begin = if cursor_style {
            POINTER_BEGIN_CURSOR
        } else {
            POINTER_BEGIN
        };
        out.push(PointerResult {
            agent: a.to_owned(),
            path: path.to_string_lossy().into_owned(),
            installed: body.contains(begin),
            note: None,
        });
    }
    Ok(out)
}
