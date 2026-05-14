//! Workspace-scoped task tracking for the BLXCode agent.
//!
//! Layout per workspace:
//!
//! ```text
//! <workspace_cwd>/.blxcode/tasks/
//!   index.json
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TASKS_REL: &str = ".blxcode/tasks";
const TASKS_INDEX: &str = "index.json";
const TASK_STORE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Blocked,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub position: u32,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub parent_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub tasks: Vec<TaskRecord>,
    pub active_task_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreateInput {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<TaskStatus>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdatePatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<TaskStatus>,
    #[serde(default)]
    pub parent_id: Option<Option<String>>,
    #[serde(default)]
    pub notes: Option<Option<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskReorderInput {
    pub ordered_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskStore {
    version: u32,
    tasks: Vec<TaskRecord>,
}

impl Default for TaskStore {
    fn default() -> Self {
        Self {
            version: TASK_STORE_VERSION,
            tasks: Vec::new(),
        }
    }
}

fn err<T>(s: impl Into<String>) -> Result<T, String> {
    Err(s.into())
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn validate_workspace_cwd(ws: &str) -> Result<PathBuf, String> {
    let trimmed = ws.trim();
    if trimmed.is_empty() {
        return err("workspace cwd is empty");
    }
    let p = PathBuf::from(trimmed);
    if !p.is_absolute() {
        return err("workspace cwd must be absolute");
    }
    if !p.exists() {
        return err(format!("workspace cwd does not exist: {trimmed}"));
    }
    Ok(p)
}

fn ensure_tasks_root(ws: &str) -> Result<PathBuf, String> {
    let ws_path = validate_workspace_cwd(ws)?;
    let root = ws_path.join(TASKS_REL);
    fs::create_dir_all(&root).map_err(|e| format!("create tasks root: {e}"))?;
    Ok(root)
}

fn index_path(root: &Path) -> PathBuf {
    root.join(TASKS_INDEX)
}

fn load_store(root: &Path) -> Result<TaskStore, String> {
    let path = index_path(root);
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(TaskStore::default()),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    if raw.trim().is_empty() {
        return Ok(TaskStore::default());
    }
    let mut store: TaskStore =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    if store.version != TASK_STORE_VERSION {
        return err(format!("unsupported task store version: {}", store.version));
    }
    normalize_store(&mut store);
    Ok(store)
}

fn normalize_store(store: &mut TaskStore) {
    store.tasks.sort_by(|a, b| {
        a.position
            .cmp(&b.position)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    for (idx, task) in store.tasks.iter_mut().enumerate() {
        task.position = idx as u32;
    }
}

fn write_store(root: &Path, store: &TaskStore) -> Result<(), String> {
    let path = index_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let body =
        serde_json::to_string_pretty(store).map_err(|e| format!("serialize task store: {e}"))?;
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))?;
    Ok(())
}

fn next_task_id(store: &TaskStore) -> String {
    format!("task-{}-{}", now_secs(), store.tasks.len() + 1)
}

fn validate_title(title: &str) -> Result<String, String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return err("task title is empty");
    }
    Ok(trimmed.to_owned())
}

fn validate_parent_id(
    store: &TaskStore,
    parent_id: Option<String>,
    current_id: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(parent_id) = parent_id else {
        return Ok(None);
    };
    let parent_id = parent_id.trim().to_owned();
    if parent_id.is_empty() {
        return Ok(None);
    }
    if current_id.is_some_and(|id| id == parent_id) {
        return err("task cannot parent itself");
    }
    if store.tasks.iter().any(|task| task.id == parent_id) {
        Ok(Some(parent_id))
    } else {
        err(format!("parent task not found: {parent_id}"))
    }
}

fn apply_status(task: &mut TaskRecord, status: TaskStatus, now: i64) {
    task.status = status;
    if matches!(task.status, TaskStatus::Completed) {
        task.completed_at = Some(now);
    } else {
        task.completed_at = None;
    }
}

fn demote_other_in_progress(tasks: &mut [TaskRecord], active_id: &str, now: i64) {
    for task in tasks {
        if task.id != active_id && matches!(task.status, TaskStatus::InProgress) {
            task.status = TaskStatus::Pending;
            task.completed_at = None;
            task.updated_at = now;
        }
    }
}

fn snapshot_from_store(store: &TaskStore) -> TaskSnapshot {
    TaskSnapshot {
        tasks: store.tasks.clone(),
        active_task_id: store
            .tasks
            .iter()
            .find(|task| matches!(task.status, TaskStatus::InProgress))
            .map(|task| task.id.clone()),
    }
}

pub fn tasks_snapshot(workspace_cwd: &str) -> Result<TaskSnapshot, String> {
    let root = ensure_tasks_root(workspace_cwd)?;
    let store = load_store(&root)?;
    Ok(snapshot_from_store(&store))
}

pub fn tasks_get_inner(workspace_cwd: &str, id: &str) -> Result<TaskRecord, String> {
    let root = ensure_tasks_root(workspace_cwd)?;
    let store = load_store(&root)?;
    store
        .tasks
        .into_iter()
        .find(|task| task.id == id)
        .ok_or_else(|| format!("task not found: {id}"))
}

pub fn tasks_create_inner(
    workspace_cwd: &str,
    input: TaskCreateInput,
) -> Result<TaskRecord, String> {
    let root = ensure_tasks_root(workspace_cwd)?;
    let mut store = load_store(&root)?;
    let now = now_secs();
    let id = next_task_id(&store);
    let status = input.status.unwrap_or(TaskStatus::Pending);
    let parent_id = validate_parent_id(&store, input.parent_id, None)?;

    let task = TaskRecord {
        id: id.clone(),
        title: validate_title(&input.title)?,
        description: input.description.unwrap_or_default(),
        status: TaskStatus::Pending,
        position: store.tasks.len() as u32,
        created_at: now,
        updated_at: now,
        completed_at: None,
        parent_id,
        notes: input.notes.filter(|s| !s.trim().is_empty()),
    };
    let mut task = task;
    apply_status(&mut task, status, now);
    store.tasks.push(task.clone());
    if matches!(task.status, TaskStatus::InProgress) {
        demote_other_in_progress(&mut store.tasks, &id, now);
    }
    normalize_store(&mut store);
    write_store(&root, &store)?;
    store
        .tasks
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| "created task missing from store".to_owned())
}

