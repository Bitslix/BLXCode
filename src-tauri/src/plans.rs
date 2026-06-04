//! Workspace-scoped Markdown plans.
//!
//! Layout per workspace:
//!
//! ```text
//! <workspace_cwd>/.agents/plans/          — durable Markdown plans
//!   PLANS.md                              — index (never deleted)
//!   <plan-slug>/plan.md                   — canonical plan file
//! ```
//!
//! Plans are Markdown-first. Each plan can contain a canonical `## Tasks`
//! (or `## Todos`) section that the harness parses into the workspace task
//! manager. The parser/writer round-trip is intentionally simple: one
//! task per line with the form
//!
//! ```text
//! - [ ] `task-id` - title
//! ```
//!
//! Status markers:
//!   `[ ]` pending,  `[>]` in progress,  `[!]` blocked,
//!   `[x]` completed,  `[-]` cancelled.

use crate::agents_layout::{ensure_agents_layout, PLANS_INDEX};
use crate::kanban;
use crate::tasks;
use crate::tasks::TaskStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

const PLAN_FILE: &str = "plan.md";
const PLANS_README: &str = "README.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMeta {
    /// Canonical relative path within the plans root.
    ///
    /// Normal plans use `<slug>/plan.md`; the protected index uses `PLANS.md`.
    pub path: String,
    /// Stable plan slug, or `PLANS` for the protected index.
    pub name: String,
    /// Stable plan slug. Empty for the protected index.
    #[serde(default)]
    pub slug: String,
    /// Relative folder path containing the canonical plan file. Empty for the
    /// protected index.
    #[serde(default)]
    pub folder_path: String,
    /// Heading from the file (first `# …`) or basename if missing.
    pub title: String,
    /// File size in bytes.
    pub size: u64,
    /// Last-modified time as seconds since UNIX epoch.
    pub modified: i64,
    /// True for `PLANS.md` — protected index.
    pub is_index: bool,
    /// Parsed task summary (counts by status, plus total).
    pub task_summary: PlanTaskSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTaskSummary {
    pub total: u32,
    pub pending: u32,
    pub in_progress: u32,
    pub blocked: u32,
    pub completed: u32,
    pub cancelled: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanContent {
    pub path: String,
    pub content: String,
    pub modified: i64,
    pub is_index: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanTask {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLoadReport {
    pub path: String,
    pub tasks_replaced: u32,
    pub tasks_added: u32,
    pub free_tasks_kept: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSyncReport {
    pub path: String,
    pub tasks_written: u32,
}

#[derive(Clone, Default)]
pub struct PlanMigrationState {
    inner: Arc<Mutex<HashMap<String, PlanMigrationProgress>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMigrationProgress {
    pub phase: String,
    pub busy: bool,
    pub total: u32,
    pub processed: u32,
    pub migrated: u32,
    pub skipped: u32,
    pub error: Option<String>,
    pub updated_at_ms: u64,
}

impl Default for PlanMigrationProgress {
    fn default() -> Self {
        Self {
            phase: "idle".into(),
            busy: false,
            total: 0,
            processed: 0,
            migrated: 0,
            skipped: 0,
            error: None,
            updated_at_ms: now_ms(),
        }
    }
}

fn err<T>(s: impl Into<String>) -> Result<T, String> {
    Err(s.into())
}

fn lock_err<T>(_: std::sync::PoisonError<T>) -> String {
    "plan migration state lock poisoned".into()
}

fn ensure_plans_root(ws: &str) -> Result<PathBuf, String> {
    let roots = ensure_agents_layout(ws)?;
    Ok(roots.plans)
}

/// Absolute path to a plan's folder (`<plans_root>/<slug>`), creating the
/// `.agents/plans` layout if needed. Validates the slug and guarantees the
/// result stays under the plans root. Used by the mermaid diagram store so
/// plan-linked diagrams live next to their `plan.md`.
pub(crate) fn plan_folder_abs(ws: &str, slug: &str) -> Result<PathBuf, String> {
    validate_slug(slug)?;
    let root = ensure_plans_root(ws)?;
    let abs = root.join(slug);
    ensure_under_root(&root, &abs)?;
    Ok(abs)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
struct PlanPath {
    abs: PathBuf,
    rel: String,
    slug: String,
    folder_path: String,
    is_index: bool,
    legacy_rel: Option<String>,
}

impl PlanPath {
    fn legacy_rel_for_canonical(&self) -> Option<String> {
        if self.is_index || self.slug.is_empty() {
            None
        } else {
            Some(format!("{}.md", self.slug))
        }
    }
}

/// Validate and normalize a plan API path.
///
/// Accepted user-facing forms are:
/// - `PLANS.md` for the protected index
/// - `slug`
/// - legacy `slug.md`
/// - canonical `slug/plan.md`
///
/// Returned normal plans always use the canonical relative path
/// `slug/plan.md`.
fn normalize_plan_path(root: &Path, rel: &str) -> Result<PlanPath, String> {
    let rel = rel.trim().trim_start_matches('/').trim_start_matches('\\');
    if rel.is_empty() {
        return err("empty plan path");
    }
    let normalized = rel.replace('\\', "/");
    let candidate = PathBuf::from(&normalized);
    for c in candidate.components() {
        match c {
            Component::Normal(_) => {}
            _ => return err(format!("disallowed path component in {rel}")),
        }
    }

    let parts: Vec<String> = candidate
        .components()
        .map(|c| match c {
            Component::Normal(s) => s.to_str().map(str::to_owned),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "plan paths must be valid UTF-8".to_owned())?;

    let (canonical_rel, slug, folder_path, is_index, legacy_rel) = match parts.as_slice() {
        [file] if file.eq_ignore_ascii_case(PLANS_INDEX) => (
            PLANS_INDEX.to_owned(),
            String::new(),
            String::new(),
            true,
            None,
        ),
        [file] if file.eq_ignore_ascii_case(PLANS_README) => {
            return err("README.md is not a plan file")
        }
        [slug] if !slug.contains('.') => {
            validate_slug(slug)?;
            (
                format!("{slug}/{PLAN_FILE}"),
                slug.clone(),
                slug.clone(),
                false,
                Some(format!("{slug}.md")),
            )
        }
        [file] => {
            let path = Path::new(file);
            let is_md = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !is_md {
                return err("plan paths must be a slug, legacy .md file, or slug/plan.md");
            }
            let slug = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_owned();
            validate_slug(&slug)?;
            (
                format!("{slug}/{PLAN_FILE}"),
                slug.clone(),
                slug.clone(),
                false,
                Some(format!("{slug}.md")),
            )
        }
        [slug, file] if file.eq_ignore_ascii_case(PLAN_FILE) => {
            validate_slug(slug)?;
            (
                format!("{slug}/{PLAN_FILE}"),
                slug.clone(),
                slug.clone(),
                false,
                Some(format!("{slug}.md")),
            )
        }
        _ => return err("plan paths must be PLANS.md, a slug, legacy slug.md, or slug/plan.md"),
    };

    let abs = root.join(&canonical_rel);
    ensure_under_root(root, &abs)?;
    Ok(PlanPath {
        abs,
        rel: canonical_rel,
        slug,
        folder_path,
        is_index,
        legacy_rel,
    })
}

fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.trim().is_empty() {
        return err("empty plan slug");
    }
    if slug == "." || slug == ".." {
        return err("disallowed plan slug");
    }
    if slug.contains('/') || slug.contains('\\') {
        return err("plan slugs may not contain path separators");
    }
    Ok(())
}

fn ensure_under_root(root: &Path, abs: &Path) -> Result<(), String> {
    let canon_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let canon_abs = abs
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        .map(|p| p.join(abs.file_name().unwrap_or_default()))
        .unwrap_or_else(|| abs.to_path_buf());
    if !canon_abs.starts_with(&canon_root) {
        return err("path escapes plans root");
    }
    Ok(())
}

fn resolve_plan_path(root: &Path, rel: &str) -> Result<PlanPath, String> {
    let mut plan = normalize_plan_path(root, rel)?;
    if !plan.is_index && !plan.abs.exists() {
        if let Some(legacy_rel) = plan.legacy_rel.clone() {
            let legacy_abs = root.join(&legacy_rel);
            ensure_under_root(root, &legacy_abs)?;
            if legacy_abs.is_file() {
                plan.abs = legacy_abs;
            }
        }
    }
    Ok(plan)
}

pub(crate) fn rel_from_root(root: &Path, abs: &Path) -> Option<String> {
    abs.strip_prefix(root)
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

fn mtime_secs(p: &Path) -> i64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn walk_plan_markdown(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(root) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            let canonical = path.join(PLAN_FILE);
            if canonical.is_file() {
                out.push(canonical);
            }
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let Some(file) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if file.eq_ignore_ascii_case(PLANS_INDEX) {
            out.push(path);
            continue;
        }
        if file.eq_ignore_ascii_case(PLANS_README) {
            continue;
        }
        let is_legacy_plan = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if is_legacy_plan {
            out.push(path);
        }
    }
}

pub(crate) fn extract_title(body: &str, fallback: &str) -> String {
    for line in body.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return title.to_owned();
            }
        }
    }
    fallback.to_owned()
}

fn plan_identity_from_abs(root: &Path, abs: &Path) -> Option<(String, String, String, bool)> {
    let rel = rel_from_root(root, abs)?;
    if rel.eq_ignore_ascii_case(PLANS_INDEX) {
        return Some((
            PLANS_INDEX.to_owned(),
            "PLANS".to_owned(),
            String::new(),
            true,
        ));
    }
    if rel.eq_ignore_ascii_case(PLANS_README) {
        return None;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.as_slice() {
        [file] => {
            let path = Path::new(file);
            let is_md = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false);
            if !is_md {
                return None;
            }
            let slug = path.file_stem()?.to_str()?.to_owned();
            Some((format!("{slug}/{PLAN_FILE}"), slug.clone(), slug, false))
        }
        [slug, file] if file.eq_ignore_ascii_case(PLAN_FILE) => Some((
            format!("{slug}/{PLAN_FILE}"),
            (*slug).to_owned(),
            (*slug).to_owned(),
            false,
        )),
        _ => None,
    }
}

fn meta_from_abs(root: &Path, abs: &Path) -> Option<PlanMeta> {
    let (rel, slug, folder_path, is_index) = plan_identity_from_abs(root, abs)?;
    let meta = fs::metadata(abs).ok();
    let body = fs::read_to_string(abs).unwrap_or_default();
    let fallback = if is_index { "PLANS" } else { slug.as_str() };
    let title = extract_title(&body, fallback);
    let tasks = parse_plan_tasks(&body);
    let mut summary = PlanTaskSummary::default();
    for t in &tasks {
        summary.total += 1;
        match t.status {
            TaskStatus::Pending => summary.pending += 1,
            TaskStatus::InProgress => summary.in_progress += 1,
            TaskStatus::Blocked => summary.blocked += 1,
            TaskStatus::Completed => summary.completed += 1,
            TaskStatus::Cancelled => summary.cancelled += 1,
        }
    }
    Some(PlanMeta {
        path: rel,
        name: fallback.to_owned(),
        slug,
        folder_path,
        title,
        size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        modified: mtime_secs(abs),
        is_index,
        task_summary: summary,
    })
}

pub(crate) fn meta_from_plan_file(root: &Path, abs: &Path) -> Option<PlanMeta> {
    meta_from_abs(root, abs)
}

/// Parse `## Tasks` / `## Todos` section into task entries.
pub fn parse_plan_tasks(body: &str) -> Vec<PlanTask> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            let head = rest.trim().to_ascii_lowercase();
            in_section = head == "tasks" || head == "todos";
            continue;
        }
        if trimmed.starts_with("# ") {
            // Top-level heading resets context but does not end section
            // (Markdown allows only one `# Foo`). We keep going.
            continue;
        }
        if in_section {
            if let Some(task) = parse_task_line(line) {
                out.push(task);
            }
            // A new `## …` heading flips `in_section` above.
        }
    }
    out
}

