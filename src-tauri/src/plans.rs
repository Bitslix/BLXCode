//! Workspace-scoped Markdown plans.
//!
//! Layout per workspace:
//!
//! ```text
//! <workspace_cwd>/.agents/plans/        — durable Markdown plans
//!   PLANS.md                            — index (never deleted)
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
use crate::tasks;
use crate::tasks::TaskStatus;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMeta {
    /// Relative path within the plans root, forward slashes, ends in `.md`.
    pub path: String,
    /// Basename without extension.
    pub name: String,
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

fn err<T>(s: impl Into<String>) -> Result<T, String> {
    Err(s.into())
}

fn ensure_plans_root(ws: &str) -> Result<PathBuf, String> {
    let roots = ensure_agents_layout(ws)?;
    Ok(roots.plans)
}

/// Validate a plan API path: relative, no `..`, `.md` extension, lives under root.
fn safe_plan_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
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
    let is_md = candidate
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("md"))
        .unwrap_or(false);
    if !is_md {
        return err("plan paths must end in .md");
    }
    let abs = root.join(&candidate);
    let canon_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let canon_abs = abs
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        .map(|p| p.join(abs.file_name().unwrap_or_default()))
        .unwrap_or_else(|| abs.clone());
    if !canon_abs.starts_with(&canon_root) {
        return err("path escapes plans root");
    }
    Ok(abs)
}

fn rel_from_root(root: &Path, abs: &Path) -> Option<String> {
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

fn extract_title(body: &str, fallback: &str) -> String {
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

fn meta_from_abs(root: &Path, abs: &Path) -> Option<PlanMeta> {
    let rel = rel_from_root(root, abs)?;
    let meta = fs::metadata(abs).ok();
    let body = fs::read_to_string(abs).unwrap_or_default();
    let basename = abs
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();
    let title = extract_title(&body, &basename);
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
    let is_index = rel.eq_ignore_ascii_case(PLANS_INDEX);
    Some(PlanMeta {
        path: rel,
        name: basename,
        title,
        size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        modified: mtime_secs(abs),
        is_index,
        task_summary: summary,
    })
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
    walk_md(&root, &mut files);
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
    let abs = safe_plan_path(&root, path)?;
    let content = fs::read_to_string(&abs).map_err(|e| format!("read {path}: {e}"))?;
    let rel = rel_from_root(&root, &abs).unwrap_or_else(|| path.to_owned());
    let is_index = rel.eq_ignore_ascii_case(PLANS_INDEX);
    Ok(PlanContent {
        path: rel,
        content,
        modified: mtime_secs(&abs),
        is_index,
    })
}

pub fn plan_create_inner(
    workspace_cwd: &str,
    path: &str,
    content: Option<&str>,
) -> Result<PlanMeta, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let abs = safe_plan_path(&root, path)?;
    if abs.exists() {
        return err(format!("already exists: {path}"));
    }
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let body = content.unwrap_or("").to_owned();
    let body = if body.is_empty() {
        let stem = abs.file_stem().and_then(|s| s.to_str()).unwrap_or("Plan");
        format!("# {stem}\n\n## Tasks\n\n")
    } else {
        body
    };
    fs::write(&abs, body.as_bytes()).map_err(|e| format!("write {path}: {e}"))?;
    meta_from_abs(&root, &abs).ok_or_else(|| "failed to read back created plan".to_owned())
}

pub fn plan_write_inner(
    workspace_cwd: &str,
    path: &str,
    content: &str,
) -> Result<PlanContent, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let abs = safe_plan_path(&root, path)?;
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let mut file = fs::File::create(&abs).map_err(|e| format!("create {path}: {e}"))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("write {path}: {e}"))?;
    let rel = rel_from_root(&root, &abs).unwrap_or_else(|| path.to_owned());
    let is_index = rel.eq_ignore_ascii_case(PLANS_INDEX);
    Ok(PlanContent {
        path: rel,
        content: content.to_owned(),
        modified: mtime_secs(&abs),
        is_index,
    })
}

pub fn plan_delete_inner(workspace_cwd: &str, path: &str) -> Result<(), String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let abs = safe_plan_path(&root, path)?;
    let rel = rel_from_root(&root, &abs).unwrap_or_else(|| path.to_owned());
    if rel.eq_ignore_ascii_case(PLANS_INDEX) {
        return err("PLANS.md is the protected index and cannot be deleted");
    }
    if !abs.exists() {
        return err(format!("not found: {path}"));
    }
    fs::remove_file(&abs).map_err(|e| format!("delete {path}: {e}"))?;
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