pub fn tasks_update_inner(
    workspace_cwd: &str,
    id: &str,
    patch: TaskUpdatePatch,
) -> Result<TaskRecord, String> {
    let root = ensure_tasks_root(workspace_cwd)?;
    let mut store = load_store(&root)?;
    let task_idx = store
        .tasks
        .iter()
        .position(|task| task.id == id)
        .ok_or_else(|| format!("task not found: {id}"))?;
    let parent_id_supplied = patch.parent_id.is_some();
    let new_parent_id = match patch.parent_id {
        Some(parent_id) => validate_parent_id(&store, parent_id, Some(id))?,
        None => store.tasks[task_idx].parent_id.clone(),
    };

    let now = now_secs();
    {
        let task = &mut store.tasks[task_idx];
        if let Some(title) = patch.title {
            task.title = validate_title(&title)?;
        }
        if let Some(description) = patch.description {
            task.description = description;
        }
        if let Some(status) = patch.status {
            apply_status(task, status, now);
        }
        if parent_id_supplied {
            task.parent_id = new_parent_id;
        }
        if let Some(notes) = patch.notes {
            task.notes = notes.filter(|s| !s.trim().is_empty());
        }
        task.updated_at = now;
    }
    if matches!(store.tasks[task_idx].status, TaskStatus::InProgress) {
        demote_other_in_progress(&mut store.tasks, id, now);
    }
    normalize_store(&mut store);
    write_store(&root, &store)?;
    store
        .tasks
        .into_iter()
        .find(|task| task.id == id)
        .ok_or_else(|| format!("task not found after update: {id}"))
}

