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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanPlanMoveInput {
    pub plan_path: String,
    pub target_state: KanbanPlanState,
    #[serde(default)]
    pub ordered_plan_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanTaskMoveInput {
    pub plan_path: String,
    pub task_id: String,
    pub target_status: TaskStatus,
    #[serde(default)]
    pub before_task_id: Option<String>,
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
    KanbanLayout {
        workspace_root: Some(workspace_cwd.to_owned()),
        ..KanbanLayout::default()
    }
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

pub fn kanban_rewrite_plan_paths(
    workspace_cwd: &str,
    mapping: &[(String, String)],
) -> Result<(), String> {
    if mapping.is_empty() {
        return Ok(());
    }
    let root = kanban_root(workspace_cwd)?;
    if !index_path(&root).is_file() {
        return Ok(());
    }
    let replacements: HashMap<&str, &str> = mapping
        .iter()
        .map(|(old_path, new_path)| (old_path.as_str(), new_path.as_str()))
        .collect();
    let mut layout = load_layout(&root, workspace_cwd)?;
    let mut changed = false;

    for path in &mut layout.expanded_plans {
        if let Some(next) = replacements.get(path.as_str()) {
            *path = (*next).to_owned();
            changed = true;
        }
    }

    let mut next_plan_order = BTreeMap::new();
    for (path, order) in &layout.plan_order {
        let next_path = replacements
            .get(path.as_str())
            .copied()
            .unwrap_or(path.as_str())
            .to_owned();
        if next_path != *path {
            changed = true;
        }
        next_plan_order
            .entry(next_path)
            .and_modify(|existing: &mut u32| *existing = (*existing).min(*order))
            .or_insert(*order);
    }
    if changed {
        layout.plan_order = next_plan_order;
        let _ = write_layout(&root, layout, workspace_cwd)?;
    }
    Ok(())
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

fn sync_runtime_task(
    workspace_cwd: &str,
    plan_path: &str,
    task_id: &str,
    title: Option<String>,
    status: Option<TaskStatus>,
) {
    let Some(runtime_id) = runtime_task_map(workspace_cwd)
        .get(&(plan_path.to_owned(), task_id.to_owned()))
        .cloned()
    else {
        return;
    };
    let _ = tasks::tasks_update_inner(
        workspace_cwd,
        &runtime_id,
        TaskUpdatePatch {
            title,
            description: None,
            status,
            parent_id: None,
            notes: None,
        },
    );
}

fn first_task_index(tasks: &[PlanTask]) -> Result<usize, String> {
    if tasks.is_empty() {
        Err("empty plans cannot be moved to a non-empty kanban state".into())
    } else {
        Ok(0)
    }
}

fn apply_minimal_plan_state(
    tasks: &mut [PlanTask],
    target: &KanbanPlanState,
) -> Result<Vec<(String, TaskStatus)>, String> {
    let mut changed = Vec::new();
    let mut set_status = |tasks: &mut [PlanTask], idx: usize, status: TaskStatus| {
        if tasks[idx].status != status {
            tasks[idx].status = status.clone();
            changed.push((tasks[idx].id.clone(), status));
        }
    };

    match target {
        KanbanPlanState::Blocked => {
            if !tasks
                .iter()
                .any(|task| matches!(task.status, TaskStatus::Blocked))
            {
                let idx = first_task_index(tasks)?;
                set_status(tasks, idx, TaskStatus::Blocked);
            }
        }
        KanbanPlanState::InProgress => {
            for idx in 0..tasks.len() {
                if matches!(tasks[idx].status, TaskStatus::Blocked) {
                    set_status(tasks, idx, TaskStatus::Pending);
                }
            }
            if !tasks
                .iter()
                .any(|task| matches!(task.status, TaskStatus::InProgress))
            {
                let idx = tasks
                    .iter()
                    .position(|task| matches!(task.status, TaskStatus::Pending))
                    .unwrap_or(first_task_index(tasks)?);
                set_status(tasks, idx, TaskStatus::InProgress);
            }
        }
        KanbanPlanState::Pending => {
            for idx in 0..tasks.len() {
                if matches!(
                    tasks[idx].status,
                    TaskStatus::Blocked | TaskStatus::InProgress
                ) {
                    set_status(tasks, idx, TaskStatus::Pending);
                }
            }
            if !tasks
                .iter()
                .any(|task| matches!(task.status, TaskStatus::Pending))
            {
                let idx = first_task_index(tasks)?;
                set_status(tasks, idx, TaskStatus::Pending);
            }
        }
        KanbanPlanState::Completed => {
            for idx in 0..tasks.len() {
                if matches!(
                    tasks[idx].status,
                    TaskStatus::Blocked | TaskStatus::InProgress | TaskStatus::Pending
                ) {
                    set_status(tasks, idx, TaskStatus::Completed);
                }
            }
            if !tasks
                .iter()
                .any(|task| matches!(task.status, TaskStatus::Completed))
            {
                let idx = first_task_index(tasks)?;
                set_status(tasks, idx, TaskStatus::Completed);
            }
        }
        KanbanPlanState::Cancelled => {
            for idx in 0..tasks.len() {
                set_status(tasks, idx, TaskStatus::Cancelled);
            }
        }
        KanbanPlanState::Empty => {
            if !tasks.is_empty() {
                return Err("non-empty plans cannot be moved to the empty kanban state".into());
            }
        }
    }
    Ok(changed)
}

fn save_plan_order(
    workspace_cwd: &str,
    ordered_plan_paths: Vec<String>,
) -> Result<KanbanLayout, String> {
    let root = kanban_root(workspace_cwd)?;
    let mut layout = load_layout(&root, workspace_cwd)?;
    let mut next = BTreeMap::new();
    for (idx, path) in ordered_plan_paths.into_iter().enumerate() {
        next.entry(path).or_insert(idx as u32);
    }
    layout.plan_order = next;
    write_layout(&root, layout, workspace_cwd)
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
    sync_runtime_task(workspace_cwd, plan_path, task_id, patch.title, patch.status);
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

pub fn kanban_plan_move_inner(
    workspace_cwd: &str,
    input: KanbanPlanMoveInput,
) -> Result<KanbanBoard, String> {
    let (body, mut tasks) = read_plan_tasks(workspace_cwd, &input.plan_path)?;
    let changed = apply_minimal_plan_state(&mut tasks, &input.target_state)?;
    write_plan_tasks(workspace_cwd, &input.plan_path, &body, &tasks)?;
    for (task_id, status) in changed {
        sync_runtime_task(
            workspace_cwd,
            &input.plan_path,
            &task_id,
            None,
            Some(status),
        );
    }
    save_plan_order(workspace_cwd, input.ordered_plan_paths)?;
    kanban_board_load_inner(workspace_cwd)
}

pub fn kanban_task_move_inner(
    workspace_cwd: &str,
    input: KanbanTaskMoveInput,
) -> Result<KanbanTaskCard, String> {
    let (body, mut tasks) = read_plan_tasks(workspace_cwd, &input.plan_path)?;
    let Some(source_idx) = tasks.iter().position(|task| task.id == input.task_id) else {
        return Err(format!("task not found: {}", input.task_id));
    };
    let mut task = tasks.remove(source_idx);
    task.status = input.target_status.clone();
    let insert_idx = match input.before_task_id.as_deref() {
        Some(before_id) if before_id == input.task_id => source_idx.min(tasks.len()),
        Some(before_id) => tasks
            .iter()
            .position(|task| task.id == before_id)
            .ok_or_else(|| format!("target task not found: {before_id}"))?,
        None => tasks.len(),
    };
    tasks.insert(insert_idx, task.clone());
    write_plan_tasks(workspace_cwd, &input.plan_path, &body, &tasks)?;
    sync_runtime_task(
        workspace_cwd,
        &input.plan_path,
        &input.task_id,
        None,
        Some(input.target_status),
    );
    let runtime_task_id = runtime_task_map(workspace_cwd)
        .get(&(input.plan_path.clone(), input.task_id.clone()))
        .cloned();
    Ok(KanbanTaskCard {
        plan_path: input.plan_path,
        id: task.id,
        title: task.title,
        status: task.status,
        runtime_task_id,
    })
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
pub async fn kanban_plan_move(
    workspace_cwd: String,
    input: KanbanPlanMoveInput,
) -> Result<KanbanBoard, String> {
    proc::run_blocking(move || kanban_plan_move_inner(&workspace_cwd, input)).await
}

#[tauri::command]
pub async fn kanban_task_move(
    workspace_cwd: String,
    input: KanbanTaskMoveInput,
) -> Result<KanbanTaskCard, String> {
    proc::run_blocking(move || kanban_task_move_inner(&workspace_cwd, input)).await
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

    #[test]
    fn plan_move_minimally_changes_statuses_and_persists_order() {
        let ws = temp_ws("plan_move");
        let cwd = ws.to_string_lossy().to_string();
        plans::plan_create_inner(
            &cwd,
            "demo.md",
            Some("# Demo\n\n## Tasks\n\n- [!] `a` - Blocked\n- [x] `b` - Done\n"),
        )
        .unwrap();
        plans::plan_create_inner(&cwd, "other.md", Some("# Other\n\n## Tasks\n\n")).unwrap();

        let board = kanban_plan_move_inner(
            &cwd,
            KanbanPlanMoveInput {
                plan_path: "demo.md".into(),
                target_state: KanbanPlanState::InProgress,
                ordered_plan_paths: vec!["other.md".into(), "demo.md".into()],
            },
        )
        .unwrap();

        let body = plans::plan_read_inner(&cwd, "demo.md").unwrap().content;
        assert!(body.contains("- [>] `a` - Blocked"));
        assert!(body.contains("- [x] `b` - Done"));
        assert_eq!(
            board
                .layout
                .plan_order
                .get("other.md")
                .copied()
                .unwrap_or_default(),
            0
        );
        assert_eq!(
            board
                .layout
                .plan_order
                .get("demo.md")
                .copied()
                .unwrap_or_default(),
            1
        );
    }

    #[test]
    fn plan_move_rejects_non_empty_plan_to_empty_state() {
        let ws = temp_ws("plan_move_empty");
        let cwd = ws.to_string_lossy().to_string();
        plans::plan_create_inner(
            &cwd,
            "demo.md",
            Some("# Demo\n\n## Tasks\n\n- [ ] `a` - One\n"),
        )
        .unwrap();

        let err = kanban_plan_move_inner(
            &cwd,
            KanbanPlanMoveInput {
                plan_path: "demo.md".into(),
                target_state: KanbanPlanState::Empty,
                ordered_plan_paths: vec!["demo.md".into()],
            },
        )
        .unwrap_err();

        assert!(err.contains("non-empty plans cannot be moved"));
    }

    #[test]
    fn task_move_changes_status_and_markdown_order() {
        let ws = temp_ws("task_move");
        let cwd = ws.to_string_lossy().to_string();
        plans::plan_create_inner(
            &cwd,
            "demo.md",
            Some(
                "# Demo\n\n## Summary\n\nKeep me.\n\n## Tasks\n\n- [ ] `a` - One\n- [ ] `b` - Two\n- [x] `c` - Three\n\n## Notes\n\nStill here.\n",
            ),
        )
        .unwrap();

        kanban_task_move_inner(
            &cwd,
            KanbanTaskMoveInput {
                plan_path: "demo.md".into(),
                task_id: "c".into(),
                target_status: TaskStatus::Pending,
                before_task_id: Some("b".into()),
            },
        )
        .unwrap();

        let body = plans::plan_read_inner(&cwd, "demo.md").unwrap().content;
        let a = body.find("`a`").unwrap();
        let c = body.find("`c`").unwrap();
        let b = body.find("`b`").unwrap();
        assert!(a < c && c < b);
        assert!(body.contains("- [ ] `c` - Three"));
        assert!(body.contains("## Summary\n\nKeep me."));
        assert!(body.contains("## Notes\n\nStill here."));
    }
}