fn parse_task_line(line: &str) -> Option<PlanTask> {
    let trimmed = line.trim_start();
    let bullet = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))?;
    let after = bullet.trim_start();
    if !after.starts_with('[') {
        return None;
    }
    let close = after.find(']')?;
    if close != 2 {
        return None;
    }
    let marker_char = after.as_bytes().get(1).copied()?;
    let status = match marker_char {
        b' ' => TaskStatus::Pending,
        b'>' => TaskStatus::InProgress,
        b'!' => TaskStatus::Blocked,
        b'x' | b'X' => TaskStatus::Completed,
        b'-' => TaskStatus::Cancelled,
        _ => return None,
    };
    let rest = after[close + 1..].trim_start();
    let (id, title) = parse_id_and_title(rest)?;
    Some(PlanTask { id, title, status })
}

fn parse_id_and_title(s: &str) -> Option<(String, String)> {
    let s = s.trim_start();
    if let Some(rest) = s.strip_prefix('`') {
        let close = rest.find('`')?;
        let id = rest[..close].trim().to_owned();
        let after = rest[close + 1..].trim_start();
        let title = after
            .strip_prefix('-')
            .map(str::trim_start)
            .unwrap_or(after)
            .trim()
            .to_owned();
        if id.is_empty() {
            return None;
        }
        Some((id, title))
    } else {
        // Tolerate `task-id - title` without backticks.
        let dash = s.find(" - ")?;
        let id = s[..dash].trim().to_owned();
        let title = s[dash + 3..].trim().to_owned();
        if id.is_empty() {
            return None;
        }
        Some((id, title))
    }
}