pub fn tasks_delete_inner(workspace_cwd: &str, id: &str) -> Result<TaskSnapshot, String> {
    let root = ensure_tasks_root(workspace_cwd)?;
    let mut store = load_store(&root)?;
    let before = store.tasks.len();
    store.tasks.retain(|task| task.id != id);
    if store.tasks.len() == before {
        return err(format!("task not found: {id}"));
    }
    for task in &mut store.tasks {
        if task.parent_id.as_deref() == Some(id) {
            task.parent_id = None;
            task.updated_at = now_secs();
        }
    }
    normalize_store(&mut store);
    write_store(&root, &store)?;
    Ok(snapshot_from_store(&store))
}

pub fn tasks_reorder_inner(
    workspace_cwd: &str,
    input: TaskReorderInput,
) -> Result<TaskSnapshot, String> {
    let root = ensure_tasks_root(workspace_cwd)?;
    let mut store = load_store(&root)?;
    if input.ordered_ids.len() != store.tasks.len() {
        return err("orderedIds must contain every known task exactly once");
    }
    let known: HashSet<&str> = store.tasks.iter().map(|task| task.id.as_str()).collect();
    let mut seen = HashSet::new();
    for id in &input.ordered_ids {
        if !known.contains(id.as_str()) {
            return err(format!("unknown task id in orderedIds: {id}"));
        }
        if !seen.insert(id.as_str()) {
            return err(format!("duplicate task id in orderedIds: {id}"));
        }
    }

    let mut reordered = Vec::with_capacity(store.tasks.len());
    for (idx, id) in input.ordered_ids.iter().enumerate() {
        let mut task = store
            .tasks
            .iter()
            .find(|task| &task.id == id)
            .cloned()
            .ok_or_else(|| format!("task not found: {id}"))?;
        task.position = idx as u32;
        reordered.push(task);
    }
    store.tasks = reordered;
    normalize_store(&mut store);
    write_store(&root, &store)?;
    Ok(snapshot_from_store(&store))
}

#[tauri::command]
pub fn tasks_list(workspace_cwd: String) -> Result<TaskSnapshot, String> {
    tasks_snapshot(&workspace_cwd)
}

#[tauri::command]
pub fn tasks_get(workspace_cwd: String, id: String) -> Result<TaskRecord, String> {
    tasks_get_inner(&workspace_cwd, &id)
}

#[tauri::command]
pub fn tasks_create(workspace_cwd: String, input: TaskCreateInput) -> Result<TaskRecord, String> {
    tasks_create_inner(&workspace_cwd, input)
}

#[tauri::command]
pub fn tasks_update(
    workspace_cwd: String,
    id: String,
    patch: TaskUpdatePatch,
) -> Result<TaskRecord, String> {
    tasks_update_inner(&workspace_cwd, &id, patch)
}

#[tauri::command]
pub fn tasks_delete(workspace_cwd: String, id: String) -> Result<TaskSnapshot, String> {
    tasks_delete_inner(&workspace_cwd, &id)
}

