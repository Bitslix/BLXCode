//! Workspace-scoped Obsidian-style memory.
//!
//! Layout per workspace:
//!
//! ```text
//! <workspace_cwd>/.agents/memory/      — user notes (+ _templates/)
//! <workspace_cwd>/.agents/learnings/ — durable repo learnings (API: `learnings/…`)
//! ```
//!
//! Legacy `.blxcode/memory/` is migrated once into `.agents/memory/` when empty.
//! All Tauri commands take the workspace cwd; API paths are sandboxed.
//!
//! Link syntax (Obsidian-compatible subset):
//!   `[[Note Name]]`            — link to `Note Name.md` (by basename, case-insensitive)
//!   `[[folder/Note]]`          — explicit relative path
//!   `[[Note Name|alias]]`      — display alias (ignored for graph)
//!   `#tag`                     — tag (graph metadata)

use crate::agents_layout::{
    ensure_agents_layout, validate_workspace_cwd, WorkspaceRoots, LEARNINGS_API_PREFIX,
    LEARNINGS_REL, MEMORY_REL,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

const TEMPLATES_DIRNAME: &str = "_templates";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    /// Relative path within the memory root, forward slashes, includes `.md`.
    pub path: String,
    /// Basename without extension.
    pub name: String,
    /// File size in bytes.
    pub size: u64,
    /// Last-modified time as seconds since UNIX epoch.
    pub modified: i64,
    /// True if under the `_templates/` subdir.
    pub is_template: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoteContent {
    pub path: String,
    pub content: String,
    pub modified: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub tags: Vec<String>,
    pub orphan: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub path: String,
    pub line: u32,
    pub snippet: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PointerResult {
    pub agent: String,
    pub path: String,
    pub installed: bool,
    pub note: Option<String>,
}

fn err<T>(s: impl Into<String>) -> Result<T, String> {
    Err(s.into())
}

fn ensure_workspace_memory(ws: &str) -> Result<WorkspaceRoots, String> {
    ensure_agents_layout(ws)
}

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

fn note_root<'a>(roots: &'a WorkspaceRoots, is_learnings: bool) -> &'a Path {
    if is_learnings {
        roots.learnings.as_path()
    } else {
        roots.memory.as_path()
    }
}

fn resolve_note_abs(roots: &WorkspaceRoots, api_path: &str) -> Result<PathBuf, String> {
    let (is_learnings, rel) = resolve_api_path(api_path)?;
    safe_join(note_root(roots, is_learnings), &rel, true)
}

fn api_path_for(roots: &WorkspaceRoots, file_root: &Path, rel: &str) -> String {
    if file_root == roots.learnings.as_path() {
        format!("{LEARNINGS_API_PREFIX}{rel}")
    } else {
        rel.to_owned()
    }
}

fn collect_all_md(roots: &WorkspaceRoots, out: &mut Vec<(String, PathBuf, PathBuf)>) {
    for root in [roots.memory.as_path(), roots.learnings.as_path()] {
        let mut files = Vec::new();
        walk_md(root, &mut files);
        for abs in files {
            let Some(rel) = rel_from_root(root, &abs) else {
                continue;
            };
            let api = api_path_for(roots, root, &rel);
            out.push((api, abs, root.to_path_buf()));
        }
    }
}

fn meta_from_abs(_roots: &WorkspaceRoots, api_path: &str, abs: &Path, file_root: &Path) -> NoteMeta {
    let rel = rel_from_root(file_root, abs).unwrap_or_default();
    let meta = fs::metadata(abs).ok();
    let name = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();
    let is_template = !api_path.starts_with(LEARNINGS_API_PREFIX)
        && rel.starts_with(&format!("{TEMPLATES_DIRNAME}/"));
    NoteMeta {
        path: api_path.to_owned(),
        name,
        size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        modified: mtime_secs(abs),
        is_template,
    }
}