fn status_marker(status: TaskStatus) -> char {
    match status {
        TaskStatus::Pending => ' ',
        TaskStatus::InProgress => '>',
        TaskStatus::Blocked => '!',
        TaskStatus::Completed => 'x',
        TaskStatus::Cancelled => '-',
    }
}

fn format_task_line(task: &PlanTask) -> String {
    format!(
        "- [{}] `{}` - {}",
        status_marker(task.status.clone()),
        task.id,
        task.title
    )
}

/// Replace `## Tasks` / `## Todos` section body with `new_tasks`. If no
/// section exists, append a `## Tasks` block at the end. Returns the new
/// Markdown body.
pub fn rewrite_plan_tasks(body: &str, new_tasks: &[PlanTask]) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut start: Option<usize> = None;
    let mut end: usize = lines.len();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            let head = rest.trim().to_ascii_lowercase();
            if head == "tasks" || head == "todos" {
                start = Some(i);
                end = lines.len();
                for (j, l2) in lines.iter().enumerate().skip(i + 1) {
                    let t2 = l2.trim_start();
                    if t2.starts_with("## ") || t2.starts_with("# ") {
                        end = j;
                        break;
                    }
                }
                break;
            }
        }
    }
    let new_body_lines: Vec<String> = if new_tasks.is_empty() {
        Vec::new()
    } else {
        new_tasks.iter().map(format_task_line).collect()
    };
    let new_section_header = "## Tasks";
    let new_section: Vec<String> = std::iter::once(String::new())
        .chain(std::iter::once(new_section_header.to_owned()))
        .chain(std::iter::once(String::new()))
        .chain(new_body_lines)
        .collect();

    let mut out = String::new();
    if let Some(s) = start {
        for line in &lines[..s] {
            out.push_str(line);
            out.push('\n');
        }
        out = out.trim_end_matches('\n').to_owned();
        out.push('\n');
        for ns in &new_section {
            out.push_str(ns);
            out.push('\n');
        }
        // Skip blank lines between section and the next heading
        if end < lines.len() {
            out.push('\n');
            for line in &lines[end..] {
                out.push_str(line);
                out.push('\n');
            }
        }
    } else {
        out.push_str(body);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        for ns in &new_section {
            out.push_str(ns);
            out.push('\n');
        }
    }
    out
}

// ---------------------------------------------------------------------
// Tauri commands and inner helpers

pub fn plan_list_inner(workspace_cwd: &str) -> Result<Vec<PlanMeta>, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let mut files = Vec::new();
    walk_plan_markdown(&root, &mut files);
    let mut out: Vec<PlanMeta> = files
        .iter()
        .filter_map(|abs| meta_from_abs(&root, abs))
        .collect();
    out.sort_by(|a, b| {
        // Index first, then by lower-case path.
        b.is_index
            .cmp(&a.is_index)
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
    });
    Ok(out)
}

pub fn plan_read_inner(workspace_cwd: &str, path: &str) -> Result<PlanContent, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let plan = resolve_plan_path(&root, path)?;
    let content = fs::read_to_string(&plan.abs).map_err(|e| format!("read {path}: {e}"))?;
    Ok(PlanContent {
        path: plan.rel,
        content,
        modified: mtime_secs(&plan.abs),
        is_index: plan.is_index,
    })
}

pub fn plan_create_inner(
    workspace_cwd: &str,
    path: &str,
    content: Option<&str>,
) -> Result<PlanMeta, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let plan = normalize_plan_path(&root, path)?;
    if plan.abs.exists()
        || plan
            .legacy_rel
            .as_ref()
            .map(|legacy| root.join(legacy).exists())
            .unwrap_or(false)
    {
        return err(format!("already exists: {path}"));
    }
    if let Some(parent) = plan.abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let body = content.unwrap_or("").to_owned();
    let body = if body.is_empty() {
        let title = if plan.is_index { "PLANS" } else { &plan.slug };
        format!("# {title}\n\n## Tasks\n\n")
    } else {
        body
    };
    fs::write(&plan.abs, body.as_bytes()).map_err(|e| format!("write {path}: {e}"))?;
    // Keep the PLANS.md index in sync with the new file. Best-effort: a
    // failed index rewrite must not fail the create itself.
    let _ = crate::plans_index::sync_plans_index(&root);
    meta_from_abs(&root, &plan.abs).ok_or_else(|| "failed to read back created plan".to_owned())
}

pub fn plan_write_inner(
    workspace_cwd: &str,
    path: &str,
    content: &str,
) -> Result<PlanContent, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let plan = resolve_plan_path(&root, path)?;
    if let Some(parent) = plan.abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let mut file = fs::File::create(&plan.abs).map_err(|e| format!("create {path}: {e}"))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("write {path}: {e}"))?;
    // A write can create a brand-new plan file; keep the index in sync.
    // Skip when the index itself is the target so we don't recurse.
    if !plan.is_index {
        let _ = crate::plans_index::sync_plans_index(&root);
    }
    Ok(PlanContent {
        path: plan.rel,
        content: content.to_owned(),
        modified: mtime_secs(&plan.abs),
        is_index: plan.is_index,
    })
}