#[tauri::command]
pub fn tasks_reorder(
    workspace_cwd: String,
    input: TaskReorderInput,
) -> Result<TaskSnapshot, String> {
    tasks_reorder_inner(&workspace_cwd, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

    fn temp_workspace() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "blxcode-task-tests-{}-{}-{}",
            std::process::id(),
            nanos,
            seq
        ));
        fs::create_dir_all(&base).expect("create temp workspace");
        base
    }

    fn ws_string(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn create_initializes_store_and_lists_task() {
        let ws = temp_workspace();
        let workspace_cwd = ws_string(&ws);

        let created = tasks_create_inner(
            &workspace_cwd,
            TaskCreateInput {
                title: "Ship task system".into(),
                description: Some("Implement persistence".into()),
                status: Some(TaskStatus::Pending),
                parent_id: None,
                notes: None,
            },
        )
        .expect("create task");

        let snapshot = tasks_snapshot(&workspace_cwd).expect("snapshot");
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.tasks[0].id, created.id);
        assert!(ws.join(TASKS_REL).join(TASKS_INDEX).is_file());
    }

    #[test]
    fn update_only_changes_patched_fields_and_manages_status() {
        let ws = temp_workspace();
        let workspace_cwd = ws_string(&ws);

        let first = tasks_create_inner(
            &workspace_cwd,
            TaskCreateInput {
                title: "First".into(),
                description: None,
                status: Some(TaskStatus::InProgress),
                parent_id: None,
                notes: None,
            },
        )
        .expect("create first");
        let second = tasks_create_inner(
            &workspace_cwd,
            TaskCreateInput {
                title: "Second".into(),
                description: Some("keep me".into()),
                status: Some(TaskStatus::Pending),
                parent_id: None,
                notes: Some("hello".into()),
            },
        )
        .expect("create second");

        let updated = tasks_update_inner(
            &workspace_cwd,
            &second.id,
            TaskUpdatePatch {
                title: None,
                description: Some("changed".into()),
                status: Some(TaskStatus::InProgress),
                parent_id: None,
                notes: Some(Some("world".into())),
            },
        )
        .expect("update second");

        assert_eq!(updated.title, "Second");
        assert_eq!(updated.description, "changed");
        assert!(matches!(updated.status, TaskStatus::InProgress));

        let first_after = tasks_get_inner(&workspace_cwd, &first.id).expect("load first");
        assert!(matches!(first_after.status, TaskStatus::Pending));
    }

    #[test]
    fn completed_status_sets_completed_at() {
        let ws = temp_workspace();
        let workspace_cwd = ws_string(&ws);
        let task = tasks_create_inner(
            &workspace_cwd,
            TaskCreateInput {
                title: "Done".into(),
                description: None,
                status: None,
                parent_id: None,
                notes: None,
            },
        )
        .expect("create");

        let updated = tasks_update_inner(
            &workspace_cwd,
            &task.id,
            TaskUpdatePatch {
                title: None,
                description: None,
                status: Some(TaskStatus::Completed),
                parent_id: None,
                notes: None,
            },
        )
        .expect("complete task");
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn reorder_requires_exact_set_and_rewrites_positions() {
        let ws = temp_workspace();
        let workspace_cwd = ws_string(&ws);
        let first = tasks_create_inner(
            &workspace_cwd,
            TaskCreateInput {
                title: "One".into(),
                description: None,
                status: None,
                parent_id: None,
                notes: None,
            },
        )
        .expect("create first");
        let second = tasks_create_inner(
            &workspace_cwd,
            TaskCreateInput {
                title: "Two".into(),
                description: None,
                status: None,
                parent_id: None,
                notes: None,
            },
        )
        .expect("create second");

        let snapshot = tasks_reorder_inner(
            &workspace_cwd,
            TaskReorderInput {
                ordered_ids: vec![second.id.clone(), first.id.clone()],
            },
        )
        .expect("reorder");

        assert_eq!(snapshot.tasks[0].id, second.id);
        assert_eq!(snapshot.tasks[0].position, 0);
        assert_eq!(snapshot.tasks[1].id, first.id);
        assert_eq!(snapshot.tasks[1].position, 1);
    }

    #[test]
    fn relative_workspace_is_rejected() {
        let err = tasks_snapshot("relative/path").expect_err("must fail");
        assert!(err.contains("absolute"));
    }

    #[test]
    fn empty_or_missing_store_reads_as_empty() {
        let ws = temp_workspace();
        let workspace_cwd = ws_string(&ws);
        let snapshot = tasks_snapshot(&workspace_cwd).expect("snapshot");
        assert!(snapshot.tasks.is_empty());

        let root = ensure_tasks_root(&workspace_cwd).expect("tasks root");
        fs::write(index_path(&root), "").expect("write empty index");
        let snapshot = tasks_snapshot(&workspace_cwd).expect("snapshot after empty file");
        assert!(snapshot.tasks.is_empty());
    }
}