/// Validates `rel` is a clean relative path with no `..` or absolute
/// segments, normalises slashes to forward, and returns the absolute
/// path inside `root`. Also enforces `.md` extension when `enforce_md`.
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
    // Cheap defense-in-depth: re-check the joined path lies under root.
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

fn rel_from_root(root: &Path, p: &Path) -> Option<String> {
    p.strip_prefix(root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
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

#[tauri::command]
pub fn workspace_ensure_agents(workspace_cwd: String) -> Result<(), String> {
    ensure_agents_layout(&workspace_cwd)?;
    Ok(())
}

#[tauri::command]
pub fn memory_root(workspace_cwd: String) -> Result<String, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    Ok(roots.memory.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn memory_list(workspace_cwd: String) -> Result<Vec<NoteMeta>, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    let mut collected = Vec::new();
    collect_all_md(&roots, &mut collected);
    let mut out: Vec<NoteMeta> = collected
        .into_iter()
        .map(|(api, abs, file_root)| meta_from_abs(&roots, &api, &abs, &file_root))
        .collect();
    out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub fn memory_read(workspace_cwd: String, path: String) -> Result<NoteContent, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    let abs = resolve_note_abs(&roots, &path)?;
    let content = fs::read_to_string(&abs).map_err(|e| format!("read {path}: {e}"))?;
    Ok(NoteContent {
        path,
        content,
        modified: mtime_secs(&abs),
    })
}

#[tauri::command]
pub fn memory_write(
    workspace_cwd: String,
    path: String,
    content: String,
) -> Result<NoteContent, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    let abs = resolve_note_abs(&roots, &path)?;
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let mut file = fs::File::create(&abs).map_err(|e| format!("create {path}: {e}"))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("write {path}: {e}"))?;
    Ok(NoteContent {
        path,
        content,
        modified: mtime_secs(&abs),
    })
}

#[tauri::command]
pub fn memory_create(
    workspace_cwd: String,
    path: String,
    content: Option<String>,
) -> Result<NoteMeta, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    let abs = resolve_note_abs(&roots, &path)?;
    if abs.exists() {
        return err(format!("already exists: {path}"));
    }
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let body = content.unwrap_or_default();
    fs::write(&abs, body.as_bytes()).map_err(|e| format!("write {path}: {e}"))?;
    let (is_learnings, _) = resolve_api_path(&path)?;
    let file_root = note_root(&roots, is_learnings);
    Ok(meta_from_abs(&roots, &path, &abs, file_root))
}