pub fn plan_delete_inner(workspace_cwd: &str, path: &str) -> Result<(), String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let plan = resolve_plan_path(&root, path)?;
    if plan.is_index {
        return err("PLANS.md is the protected index and cannot be deleted");
    }
    if !plan.abs.exists() {
        return err(format!("not found: {path}"));
    }
    fs::remove_file(&plan.abs).map_err(|e| format!("delete {path}: {e}"))?;
    if !plan.folder_path.is_empty() {
        let folder = root.join(&plan.folder_path);
        ensure_under_root(&root, &folder)?;
        if folder.is_dir() {
            fs::remove_dir_all(&folder)
                .map_err(|e| format!("delete plan folder {}: {e}", plan.folder_path))?;
        }
    }
    if let Some(mut parent) = plan.abs.parent() {
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
    // Drop the deleted file's row from the PLANS.md index.
    let _ = crate::plans_index::sync_plans_index(&root);
    Ok(())
}

pub fn plan_rename_inner(
    workspace_cwd: &str,
    old_path: &str,
    new_path: &str,
) -> Result<PlanMeta, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let old_plan = resolve_plan_path(&root, old_path)?;
    let new_plan = normalize_plan_path(&root, new_path)?;
    if old_plan.is_index {
        return err("PLANS.md is the protected index and cannot be renamed");
    }
    if new_plan.is_index {
        return err("PLANS.md is the protected index and cannot be renamed");
    }
    if !old_plan.abs.exists() {
        return err(format!("not found: {old_path}"));
    }
    if new_plan.abs.exists()
        || new_plan
            .legacy_rel
            .as_ref()
            .map(|legacy| root.join(legacy).exists())
            .unwrap_or(false)
    {
        return err(format!("already exists: {new_path}"));
    }

    let old_parent = old_plan.abs.parent();
    let old_is_canonical_folder = old_parent
        .and_then(|p| p.parent())
        .map(|p| p == root)
        .unwrap_or(false)
        && old_plan
            .abs
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case(PLAN_FILE))
            .unwrap_or(false);

    if old_is_canonical_folder {
        let old_folder = old_parent.ok_or_else(|| "missing plan folder".to_owned())?;
        let new_folder = root.join(&new_plan.folder_path);
        fs::rename(old_folder, &new_folder).map_err(|e| format!("rename: {e}"))?;
    } else {
        if let Some(parent) = new_plan.abs.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        fs::rename(&old_plan.abs, &new_plan.abs).map_err(|e| format!("rename: {e}"))?;
    }

    // Plan task records that referenced the old path get rewritten too.
    tasks::tasks_rewrite_plan_path(workspace_cwd, &old_plan.rel, &new_plan.rel)?;
    if let Some(legacy_old) = old_plan.legacy_rel_for_canonical() {
        if legacy_old != old_plan.rel {
            tasks::tasks_rewrite_plan_path(workspace_cwd, &legacy_old, &new_plan.rel)?;
        }
    }
    // Rewrite the index so the row's path/link follows the rename, carrying
    // the curated status/description over to the new path. Best-effort.
    let old_index_rel = old_plan
        .legacy_rel_for_canonical()
        .filter(|legacy| root.join(legacy) == old_plan.abs)
        .unwrap_or_else(|| old_plan.rel.clone());
    let _ = crate::plans_index::sync_plans_index_after_rename(&root, &old_index_rel, &new_plan.rel);
    meta_from_abs(&root, &new_plan.abs).ok_or_else(|| "failed to read back renamed plan".to_owned())
}

/// Load plan tasks into the workspace task manager. Replaces only those
/// tasks whose `planPath == path`; free tasks remain untouched. Sets
/// `activePlanPath` on the store.
pub fn plan_load_inner(workspace_cwd: &str, path: &str) -> Result<PlanLoadReport, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let plan = resolve_plan_path(&root, path)?;
    let body = fs::read_to_string(&plan.abs).map_err(|e| format!("read {path}: {e}"))?;
    let parsed = parse_plan_tasks(&body);

    let snapshot = tasks::tasks_snapshot(workspace_cwd)?;
    let free_kept = snapshot
        .tasks
        .iter()
        .filter(|t| t.plan_path.is_none())
        .count() as u32;

    let report = tasks::tasks_replace_plan_set(workspace_cwd, &plan.rel, &parsed)?;

    Ok(PlanLoadReport {
        path: plan.rel,
        tasks_replaced: report.replaced,
        tasks_added: report.added,
        free_tasks_kept: free_kept,
    })
}

/// Write current plan-linked task state from the task manager back into the
/// plan Markdown. Useful when the agent re-orders tasks programmatically.
pub fn plan_sync_from_tasks_inner(
    workspace_cwd: &str,
    path: &str,
) -> Result<PlanSyncReport, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let plan = resolve_plan_path(&root, path)?;
    let body = fs::read_to_string(&plan.abs).map_err(|e| format!("read {path}: {e}"))?;

    let snapshot = tasks::tasks_snapshot(workspace_cwd)?;
    let plan_tasks: Vec<PlanTask> = snapshot
        .tasks
        .iter()
        .filter(|t| {
            t.plan_path.as_deref() == Some(plan.rel.as_str())
                || plan
                    .legacy_rel_for_canonical()
                    .as_deref()
                    .is_some_and(|legacy| t.plan_path.as_deref() == Some(legacy))
        })
        .map(|t| {
            let id = t.plan_task_id.clone().unwrap_or_else(|| t.id.clone());
            PlanTask {
                id,
                title: t.title.clone(),
                status: t.status.clone(),
            }
        })
        .collect();

    let new_body = rewrite_plan_tasks(&body, &plan_tasks);
    fs::write(&plan.abs, new_body.as_bytes()).map_err(|e| format!("write {path}: {e}"))?;
    Ok(PlanSyncReport {
        path: plan.rel,
        tasks_written: plan_tasks.len() as u32,
    })
}

/// Write a single task's status change back into the plan Markdown without
/// touching other lines. Used by task_update on plan-linked tasks.
pub fn plan_write_back_task_status(
    workspace_cwd: &str,
    plan_path: &str,
    plan_task_id: &str,
    status: TaskStatus,
) -> Result<(), String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let plan = resolve_plan_path(&root, plan_path)?;
    let body = fs::read_to_string(&plan.abs).map_err(|e| format!("read {plan_path}: {e}"))?;
    let mut tasks_parsed = parse_plan_tasks(&body);
    let mut found = false;
    for t in &mut tasks_parsed {
        if t.id == plan_task_id {
            t.status = status.clone();
            found = true;
            break;
        }
    }
    if !found {
        // Append the task line so the plan stays in sync.
        tasks_parsed.push(PlanTask {
            id: plan_task_id.to_owned(),
            title: String::new(),
            status: status.clone(),
        });
    }
    let new_body = rewrite_plan_tasks(&body, &tasks_parsed);
    fs::write(&plan.abs, new_body.as_bytes()).map_err(|e| format!("write {plan_path}: {e}"))?;
    Ok(())
}