pub fn plan_rename_inner(
    workspace_cwd: &str,
    old_path: &str,
    new_path: &str,
) -> Result<PlanMeta, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let abs_old = safe_plan_path(&root, old_path)?;
    let abs_new = safe_plan_path(&root, new_path)?;
    let rel_old = rel_from_root(&root, &abs_old).unwrap_or_else(|| old_path.to_owned());
    if rel_old.eq_ignore_ascii_case(PLANS_INDEX) {
        return err("PLANS.md is the protected index and cannot be renamed");
    }
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
    // Plan task records that referenced the old path get rewritten too.
    tasks::tasks_rewrite_plan_path(
        workspace_cwd,
        &rel_from_root(&root, &abs_old).unwrap_or_default(),
        &rel_from_root(&root, &abs_new).unwrap_or_default(),
    )?;
    meta_from_abs(&root, &abs_new).ok_or_else(|| "failed to read back renamed plan".to_owned())
}

/// Load plan tasks into the workspace task manager. Replaces only those
/// tasks whose `planPath == path`; free tasks remain untouched. Sets
/// `activePlanPath` on the store.
pub fn plan_load_inner(workspace_cwd: &str, path: &str) -> Result<PlanLoadReport, String> {
    let root = ensure_plans_root(workspace_cwd)?;
    let abs = safe_plan_path(&root, path)?;
    let body = fs::read_to_string(&abs).map_err(|e| format!("read {path}: {e}"))?;
    let rel = rel_from_root(&root, &abs).unwrap_or_else(|| path.to_owned());
    let parsed = parse_plan_tasks(&body);

    let snapshot = tasks::tasks_snapshot(workspace_cwd)?;
    let free_kept = snapshot
        .tasks
        .iter()
        .filter(|t| t.plan_path.is_none())
        .count() as u32;

    let report = tasks::tasks_replace_plan_set(workspace_cwd, &rel, &parsed)?;

    Ok(PlanLoadReport {
        path: rel,
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
    let abs = safe_plan_path(&root, path)?;
    let rel = rel_from_root(&root, &abs).unwrap_or_else(|| path.to_owned());
    let body = fs::read_to_string(&abs).map_err(|e| format!("read {path}: {e}"))?;

    let snapshot = tasks::tasks_snapshot(workspace_cwd)?;
    let plan_tasks: Vec<PlanTask> = snapshot
        .tasks
        .iter()
        .filter(|t| t.plan_path.as_deref() == Some(rel.as_str()))
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
    fs::write(&abs, new_body.as_bytes()).map_err(|e| format!("write {path}: {e}"))?;
    Ok(PlanSyncReport {
        path: rel,
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
    let abs = safe_plan_path(&root, plan_path)?;
    let body = fs::read_to_string(&abs).map_err(|e| format!("read {plan_path}: {e}"))?;
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
    fs::write(&abs, new_body.as_bytes()).map_err(|e| format!("write {plan_path}: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn plan_list(workspace_cwd: String) -> Result<Vec<PlanMeta>, String> {
    plan_list_inner(&workspace_cwd)
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
    let abs = safe_plan_path(&root, path).ok()?;
    meta_from_abs(&root, &abs)
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
        assert_eq!(meta.path, "my-plan.md");
        assert!(meta.title.contains("my-plan"));

        let content = plan_read_inner(&cwd, "my-plan.md").unwrap();
        assert!(content.content.contains("## Tasks"));

        plan_write_inner(
            &cwd,
            "my-plan.md",
            "# Plan A\n\n## Tasks\n\n- [ ] `t-1` - First\n- [x] `t-2` - Done\n",
        )
        .unwrap();
        let list = plan_list_inner(&cwd).unwrap();
        // Index first, then plan
        assert!(list.iter().any(|m| m.path == "my-plan.md"));
        let plan = list.iter().find(|m| m.path == "my-plan.md").unwrap();
        assert_eq!(plan.task_summary.total, 2);
        assert_eq!(plan.task_summary.completed, 1);

        plan_delete_inner(&cwd, "my-plan.md").unwrap();
        assert!(plan_list_inner(&cwd)
            .unwrap()
            .iter()
            .all(|m| m.path != "my-plan.md"));

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
        assert_eq!(snap.active_plan_path.as_deref(), Some("demo.md"));
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
            .find(|t| t.plan_path.as_deref() == Some("writeback.md"))
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

        let body = fs::read_to_string(ws.join(".agents/plans/writeback.md")).unwrap();
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
        let body = fs::read_to_string(ws.join(".agents/plans/sync.md")).unwrap();
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