#[tauri::command]
pub fn memory_delete(workspace_cwd: String, path: String) -> Result<(), String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    let (is_learnings, _) = resolve_api_path(&path)?;
    let root = note_root(&roots, is_learnings);
    let abs = resolve_note_abs(&roots, &path)?;
    if !abs.exists() {
        return err(format!("not found: {path}"));
    }
    fs::remove_file(&abs).map_err(|e| format!("delete {path}: {e}"))?;
    // best-effort: remove empty parent dirs up to root
    if let Some(mut parent) = abs.parent() {
        while parent != root {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RenameReport {
    pub old_path: String,
    pub new_path: String,
    pub link_rewrites: u32,
    pub files_changed: u32,
}

#[tauri::command]
pub fn memory_rename(
    workspace_cwd: String,
    old_path: String,
    new_path: String,
    rewrite_links: bool,
) -> Result<RenameReport, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    let (old_learn, _) = resolve_api_path(&old_path)?;
    let (new_learn, _) = resolve_api_path(&new_path)?;
    if old_learn != new_learn {
        return err("cannot rename across memory and learnings roots");
    }
    let abs_old = resolve_note_abs(&roots, &old_path)?;
    let abs_new = resolve_note_abs(&roots, &new_path)?;
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

    let old_basename = Path::new(&old_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();
    let new_basename = Path::new(&new_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();

    let mut total_rewrites = 0u32;
    let mut files_changed = 0u32;

    if rewrite_links && !old_basename.is_empty() && !new_basename.is_empty() {
        let mut collected = Vec::new();
        collect_all_md(&roots, &mut collected);
        for (_, f, _) in collected {
            let Ok(content) = fs::read_to_string(&f) else {
                continue;
            };
            let (updated, n) =
                rewrite_wikilinks(&content, &old_basename, &new_basename, &old_path, &new_path);
            if n > 0 {
                if fs::write(&f, updated.as_bytes()).is_ok() {
                    total_rewrites += n;
                    files_changed += 1;
                }
            }
        }
    }
    Ok(RenameReport {
        old_path,
        new_path,
        link_rewrites: total_rewrites,
        files_changed,
    })
}

/// Rewrites `[[old_basename|...]]` -> `[[new_basename|...]]` and
/// `[[old_path_without_ext|...]]` -> `[[new_path_without_ext|...]]`.
/// Returns (new_content, count).
fn rewrite_wikilinks(
    content: &str,
    old_basename: &str,
    new_basename: &str,
    old_path: &str,
    new_path: &str,
) -> (String, u32) {
    let old_pwx = strip_md_ext(old_path);
    let new_pwx = strip_md_ext(new_path);
    let mut out = String::with_capacity(content.len());
    let mut i = 0usize;
    let bytes = content.as_bytes();
    let mut count = 0u32;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = find_close_link(&content[i + 2..]) {
                let inner = &content[i + 2..i + 2 + end];
                // split target|alias
                let (target, alias) = match inner.find('|') {
                    Some(j) => (&inner[..j], Some(&inner[j + 1..])),
                    None => (inner, None),
                };
                let target_t = target.trim();
                let new_target = if target_t.eq_ignore_ascii_case(old_basename)
                    || target_t.eq_ignore_ascii_case(&old_pwx)
                {
                    if target_t.contains('/') {
                        Some(new_pwx.clone())
                    } else {
                        Some(new_basename.to_owned())
                    }
                } else {
                    None
                };
                if let Some(t) = new_target {
                    out.push_str("[[");
                    out.push_str(&t);
                    if let Some(a) = alias {
                        out.push('|');
                        out.push_str(a);
                    }
                    out.push_str("]]");
                    count += 1;
                } else {
                    out.push_str(&content[i..i + 2 + end + 2]);
                }
                i += 2 + end + 2;
                continue;
            }
        }
        out.push(content[i..].chars().next().unwrap());
        i += content[i..].chars().next().unwrap().len_utf8();
    }
    (out, count)
}

fn strip_md_ext(p: &str) -> String {
    if let Some(idx) = p.rfind('.') {
        if p[idx + 1..].eq_ignore_ascii_case("md") {
            return p[..idx].to_owned();
        }
    }
    p.to_owned()
}

fn find_close_link(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b']' && bytes[i + 1] == b']' {
            return Some(i);
        }
        if bytes[i] == b'\n' {
            return None;
        }
        i += 1;
    }
    None
}

/// Extracts `[[link]]` targets (without alias) and `#tag`s from a body.
fn parse_links_and_tags(body: &str) -> (Vec<String>, Vec<String>) {
    let mut links: Vec<String> = Vec::new();
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let mut i = 0;
    let bytes = body.as_bytes();
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = find_close_link(&body[i + 2..]) {
                let inner = &body[i + 2..i + 2 + end];
                let target = match inner.find('|') {
                    Some(j) => &inner[..j],
                    None => inner,
                };
                let t = target.trim();
                if !t.is_empty() {
                    links.push(t.to_owned());
                }
                i += 2 + end + 2;
                continue;
            }
        }
        if bytes[i] == b'#' {
            let prev = if i == 0 { b'\n' } else { bytes[i - 1] };
            // tag must follow whitespace or start of line; skip markdown headings
            if prev == b'\n' || prev == b' ' || prev == b'\t' {
                let mut j = i + 1;
                while j < bytes.len() {
                    let c = bytes[j];
                    let ok = c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'/';
                    if !ok {
                        break;
                    }
                    j += 1;
                }
                // Heading `# Foo` => j == i+1 with space after; require at least 1 char
                // and that the char after '#' is not space (so headings are excluded).
                if j > i + 1 {
                    let after_hash = bytes[i + 1];
                    if after_hash != b' ' {
                        let tag = &body[i + 1..j];
                        if !tag.is_empty() && !tag.chars().all(|c| c.is_ascii_digit()) {
                            tags.insert(tag.to_owned());
                        }
                    }
                }
                i = j.max(i + 1);
                continue;
            }
        }
        i += 1;
    }
    (links, tags.into_iter().collect())
}