fn current_migration_progress(
    state: &PlanMigrationState,
    workspace_cwd: &str,
) -> Result<PlanMigrationProgress, String> {
    let inner = state.inner.lock().map_err(lock_err)?;
    Ok(inner.get(workspace_cwd).cloned().unwrap_or_default())
}

fn set_migration_progress(
    state: &Arc<Mutex<HashMap<String, PlanMigrationProgress>>>,
    workspace_cwd: &str,
    update: impl FnOnce(&mut PlanMigrationProgress),
) -> Result<PlanMigrationProgress, String> {
    let mut inner = state.lock().map_err(lock_err)?;
    let progress = inner.entry(workspace_cwd.to_owned()).or_default();
    update(progress);
    progress.updated_at_ms = now_ms();
    Ok(progress.clone())
}

fn discover_legacy_plan_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(read) = fs::read_dir(root) else {
        return files;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case(PLANS_INDEX) || name.eq_ignore_ascii_case(PLANS_README) {
            continue;
        }
        let is_md = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("md"))
            .unwrap_or(false);
        if is_md {
            files.push(path);
        }
    }
    files.sort_by(|a, b| {
        a.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_lowercase()
            .cmp(
                &b.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_lowercase(),
            )
    });
    files
}

fn unique_plan_target_rel(root: &Path, slug: &str, source_legacy_rel: &str) -> String {
    let mut candidate = slug.to_owned();
    for n in 2..10_000 {
        let rel = format!("{candidate}/{PLAN_FILE}");
        let legacy_rel = format!("{candidate}.md");
        let legacy_blocks = legacy_rel != source_legacy_rel && root.join(&legacy_rel).exists();
        if !root.join(&rel).exists() && !legacy_blocks {
            return rel;
        }
        candidate = format!("{slug}-{n}");
    }
    format!("{slug}-{}", now_ms())
}

fn migrate_legacy_plan_files(
    workspace_cwd: &str,
    root: &Path,
    state: Arc<Mutex<HashMap<String, PlanMigrationProgress>>>,
) -> Result<(), String> {
    let legacy_files = discover_legacy_plan_files(root);
    let total = legacy_files.len() as u32;
    set_migration_progress(&state, workspace_cwd, |progress| {
        progress.phase = if total == 0 {
            "done".into()
        } else {
            "migrating".into()
        };
        progress.busy = total > 0;
        progress.total = total;
        progress.processed = 0;
        progress.migrated = 0;
        progress.skipped = 0;
        progress.error = None;
    })?;
    if legacy_files.is_empty() {
        return Ok(());
    }

    let mut mapping: Vec<(String, String)> = Vec::new();
    for legacy_abs in legacy_files {
        let old_rel = rel_from_root(root, &legacy_abs).unwrap_or_default();
        let Some(slug) = legacy_abs.file_stem().and_then(|s| s.to_str()) else {
            set_migration_progress(&state, workspace_cwd, |progress| {
                progress.processed = progress.processed.saturating_add(1);
                progress.skipped = progress.skipped.saturating_add(1);
            })?;
            continue;
        };
        let target_rel = unique_plan_target_rel(root, slug, &old_rel);
        let target_abs = root.join(&target_rel);
        if let Some(parent) = target_abs.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        fs::rename(&legacy_abs, &target_abs)
            .map_err(|e| format!("migrate {old_rel} -> {target_rel}: {e}"))?;
        mapping.push((old_rel, target_rel));
        set_migration_progress(&state, workspace_cwd, |progress| {
            progress.processed = progress.processed.saturating_add(1);
            progress.migrated = progress.migrated.saturating_add(1);
        })?;
    }

    set_migration_progress(&state, workspace_cwd, |progress| {
        progress.phase = "rewriting".into();
    })?;
    for (old_path, new_path) in &mapping {
        tasks::tasks_rewrite_plan_path(workspace_cwd, old_path, new_path)?;
    }
    kanban::kanban_rewrite_plan_paths(workspace_cwd, &mapping)?;
    crate::plans_index::sync_plans_index(root)?;

    set_migration_progress(&state, workspace_cwd, |progress| {
        progress.phase = "done".into();
        progress.busy = false;
        progress.error = None;
    })?;
    Ok(())
}

fn mark_migration_error(
    state: &Arc<Mutex<HashMap<String, PlanMigrationProgress>>>,
    workspace_cwd: &str,
    error: String,
) {
    let _ = set_migration_progress(state, workspace_cwd, |progress| {
        progress.phase = "error".into();
        progress.busy = false;
        progress.error = Some(error);
    });
}

#[tauri::command]
pub fn plan_list(workspace_cwd: String) -> Result<Vec<PlanMeta>, String> {
    plan_list_inner(&workspace_cwd)
}

#[tauri::command]
pub fn plan_migration_poll(
    workspace_cwd: String,
    state: tauri::State<'_, PlanMigrationState>,
) -> Result<PlanMigrationProgress, String> {
    current_migration_progress(&state, &workspace_cwd)
}

#[tauri::command]
pub fn plan_migration_ensure_started(
    workspace_cwd: String,
    state: tauri::State<'_, PlanMigrationState>,
) -> Result<PlanMigrationProgress, String> {
    let root = ensure_plans_root(&workspace_cwd)?;
    let state_inner = state.inner.clone();
    let current = current_migration_progress(&state, &workspace_cwd)?;
    if current.busy {
        return Ok(current);
    }

    let legacy_files = discover_legacy_plan_files(&root);
    if legacy_files.is_empty() {
        return set_migration_progress(&state_inner, &workspace_cwd, |progress| {
            progress.phase = "done".into();
            progress.busy = false;
            progress.total = 0;
            progress.processed = 0;
            progress.migrated = 0;
            progress.skipped = 0;
            progress.error = None;
        });
    }

    let total = legacy_files.len() as u32;
    let initial = set_migration_progress(&state_inner, &workspace_cwd, |progress| {
        progress.phase = "migrating".into();
        progress.busy = true;
        progress.total = total;
        progress.processed = 0;
        progress.migrated = 0;
        progress.skipped = 0;
        progress.error = None;
    })?;

    let ws = workspace_cwd.clone();
    let state_for_task = state_inner.clone();
    tauri::async_runtime::spawn(async move {
        let state_for_work = state_for_task.clone();
        let ws_for_work = ws.clone();
        let result = crate::proc::run_blocking(move || {
            migrate_legacy_plan_files(&ws_for_work, &root, state_for_work)
        })
        .await;
        if let Err(error) = result {
            mark_migration_error(&state_for_task, &ws, error);
        }
    });

    Ok(initial)
}

