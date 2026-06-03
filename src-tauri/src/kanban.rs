//! Workspace Multi-Kanban over durable `.agents/plans/*.md` files.
//!
//! Plan Markdown remains the source of truth. This module stores only layout
//! metadata under `.agents/kanban/index.json` and uses the existing plan task
//! parser/rewriter for all content mutations.

use crate::agents_layout::{ensure_agents_layout, PLANS_INDEX};
use crate::plans::{self, PlanMeta, PlanTask};
use crate::proc;
use crate::tasks::{self, TaskStatus, TaskUpdatePatch};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const KANBAN_INDEX: &str = "index.json";
const KANBAN_STORE_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KanbanPlanState {
    Blocked,
    InProgress,
    Pending,
    Completed,
    Cancelled,
    Empty,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KanbanLayout {
    pub version: u32,
    #[serde(default)]
    pub workspace_root: Option<String>,
    pub plan_section_order: Vec<KanbanPlanState>,
    pub collapsed_plan_sections: Vec<KanbanPlanState>,
    pub expanded_plans: Vec<String>,
    pub task_lane_order: Vec<TaskStatus>,
    pub collapsed_task_lanes: Vec<TaskStatus>,
    pub plan_order: BTreeMap<String, u32>,
    pub task_order: BTreeMap<String, u32>,
    pub filters: KanbanFilters,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KanbanFilters {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub show_completed: bool,
    #[serde(default)]
    pub show_cancelled: bool,
    #[serde(default)]
    pub active_only: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanBoard {
    pub layout: KanbanLayout,
    pub plans: Vec<KanbanPlanNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanPlanNode {
    pub meta: PlanMeta,
    pub state: KanbanPlanState,
    pub tasks: Vec<KanbanTaskCard>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanTaskCard {
    pub plan_path: String,
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_task_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanTaskCreateInput {
    pub plan_path: String,
    pub title: String,
    #[serde(default)]
    pub status: Option<TaskStatus>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanTaskUpdatePatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<TaskStatus>,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn default_plan_section_order() -> Vec<KanbanPlanState> {
    vec![
        KanbanPlanState::Blocked,
        KanbanPlanState::InProgress,
        KanbanPlanState::Pending,
        KanbanPlanState::Completed,
        KanbanPlanState::Cancelled,
        KanbanPlanState::Empty,
    ]
}

fn default_task_lane_order() -> Vec<TaskStatus> {
    vec![
        TaskStatus::Pending,
        TaskStatus::InProgress,
        TaskStatus::Blocked,
        TaskStatus::Completed,
        TaskStatus::Cancelled,
    ]
}

impl Default for KanbanLayout {
    fn default() -> Self {
        Self {
            version: KANBAN_STORE_VERSION,
            workspace_root: None,
            plan_section_order: default_plan_section_order(),
            collapsed_plan_sections: Vec::new(),
            expanded_plans: Vec::new(),
            task_lane_order: default_task_lane_order(),
            collapsed_task_lanes: Vec::new(),
            plan_order: BTreeMap::new(),
            task_order: BTreeMap::new(),
            filters: KanbanFilters {
                show_completed: true,
                show_cancelled: true,
                ..KanbanFilters::default()
            },
            updated_at: now_secs(),
        }
    }
}

fn normalize_layout(layout: &mut KanbanLayout, workspace_cwd: &str) {
    layout.version = KANBAN_STORE_VERSION;
    layout.workspace_root = Some(workspace_cwd.to_owned());
    if layout.plan_section_order.is_empty() {
        layout.plan_section_order = default_plan_section_order();
    }
    if layout.task_lane_order.is_empty() {
        layout.task_lane_order = default_task_lane_order();
    }
    layout.updated_at = now_secs();
}

fn kanban_root(workspace_cwd: &str) -> Result<PathBuf, String> {
    let roots = ensure_agents_layout(workspace_cwd)?;
    Ok(roots.kanban)
}

fn index_path(root: &Path) -> PathBuf {
    root.join(KANBAN_INDEX)
}

fn load_layout(root: &Path, workspace_cwd: &str) -> Result<KanbanLayout, String> {
    let path = index_path(root);
    let raw = match fs::read_to_string(&path) {
        Ok(s) if s.trim().is_empty() => return Ok(default_layout_for(workspace_cwd)),
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(default_layout_for(workspace_cwd));
        }
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let mut layout: KanbanLayout =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    if layout.version != KANBAN_STORE_VERSION {
        return Err(format!(
            "unsupported kanban layout version: {}",
            layout.version
        ));
    }
    normalize_layout(&mut layout, workspace_cwd);
    Ok(layout)
}

fn default_layout_for(workspace_cwd: &str) -> KanbanLayout {
    let mut layout = KanbanLayout::default();
    layout.workspace_root = Some(workspace_cwd.to_owned());
    layout
}

fn write_layout(
    root: &Path,
    mut layout: KanbanLayout,
    workspace_cwd: &str,
) -> Result<KanbanLayout, String> {
    normalize_layout(&mut layout, workspace_cwd);
    fs::create_dir_all(root).map_err(|e| format!("mkdir {}: {e}", root.display()))?;
    let path = index_path(root);
    let tmp = path.with_extension("json.tmp");
    let body =
        serde_json::to_string_pretty(&layout).map_err(|e| format!("serialize kanban: {e}"))?;
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))?;
    Ok(layout)
}

fn derived_plan_state(meta: &PlanMeta) -> KanbanPlanState {
    let s = &meta.task_summary;
    if s.blocked > 0 {
        KanbanPlanState::Blocked
    } else if s.in_progress > 0 {
        KanbanPlanState::InProgress
    } else if s.pending > 0 {
        KanbanPlanState::Pending
    } else if s.completed > 0 {
        KanbanPlanState::Completed
    } else if s.cancelled > 0 {
        KanbanPlanState::Cancelled
    } else {
        KanbanPlanState::Empty
    }
}

fn runtime_task_map(workspace_cwd: &str) -> HashMap<(String, String), String> {
    tasks::tasks_snapshot(workspace_cwd)
        .map(|snapshot| {
            snapshot
                .tasks
                .into_iter()
                .filter_map(|task| Some(((task.plan_path?, task.plan_task_id?), task.id)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn kanban_board_load_inner(workspace_cwd: &str) -> Result<KanbanBoard, String> {
    let root = kanban_root(workspace_cwd)?;
    let layout = load_layout(&root, workspace_cwd)?;
    let runtime = runtime_task_map(workspace_cwd);
    let mut plans: Vec<KanbanPlanNode> = plans::plan_list_inner(workspace_cwd)?
        .into_iter()
        .filter(|meta| !meta.is_index && !meta.path.eq_ignore_ascii_case(PLANS_INDEX))
        .map(|meta| {
            let content = plans::plan_read_inner(workspace_cwd, &meta.path)
                .map(|body| body.content)
                .unwrap_or_default();
            let tasks = plans::parse_plan_tasks(&content)
                .into_iter()
                .map(|task| KanbanTaskCard {
                    runtime_task_id: runtime.get(&(meta.path.clone(), task.id.clone())).cloned(),
                    plan_path: meta.path.clone(),
                    id: task.id,
                    title: task.title,
                    status: task.status,
                })
                .collect();
            KanbanPlanNode {
                state: derived_plan_state(&meta),
                meta,
                tasks,
            }
        })
        .collect();
    plans.sort_by(|a, b| {
        layout
            .plan_order
            .get(&a.meta.path)
            .copied()
            .unwrap_or(u32::MAX)
            .cmp(
                &layout
                    .plan_order
                    .get(&b.meta.path)
                    .copied()
                    .unwrap_or(u32::MAX),
            )
            .then_with(|| a.meta.path.to_lowercase().cmp(&b.meta.path.to_lowercase()))
    });
    Ok(KanbanBoard { layout, plans })
}

pub fn kanban_layout_save_inner(
    workspace_cwd: &str,
    layout: KanbanLayout,
) -> Result<KanbanLayout, String> {
    let root = kanban_root(workspace_cwd)?;
    write_layout(&root, layout, workspace_cwd)
}

fn read_plan_tasks(
    workspace_cwd: &str,
    plan_path: &str,
) -> Result<(String, Vec<PlanTask>), String> {
    let content = plans::plan_read_inner(workspace_cwd, plan_path)?;
    if content.is_index {
        return Err("PLANS.md is the protected index and cannot be used as a kanban plan".into());
    }
    let tasks = plans::parse_plan_tasks(&content.content);
    Ok((content.content, tasks))
}

fn write_plan_tasks(
    workspace_cwd: &str,
    plan_path: &str,
    body: &str,
    tasks: &[PlanTask],
) -> Result<(), String> {
    let next = plans::rewrite_plan_tasks(body, tasks);
    plans::plan_write_inner(workspace_cwd, plan_path, &next)?;
    Ok(())
}

fn next_plan_task_id(tasks: &[PlanTask], title: &str) -> String {
    let slug = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(5)
        .collect::<Vec<_>>()
        .join("-");
    let base = if slug.is_empty() {
        "task".to_string()
    } else {
        slug
    };
    if !tasks.iter().any(|task| task.id == base) {
        return base;
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !tasks.iter().any(|task| task.id == candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", now_secs())
}

pub fn kanban_task_create_inner(
    workspace_cwd: &str,
    input: KanbanTaskCreateInput,
) -> Result<KanbanTaskCard, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("task title is empty".into());
    }
    let (body, mut tasks) = read_plan_tasks(workspace_cwd, &input.plan_path)?;
    let id = next_plan_task_id(&tasks, title);
    let task = PlanTask {
        id: id.clone(),
        title: title.to_owned(),
        status: input.status.unwrap_or(TaskStatus::Pending),
    };
    tasks.push(task.clone());
    write_plan_tasks(workspace_cwd, &input.plan_path, &body, &tasks)?;
    Ok(KanbanTaskCard {
        plan_path: input.plan_path,
        id: task.id,
        title: task.title,
        status: task.status,
        runtime_task_id: None,
    })
}

pub fn kanban_task_update_inner(
    workspace_cwd: &str,
    plan_path: &str,
    task_id: &str,
    patch: KanbanTaskUpdatePatch,
) -> Result<KanbanTaskCard, String> {
    let (body, mut tasks) = read_plan_tasks(workspace_cwd, plan_path)?;
    let Some(task) = tasks.iter_mut().find(|task| task.id == task_id) else {
        return Err(format!("task not found: {task_id}"));
    };
    if let Some(title) = patch
        .title
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        task.title = title.to_owned();
    }
    if let Some(status) = patch.status.clone() {
        task.status = status;
    }
    let updated = task.clone();
    write_plan_tasks(workspace_cwd, plan_path, &body, &tasks)?;
    if let Some(runtime_id) = runtime_task_map(workspace_cwd)
        .get(&(plan_path.to_owned(), task_id.to_owned()))
        .cloned()
    {
        let _ = tasks::tasks_update_inner(
            workspace_cwd,
            &runtime_id,
            TaskUpdatePatch {
                title: patch.title,
                description: None,
                status: patch.status,
                parent_id: None,
                notes: None,
            },
        );
    }
    Ok(KanbanTaskCard {
        plan_path: plan_path.to_owned(),
        id: updated.id,
        title: updated.title,
        status: updated.status,
        runtime_task_id: runtime_task_map(workspace_cwd)
            .get(&(plan_path.to_owned(), task_id.to_owned()))
            .cloned(),
    })
}

pub fn kanban_task_delete_inner(
    workspace_cwd: &str,
    plan_path: &str,
    task_id: &str,
) -> Result<(), String> {
    let (body, mut tasks) = read_plan_tasks(workspace_cwd, plan_path)?;
    let before = tasks.len();
    tasks.retain(|task| task.id != task_id);
    if tasks.len() == before {
        return Err(format!("task not found: {task_id}"));
    }
    write_plan_tasks(workspace_cwd, plan_path, &body, &tasks)
}

#[tauri::command]
pub async fn kanban_board_load(workspace_cwd: String) -> Result<KanbanBoard, String> {
    proc::run_blocking(move || kanban_board_load_inner(&workspace_cwd)).await
}

#[tauri::command]
pub async fn kanban_layout_save(
    workspace_cwd: String,
    layout: KanbanLayout,
) -> Result<KanbanLayout, String> {
    proc::run_blocking(move || kanban_layout_save_inner(&workspace_cwd, layout)).await
}

#[tauri::command]
pub async fn kanban_task_create(
    workspace_cwd: String,
    input: KanbanTaskCreateInput,
) -> Result<KanbanTaskCard, String> {
    proc::run_blocking(move || kanban_task_create_inner(&workspace_cwd, input)).await
}

#[tauri::command]
pub async fn kanban_task_update(
    workspace_cwd: String,
    plan_path: String,
    task_id: String,
    patch: KanbanTaskUpdatePatch,
) -> Result<KanbanTaskCard, String> {
    proc::run_blocking(move || {
        kanban_task_update_inner(&workspace_cwd, &plan_path, &task_id, patch)
    })
    .await
}

#[tauri::command]
pub async fn kanban_task_delete(
    workspace_cwd: String,
    plan_path: String,
    task_id: String,
) -> Result<(), String> {
    proc::run_blocking(move || kanban_task_delete_inner(&workspace_cwd, &plan_path, &task_id)).await
}

#[tauri::command]
pub async fn kanban_export_layout(workspace_cwd: String) -> Result<String, String> {
    proc::run_blocking(move || {
        let root = kanban_root(&workspace_cwd)?;
        let layout = load_layout(&root, &workspace_cwd)?;
        serde_json::to_string_pretty(&layout).map_err(|e| format!("serialize kanban export: {e}"))
    })
    .await
}

#[tauri::command]
pub async fn kanban_import_layout(
    workspace_cwd: String,
    json: String,
) -> Result<KanbanLayout, String> {
    proc::run_blocking(move || {
        let layout: KanbanLayout =
            serde_json::from_str(&json).map_err(|e| format!("parse kanban import: {e}"))?;
        kanban_layout_save_inner(&workspace_cwd, layout)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_ws(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "blxcode_kanban_test_{label}_{}_{}",
            std::process::id(),
            now_secs()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn layout_defaults_and_persists() {
        let ws = temp_ws("layout");
        let cwd = ws.to_string_lossy().to_string();
        let root = kanban_root(&cwd).unwrap();
        let mut layout = load_layout(&root, &cwd).unwrap();
        layout.expanded_plans.push("demo.md".into());
        write_layout(&root, layout, &cwd).unwrap();
        let reloaded = load_layout(&root, &cwd).unwrap();
        assert_eq!(reloaded.expanded_plans, vec!["demo.md"]);
    }

    #[test]
    fn task_create_update_delete_round_trips_markdown() {
        let ws = temp_ws("tasks");
        let cwd = ws.to_string_lossy().to_string();
        plans::plan_create_inner(&cwd, "demo.md", Some("# Demo\n\n## Tasks\n\n")).unwrap();
        let card = kanban_task_create_inner(
            &cwd,
            KanbanTaskCreateInput {
                plan_path: "demo.md".into(),
                title: "Wire board".into(),
                status: Some(TaskStatus::Pending),
            },
        )
        .unwrap();
        assert_eq!(card.id, "wire-board");
        kanban_task_update_inner(
            &cwd,
            "demo.md",
            &card.id,
            KanbanTaskUpdatePatch {
                title: None,
                status: Some(TaskStatus::InProgress),
            },
        )
        .unwrap();
        let body = plans::plan_read_inner(&cwd, "demo.md").unwrap().content;
        assert!(body.contains("- [>] `wire-board` - Wire board"));
        kanban_task_delete_inner(&cwd, "demo.md", &card.id).unwrap();
        let body = plans::plan_read_inner(&cwd, "demo.md").unwrap().content;
        assert!(!body.contains("wire-board"));
    }
}