#[tauri::command]
pub fn memory_graph(workspace_cwd: String) -> Result<GraphData, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    let mut collected = Vec::new();
    collect_all_md(&roots, &mut collected);
    collected.retain(|(api, _, _)| !api.starts_with(&format!("{TEMPLATES_DIRNAME}/")));

    let mut by_basename: HashMap<String, Vec<String>> = HashMap::new();
    let mut by_pwx: HashMap<String, String> = HashMap::new();
    let mut bodies: BTreeMap<String, String> = BTreeMap::new();

    for (api, f, _) in &collected {
        if let Some(stem) = Path::new(api)
            .file_stem()
            .and_then(|s| s.to_str())
        {
            by_basename
                .entry(stem.to_ascii_lowercase())
                .or_default()
                .push(api.clone());
        }
        by_pwx.insert(strip_md_ext(api).to_ascii_lowercase(), api.clone());
        if let Ok(body) = fs::read_to_string(f) {
            bodies.insert(api.clone(), body);
        }
    }

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut has_edge: BTreeSet<String> = BTreeSet::new();

    for (rel, body) in &bodies {
        let (links, tags) = parse_links_and_tags(body);
        let mut node_tags = tags;
        node_tags.sort();
        node_tags.dedup();
        let stem = Path::new(rel)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        nodes.push(GraphNode {
            id: rel.clone(),
            label: stem,
            tags: node_tags,
            orphan: false,
        });
        for raw in links {
            let lower = raw.to_ascii_lowercase();
            let target = if let Some(t) = by_pwx.get(&lower) {
                Some(t.clone())
            } else if let Some(list) = by_basename.get(&lower) {
                list.first().cloned()
            } else {
                None
            };
            if let Some(t) = target {
                if t != *rel {
                    has_edge.insert(rel.clone());
                    has_edge.insert(t.clone());
                    edges.push(GraphEdge {
                        source: rel.clone(),
                        target: t,
                    });
                }
            }
        }
    }

    for n in nodes.iter_mut() {
        n.orphan = !has_edge.contains(&n.id);
    }

    Ok(GraphData { nodes, edges })
}

#[tauri::command]
pub fn memory_backlinks(workspace_cwd: String, path: String) -> Result<Vec<String>, String> {
    let g = memory_graph(workspace_cwd)?;
    let mut out: Vec<String> = g
        .edges
        .into_iter()
        .filter_map(|e| {
            if e.target == path {
                Some(e.source)
            } else {
                None
            }
        })
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

#[tauri::command]
pub fn memory_search(workspace_cwd: String, query: String) -> Result<Vec<SearchHit>, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
    let needle = query.trim();
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let needle_l = needle.to_ascii_lowercase();
    let mut collected = Vec::new();
    collect_all_md(&roots, &mut collected);
    let mut hits: Vec<SearchHit> = Vec::new();
    for (api, f, _) in collected {
        let Ok(body) = fs::read_to_string(&f) else {
            continue;
        };
        for (idx, line) in body.lines().enumerate() {
            if line.to_ascii_lowercase().contains(&needle_l) {
                let snip = if line.len() > 200 { &line[..200] } else { line };
                hits.push(SearchHit {
                    path: api.clone(),
                    line: (idx + 1) as u32,
                    snippet: snip.to_owned(),
                });
                if hits.len() >= 500 {
                    return Ok(hits);
                }
            }
        }
    }
    Ok(hits)
}