#[tauri::command]
pub fn plan_read(workspace_cwd: String, path: String) -> Result<PlanContent, String> {
    plan_read_inner(&workspace_cwd, &path)
}

#[tauri::command]
pub fn plan_create(
    workspace_cwd: String,
    path: String,
    content: Option<String>,
) -> Result<PlanMeta, String> {
    plan_create_inner(&workspace_cwd, &path, content.as_deref())
}

#[tauri::command]
pub fn plan_write(
    workspace_cwd: String,
    path: String,
    content: String,
) -> Result<PlanContent, String> {
    plan_write_inner(&workspace_cwd, &path, &content)
}

#[tauri::command]
pub fn plan_delete(workspace_cwd: String, path: String) -> Result<(), String> {
    plan_delete_inner(&workspace_cwd, &path)
}

#[tauri::command]
pub fn plan_rename(
    workspace_cwd: String,
    old_path: String,
    new_path: String,
) -> Result<PlanMeta, String> {
    plan_rename_inner(&workspace_cwd, &old_path, &new_path)
}

#[tauri::command]
pub fn plan_load(workspace_cwd: String, path: String) -> Result<PlanLoadReport, String> {
    plan_load_inner(&workspace_cwd, &path)
}

#[tauri::command]
pub fn plan_sync_from_tasks(workspace_cwd: String, path: String) -> Result<PlanSyncReport, String> {
    plan_sync_from_tasks_inner(&workspace_cwd, &path)
}

/// Helper used by render_context_prompt and the terminal handoff renderer
/// to summarise an attached plan from its on-disk meta.
pub fn plan_meta_for(workspace_cwd: &str, path: &str) -> Option<PlanMeta> {
    let root = ensure_plans_root(workspace_cwd).ok()?;
    let plan = resolve_plan_path(&root, path).ok()?;
    meta_from_abs(&root, &plan.abs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents_layout::PLANS_REL;
    use crate::app_paths::test_support::AppDataDirGuard;
    use std::time::SystemTime;

    fn temp_ws(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "blxcode_plans_test_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Tests that touch the tasks store need an app-data dir override
    /// because tasks now live under `{app_data_dir}/tasks/<hash>/`.
    fn temp_ws_with_tasks(label: &str) -> (PathBuf, AppDataDirGuard) {
        let ws = temp_ws(label);
        let app_data = ws.with_extension("appdata");
        let _ = fs::remove_dir_all(&app_data);
        fs::create_dir_all(&app_data).unwrap();
        let guard = AppDataDirGuard::new(app_data);
        (ws, guard)
    }

    #[test]
    fn plans_root_is_seeded_with_index() {
        let ws = temp_ws("seed");
        let _ = plan_list_inner(&ws.to_string_lossy()).unwrap();
        assert!(ws.join(PLANS_REL).join(PLANS_INDEX).is_file());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn create_read_list_delete_round_trip() {
        let ws = temp_ws("crud");
        let cwd = ws.to_string_lossy().into_owned();

        let meta = plan_create_inner(&cwd, "my-plan.md", None).unwrap();
        assert_eq!(meta.path, "my-plan/plan.md");
        assert_eq!(meta.slug, "my-plan");
        assert_eq!(meta.folder_path, "my-plan");
        assert!(meta.title.contains("my-plan"));

        let content = plan_read_inner(&cwd, "my-plan.md").unwrap();
        assert_eq!(content.path, "my-plan/plan.md");
        assert!(content.content.contains("## Tasks"));

        plan_write_inner(
            &cwd,
            "my-plan.md",
            "# Plan A\n\n## Tasks\n\n- [ ] `t-1` - First\n- [x] `t-2` - Done\n",
        )
        .unwrap();
        let list = plan_list_inner(&cwd).unwrap();
        // Index first, then plan
        assert!(list.iter().any(|m| m.path == "my-plan/plan.md"));
        let plan = list.iter().find(|m| m.path == "my-plan/plan.md").unwrap();
        assert_eq!(plan.task_summary.total, 2);
        assert_eq!(plan.task_summary.completed, 1);

        plan_delete_inner(&cwd, "my-plan.md").unwrap();
        assert!(plan_list_inner(&cwd)
            .unwrap()
            .iter()
            .all(|m| m.path != "my-plan/plan.md"));

        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn delete_plan_removes_persisted_mermaid_diagrams() {
        let ws = temp_ws("delete_mermaids");
        let cwd = ws.to_string_lossy().into_owned();

        let meta = plan_create_inner(&cwd, "diagram-plan.md", Some("# Diagram Plan\n")).unwrap();
        crate::agent::mermaid::store::create_diagram(
            &cwd,
            &meta.slug,
            "Auth Flow",
            "flowchart TD\n A-->B",
            "flowchart",
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let plan_folder = ws.join(PLANS_REL).join(&meta.folder_path);
        assert!(plan_folder.join("plan.md").is_file());
        assert!(plan_folder.join("diagrams").join("diagrams.json").is_file());

        plan_delete_inner(&cwd, "diagram-plan.md").unwrap();

        assert!(
            !plan_folder.exists(),
            "plan folder and diagram sidecars should be deleted"
        );
        assert!(
            crate::agent::mermaid::store::list_diagrams(&cwd, &meta.slug)
                .unwrap()
                .is_empty()
        );

        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn list_ignores_readme_and_plan_sidecars() {
        let ws = temp_ws("sidecars");
        let cwd = ws.to_string_lossy().into_owned();
        let plans_root = ws.join(PLANS_REL);

        plan_create_inner(&cwd, "with-sidecar.md", Some("# With Sidecar\n")).unwrap();
        fs::write(
            plans_root.join("with-sidecar").join("notes.md"),
            "# Sidecar\n",
        )
        .unwrap();
        fs::write(
            plans_root.join("with-sidecar").join(PLANS_INDEX),
            "# Nested index\n",
        )
        .unwrap();

        let list = plan_list_inner(&cwd).unwrap();
        assert!(list.iter().any(|m| m.path == "with-sidecar/plan.md"));
        assert!(list.iter().all(|m| m.path != "README.md"));
        assert!(list.iter().all(|m| m.path != "with-sidecar/notes.md"));
        assert!(list.iter().all(|m| m.path != "with-sidecar/PLANS.md"));

        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn migration_moves_legacy_plans_and_rewrites_references() {
        use crate::kanban::{kanban_board_load_inner, kanban_layout_save_inner, KanbanLayout};
        use crate::tasks::tasks_snapshot;

        let (ws, _guard) = temp_ws_with_tasks("migration_refs");
        let cwd = ws.to_string_lossy().into_owned();
        let root = ensure_plans_root(&cwd).unwrap();
        fs::write(
            root.join("legacy.md"),
            "# Legacy\n\n## Tasks\n\n- [ ] `a` - Old task\n",
        )
        .unwrap();
        fs::write(
            root.join(PLANS_INDEX),
            "# Plans\n\n## Index\n\n| Status | Plan | Description |\n|--------|------|-------------|\n| done | [legacy.md](legacy.md) | Curated legacy row |\n",
        )
        .unwrap();

        let task_root = crate::app_paths::tasks_root_for(&cwd).unwrap();
        fs::write(
            task_root.join("index.json"),
            serde_json::json!({
                "version": 1,
                "workspaceRoot": cwd.clone(),
                "activePlanPath": "legacy.md",
                "tasks": [{
                    "id": "task-1",
                    "title": "Old task",
                    "description": "",
                    "status": "pending",
                    "position": 0,
                    "createdAt": 1,
                    "updatedAt": 1,
                    "completedAt": null,
                    "parentId": null,
                    "notes": null,
                    "planPath": "legacy.md",
                    "planTaskId": "a"
                }]
            })
            .to_string(),
        )
        .unwrap();

        let mut layout = KanbanLayout::default();
        layout.expanded_plans.push("legacy.md".into());
        layout.plan_order.insert("legacy.md".into(), 4);
        kanban_layout_save_inner(&cwd, layout).unwrap();

        let state = Arc::new(Mutex::new(HashMap::new()));
        migrate_legacy_plan_files(&cwd, &root, state).unwrap();

        assert!(!root.join("legacy.md").exists());
        assert!(root.join("legacy").join(PLAN_FILE).is_file());
        assert!(root.join(PLANS_INDEX).is_file());
        assert!(!root.join("legacy").join(PLANS_INDEX).exists());
        let index = fs::read_to_string(root.join(PLANS_INDEX)).unwrap();
        assert!(
            index.contains("| done | [legacy/plan.md](legacy/plan.md) | Curated legacy row |"),
            "index did not preserve curated row: {index}"
        );

        let snap = tasks_snapshot(&cwd).unwrap();
        assert_eq!(snap.active_plan_path.as_deref(), Some("legacy/plan.md"));
        assert_eq!(
            snap.tasks
                .first()
                .and_then(|task| task.plan_path.as_deref()),
            Some("legacy/plan.md")
        );

        let board = kanban_board_load_inner(&cwd).unwrap();
        assert_eq!(board.layout.expanded_plans, vec!["legacy/plan.md"]);
        assert_eq!(board.layout.plan_order.get("legacy/plan.md"), Some(&4));

        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn create_delete_keep_plans_index_in_sync() {
        let ws = temp_ws("index_sync");
        let cwd = ws.to_string_lossy().into_owned();
        let index = ws.join(PLANS_REL).join(PLANS_INDEX);

        plan_create_inner(&cwd, "synced.md", Some("# Synced Plan\n")).unwrap();
        let body = fs::read_to_string(&index).unwrap();
        assert!(
            body.contains("[synced/plan.md](synced/plan.md)"),
            "index missing row after create: {body}"
        );
        assert!(body.contains("Synced Plan"));

        plan_delete_inner(&cwd, "synced.md").unwrap();
        let body = fs::read_to_string(&index).unwrap();
        assert!(
            !body.contains("synced/plan.md"),
            "index kept row after delete: {body}"
        );
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn rename_updates_index_and_keeps_curated_cells() {
        let (ws, _guard) = temp_ws_with_tasks("index_rename");
        let cwd = ws.to_string_lossy().into_owned();
        let index = ws.join(PLANS_REL).join(PLANS_INDEX);

        plan_create_inner(&cwd, "before.md", Some("# Before\n")).unwrap();
        // Mark the row as done with a curated description.
        let body = fs::read_to_string(&index).unwrap();
        let edited = body.replace(
            "| planned | [before/plan.md](before/plan.md) | Before |",
            "| done | [before/plan.md](before/plan.md) | Curated |",
        );
        fs::write(&index, edited).unwrap();

        plan_rename_inner(&cwd, "before.md", "after.md").unwrap();
        let body = fs::read_to_string(&index).unwrap();
        assert!(
            body.contains("| done | [after/plan.md](after/plan.md) | Curated |"),
            "rename lost curated cells: {body}"
        );
        assert!(!body.contains("before/plan.md"));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn cannot_delete_or_rename_plans_index() {
        let ws = temp_ws("idx");
        let cwd = ws.to_string_lossy().into_owned();
        let _ = plan_list_inner(&cwd).unwrap();
        assert!(plan_delete_inner(&cwd, "PLANS.md").is_err());
        assert!(plan_rename_inner(&cwd, "PLANS.md", "INDEX.md").is_err());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn path_sandbox_rejects_traversal() {
        let ws = temp_ws("sandbox");
        let cwd = ws.to_string_lossy().into_owned();
        assert!(plan_create_inner(&cwd, "../escape.md", None).is_err());
        assert!(plan_create_inner(&cwd, "/abs.md", None).is_ok()); // strips leading slash
        assert!(plan_create_inner(&cwd, "notes.txt", None).is_err());
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn parser_extracts_status_markers() {
        let body = "# Foo\n\n## Tasks\n\n- [ ] `a` - pending\n- [>] `b` - active\n- [!] `c` - blocked\n- [x] `d` - done\n- [-] `e` - cancelled\n";
        let tasks = parse_plan_tasks(body);
        assert_eq!(tasks.len(), 5);
        assert!(matches!(tasks[0].status, TaskStatus::Pending));
        assert!(matches!(tasks[1].status, TaskStatus::InProgress));
        assert!(matches!(tasks[2].status, TaskStatus::Blocked));
        assert!(matches!(tasks[3].status, TaskStatus::Completed));
        assert!(matches!(tasks[4].status, TaskStatus::Cancelled));
        assert_eq!(tasks[3].id, "d");
        assert_eq!(tasks[3].title, "done");
    }

    #[test]
    fn parser_accepts_todos_alias() {
        let body = "## Todos\n\n- [ ] `t1` - one\n";
        let tasks = parse_plan_tasks(body);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "t1");
    }

    #[test]
    fn load_replaces_only_plan_tasks_and_keeps_free_tasks() {
        use crate::tasks::{tasks_create_inner, tasks_snapshot, TaskCreateInput, TaskStatus};
        let (ws, _guard) = temp_ws_with_tasks("load_keep_free");
        let cwd = ws.to_string_lossy().into_owned();

        // Create a free task (no plan link).
        let _ = tasks_create_inner(
            &cwd,
            TaskCreateInput {
                title: "Free task".into(),
                description: None,
                status: Some(TaskStatus::Pending),
                parent_id: None,
                notes: None,
            },
        )
        .unwrap();

        // Create plan with two tasks.
        plan_create_inner(
            &cwd,
            "demo.md",
            Some("# Demo\n\n## Tasks\n\n- [ ] `a` - First\n- [>] `b` - Active\n"),
        )
        .unwrap();

        let report = plan_load_inner(&cwd, "demo.md").unwrap();
        assert_eq!(report.tasks_added, 2);
        assert_eq!(report.free_tasks_kept, 1);

        let snap = tasks_snapshot(&cwd).unwrap();
        assert_eq!(snap.active_plan_path.as_deref(), Some("demo/plan.md"));
        let free_count = snap.tasks.iter().filter(|t| t.plan_path.is_none()).count();
        let plan_count = snap.tasks.iter().filter(|t| t.plan_path.is_some()).count();
        assert_eq!(free_count, 1);
        assert_eq!(plan_count, 2);

        // Re-load — should replace the plan tasks but leave the free task.
        let report = plan_load_inner(&cwd, "demo.md").unwrap();
        assert_eq!(report.tasks_replaced, 2);
        assert_eq!(report.tasks_added, 2);
        let snap = tasks_snapshot(&cwd).unwrap();
        assert_eq!(
            snap.tasks.iter().filter(|t| t.plan_path.is_none()).count(),
            1
        );
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn task_update_status_writes_back_to_plan_markdown() {
        use crate::tasks::{tasks_snapshot, tasks_update_inner, TaskStatus, TaskUpdatePatch};
        let (ws, _guard) = temp_ws_with_tasks("writeback");
        let cwd = ws.to_string_lossy().into_owned();

        plan_create_inner(
            &cwd,
            "writeback.md",
            Some("# WB\n\n## Tasks\n\n- [ ] `t-1` - One\n"),
        )
        .unwrap();
        plan_load_inner(&cwd, "writeback.md").unwrap();

        let snap = tasks_snapshot(&cwd).unwrap();
        let task = snap
            .tasks
            .iter()
            .find(|t| t.plan_path.as_deref() == Some("writeback/plan.md"))
            .cloned()
            .unwrap();
        tasks_update_inner(
            &cwd,
            &task.id,
            TaskUpdatePatch {
                title: None,
                description: None,
                status: Some(TaskStatus::Completed),
                parent_id: None,
                notes: None,
            },
        )
        .unwrap();

        let body = fs::read_to_string(ws.join(".agents/plans/writeback/plan.md")).unwrap();
        assert!(body.contains("- [x] `t-1`"), "got body: {body}");
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn sync_from_tasks_round_trip_preserves_other_sections() {
        use crate::tasks::{tasks_snapshot, tasks_update_inner, TaskStatus, TaskUpdatePatch};
        let (ws, _guard) = temp_ws_with_tasks("sync");
        let cwd = ws.to_string_lossy().into_owned();

        plan_create_inner(
            &cwd,
            "sync.md",
            Some(
                "# Sync\n\n## Summary\n\nKeep me.\n\n## Tasks\n\n- [ ] `t-1` - One\n- [ ] `t-2` - Two\n\n## Notes\n\nBye\n",
            ),
        )
        .unwrap();
        plan_load_inner(&cwd, "sync.md").unwrap();
        let snap = tasks_snapshot(&cwd).unwrap();
        let task = snap
            .tasks
            .iter()
            .find(|t| t.plan_task_id.as_deref() == Some("t-2"))
            .cloned()
            .unwrap();
        tasks_update_inner(
            &cwd,
            &task.id,
            TaskUpdatePatch {
                title: None,
                description: None,
                status: Some(TaskStatus::InProgress),
                parent_id: None,
                notes: None,
            },
        )
        .unwrap();
        let rep = plan_sync_from_tasks_inner(&cwd, "sync.md").unwrap();
        assert_eq!(rep.tasks_written, 2);
        let body = fs::read_to_string(ws.join(".agents/plans/sync/plan.md")).unwrap();
        assert!(body.contains("## Summary"));
        assert!(body.contains("## Notes"));
        assert!(body.contains("- [>] `t-2` - Two"));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn rewrite_round_trip_preserves_other_sections() {
        let body = "# Plan\n\n## Summary\n\nSome notes.\n\n## Tasks\n\n- [ ] `a` - one\n\n## Notes\n\nOther.\n";
        let new_tasks = vec![
            PlanTask {
                id: "a".into(),
                title: "one".into(),
                status: TaskStatus::Completed,
            },
            PlanTask {
                id: "b".into(),
                title: "two".into(),
                status: TaskStatus::Pending,
            },
        ];
        let new_body = rewrite_plan_tasks(body, &new_tasks);
        assert!(new_body.contains("## Summary"));
        assert!(new_body.contains("## Notes"));
        assert!(new_body.contains("- [x] `a` - one"));
        assert!(new_body.contains("- [ ] `b` - two"));
        let reparsed = parse_plan_tasks(&new_body);
        assert_eq!(reparsed.len(), 2);
    }
}