#[tauri::command]
pub fn memory_export(workspace_cwd: String, dest_dir: String) -> Result<u32, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
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

#[tauri::command]
pub fn memory_import(workspace_cwd: String, src_dir: String) -> Result<u32, String> {
    let roots = ensure_workspace_memory(&workspace_cwd)?;
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
        n += import_tree_into_root(&learnings_src, &roots.learnings)?;
    }
    if memory_src.is_dir() {
        n += import_tree_into_root(&memory_src, &roots.memory)?;
    } else if !learnings_src.is_dir() {
        n += import_tree_into_root(&src, &roots.memory)?;
    }
    Ok(n)
}

fn import_tree_into_root(src: &Path, dest_root: &Path) -> Result<u32, String> {
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

// ---------------------------------------------------------------------
// Phase 4: agent pointer files.
//
// Each supported agent CLI has its own "memory" convention. blxcode can
// manage a short pointer block inside those files, but must never create
// them implicitly or modify unrelated user-owned content.
//
//   claude    -> CLAUDE.md
//   codex     -> AGENTS.md
//   gemini    -> GEMINI.md
//   cursor    -> .cursorrules
//   opencode  -> AGENTS.md   (shares with codex; we write once)
//
// The block is delimited by a marker so re-installs replace cleanly
// instead of duplicating content. Anything outside the markers is
// preserved.

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
    if notes.is_empty() {
        s.push_str("(no notes yet)\n");
    } else {
        s.push_str("Current notes:\n");
        let mut shown = 0;
        for n in notes {
            if n.is_template {
                continue;
            }
            s.push_str(&format!("- `{}`\n", n.path));
            shown += 1;
            if shown >= 80 {
                s.push_str(&format!("- … and {} more\n", notes.len() - shown));
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
        let head = &existing[..bi];
        let tail = &existing[tail_start..];
        let mut out = String::with_capacity(head.len() + block.len() + tail.len());
        out.push_str(head);
        out.push_str(&block);
        out.push_str(tail);
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

#[tauri::command]
pub fn memory_install_pointers(
    workspace_cwd: String,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    let ws = validate_workspace_cwd(&workspace_cwd)?;
    let _ = ensure_workspace_memory(&workspace_cwd)?;
    let notes = memory_list(workspace_cwd.clone()).unwrap_or_default();
    let mut results: Vec<PointerResult> = Vec::new();
    let mut written_files: BTreeSet<String> = BTreeSet::new();
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
        if written_files.contains(fname) {
            // codex+opencode share AGENTS.md; report second one as skipped
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
            written_files.insert(fname.to_owned());
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
            written_files.insert(fname.to_owned());
            continue;
        }
        let updated = splice_block(&existing, begin, end, &body);
        match fs::write(&path, updated.as_bytes()) {
            Ok(()) => {
                written_files.insert(fname.to_owned());
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

#[tauri::command]
pub fn memory_uninstall_pointers(
    workspace_cwd: String,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    let ws = validate_workspace_cwd(&workspace_cwd)?;
    let mut results = Vec::new();
    let mut handled: BTreeSet<String> = BTreeSet::new();
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

#[tauri::command]
pub fn memory_pointer_status(workspace_cwd: String) -> Result<Vec<PointerResult>, String> {
    let ws = validate_workspace_cwd(&workspace_cwd)?;
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
        let installed = body.contains(begin);
        out.push(PointerResult {
            agent: a.to_owned(),
            path: path.to_string_lossy().into_owned(),
            installed,
            note: None,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents_layout::{LEGACY_MEMORY_REL, LEARNINGS_REL};

    #[test]
    fn safe_join_rejects_traversal() {
        let tmp = std::env::temp_dir().join("blxcode_memtest_safe");
        let _ = fs::create_dir_all(&tmp);
        assert!(safe_join(&tmp, "../etc/passwd", false).is_err());
        // Leading slash is stripped and treated as relative — the safety
        // invariant is "result stays under root", not "reject the input".
        assert!(safe_join(&tmp, "/abs/note.md", true).is_ok());
        assert!(safe_join(&tmp, "ok/file.md", true).is_ok());
        assert!(safe_join(&tmp, "ok/file.txt", true).is_err());
        assert!(safe_join(&tmp, "ok/../../../etc/passwd", true).is_err());
    }

    #[test]
    fn rewrite_wikilinks_basic() {
        let body = "see [[Old]] and [[Old|label]] but not [[Other]]";
        let (out, n) = rewrite_wikilinks(body, "Old", "New", "Old.md", "New.md");
        assert_eq!(n, 2);
        assert!(out.contains("[[New]]"));
        assert!(out.contains("[[New|label]]"));
        assert!(out.contains("[[Other]]"));
    }

    #[test]
    fn parse_tags_skips_headings() {
        let (_, tags) = parse_links_and_tags("# Heading\n\nText with #foo and #bar-baz here");
        assert!(tags.contains(&"foo".to_owned()));
        assert!(tags.contains(&"bar-baz".to_owned()));
        assert!(!tags.iter().any(|t| t == "Heading"));
    }

    #[test]
    fn memory_list_includes_learnings_and_legacy_migrates() {
        let ws = std::env::temp_dir().join(format!(
            "blxcode_mem_multi_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&ws);
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(ws.join(LEARNINGS_REL)).unwrap();
        fs::write(
            ws.join(LEARNINGS_REL).join("LEARNINGS.md"),
            "## Index\n\n- [Topic](topic.md)\n",
        )
        .unwrap();
        fs::write(ws.join(LEARNINGS_REL).join("topic.md"), "# Topic").unwrap();
        fs::create_dir_all(ws.join(LEGACY_MEMORY_REL)).unwrap();
        fs::write(ws.join(LEGACY_MEMORY_REL).join("legacy.md"), "old").unwrap();

        let cwd = ws.to_string_lossy().into_owned();
        let list = memory_list(cwd.clone()).unwrap();
        assert!(list.iter().any(|n| n.path == "learnings/topic.md"));
        assert!(list.iter().any(|n| n.path == "legacy.md"));

        let g = memory_graph(cwd).unwrap();
        assert!(g.edges.iter().any(|e| e.source.contains("LEARNINGS") || e.target.contains("topic")));

        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn install_pointers_does_not_create_missing_root_files() {
        let ws = std::env::temp_dir().join(format!(
            "blxcode_memtest_no_create_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&ws).unwrap();

        let out = memory_install_pointers(
            ws.to_string_lossy().into_owned(),
            vec!["claude".into(), "codex".into()],
        )
        .unwrap();

        assert!(!ws.join("CLAUDE.md").exists());
        assert!(!ws.join("AGENTS.md").exists());
        assert!(out.iter().all(|r| !r.installed));
    }

    #[test]
    fn install_pointers_does_not_touch_unmanaged_existing_files() {
        let ws = std::env::temp_dir().join(format!(
            "blxcode_memtest_unmanaged_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&ws).unwrap();
        let agents = ws.join("AGENTS.md");
        fs::write(&agents, "# user content\n").unwrap();

        let before = fs::read_to_string(&agents).unwrap();
        let out = memory_install_pointers(ws.to_string_lossy().into_owned(), vec!["codex".into()])
            .unwrap();
        let after = fs::read_to_string(&agents).unwrap();

        assert_eq!(before, after);
        assert_eq!(out.len(), 1);
        assert!(!out[0].installed);
    }
}
