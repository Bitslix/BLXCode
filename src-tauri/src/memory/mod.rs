//! Obsidian-style memory system with workspace + global scope.
//!
//! Layout:
//! ```text
//! <workspace>/.agents/memory/      workspace notes
//! <workspace>/.agents/learnings/   workspace learnings (API: `learnings/…`)
//! ~/.blxcode/memory/               global notes
//! ~/.blxcode/learnings/            global learnings
//! ```
//!
//! Node IDs: `{scope}:{api_path}` (e.g. `workspace:decisions/note.md`)

mod frontmatter;
mod graph;
pub mod paths;
mod store;
mod types;
pub mod wikilinks;

pub use types::*;

use crate::agents_layout::ensure_agents_layout;
use store::{
    memory_backlinks_impl, memory_bootstrap_impl, memory_create_category_impl, memory_create_impl,
    memory_delete_impl, memory_export_impl, memory_graph_impl, memory_import_impl,
    memory_install_pointers_impl, memory_list_impl, memory_pointer_status_impl, memory_read_impl,
    memory_rename_impl, memory_search_impl, memory_status_impl, memory_uninstall_pointers_impl,
    memory_write_impl,
};

// ── Bootstrap / status ────────────────────────────────────────────────────────

#[tauri::command]
pub fn workspace_ensure_agents(workspace_cwd: String) -> Result<(), String> {
    ensure_agents_layout(&workspace_cwd)?;
    Ok(())
}

#[tauri::command]
pub fn memory_status(workspace_cwd: String) -> Result<MemoryStatusResponse, String> {
    Ok(memory_status_impl(&workspace_cwd))
}

#[tauri::command]
pub fn memory_bootstrap(workspace_cwd: String, target: String) -> Result<(), String> {
    memory_bootstrap_impl(&target, &workspace_cwd)
}

// ── List ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn memory_list(workspace_cwd: String) -> Result<MemoryListResponse, String> {
    Ok(memory_list_impl(&workspace_cwd))
}

// ── CRUD ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn memory_read(
    workspace_cwd: String,
    scope: MemoryScope,
    path: String,
) -> Result<NoteContent, String> {
    memory_read_impl(&scope, &workspace_cwd, &path)
}

#[tauri::command]
pub fn memory_write(
    workspace_cwd: String,
    scope: MemoryScope,
    path: String,
    content: String,
) -> Result<NoteContent, String> {
    memory_write_impl(&scope, &workspace_cwd, &path, &content)
}

#[tauri::command]
pub fn memory_create(
    workspace_cwd: String,
    scope: MemoryScope,
    path: String,
    content: Option<String>,
) -> Result<NoteMeta, String> {
    memory_create_impl(&scope, &workspace_cwd, &path, content)
}

#[tauri::command]
pub fn memory_delete(
    workspace_cwd: String,
    scope: MemoryScope,
    path: String,
) -> Result<(), String> {
    memory_delete_impl(&scope, &workspace_cwd, &path)
}

#[tauri::command]
pub fn memory_rename(
    workspace_cwd: String,
    scope: MemoryScope,
    old_path: String,
    new_path: String,
    rewrite_links: bool,
) -> Result<RenameReport, String> {
    memory_rename_impl(&scope, &workspace_cwd, &old_path, &new_path, rewrite_links)
}

#[tauri::command]
pub fn memory_create_category(
    workspace_cwd: String,
    scope: MemoryScope,
    name: String,
) -> Result<String, String> {
    memory_create_category_impl(&scope, &workspace_cwd, &name)
}

// ── Graph, backlinks, search ──────────────────────────────────────────────────

#[tauri::command]
pub fn memory_graph(workspace_cwd: String) -> Result<GraphData, String> {
    memory_graph_impl(&workspace_cwd)
}

#[tauri::command]
pub fn memory_backlinks(
    workspace_cwd: String,
    scope: MemoryScope,
    path: String,
) -> Result<Vec<BacklinkRef>, String> {
    memory_backlinks_impl(&scope, &workspace_cwd, &path)
}

#[tauri::command]
pub fn memory_search(workspace_cwd: String, query: String) -> Result<Vec<SearchHit>, String> {
    memory_search_impl(&workspace_cwd, &query)
}

// ── Export / Import ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn memory_export(workspace_cwd: String, dest_dir: String) -> Result<u32, String> {
    memory_export_impl(&workspace_cwd, &dest_dir)
}

#[tauri::command]
pub fn memory_import(workspace_cwd: String, src_dir: String) -> Result<u32, String> {
    memory_import_impl(&workspace_cwd, &src_dir)
}

// ── Pointer files ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn memory_install_pointers(
    workspace_cwd: String,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    memory_install_pointers_impl(&workspace_cwd, agents)
}

#[tauri::command]
pub fn memory_uninstall_pointers(
    workspace_cwd: String,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    memory_uninstall_pointers_impl(&workspace_cwd, agents)
}

#[tauri::command]
pub fn memory_pointer_status(workspace_cwd: String) -> Result<Vec<PointerResult>, String> {
    memory_pointer_status_impl(&workspace_cwd)
}

// ── Legacy stub (kept for binary compatibility) ───────────────────────────────

#[tauri::command]
pub fn memory_root(workspace_cwd: String) -> Result<String, String> {
    let roots = paths::get_workspace_roots(&workspace_cwd)?;
    Ok(roots.memory.to_string_lossy().into_owned())
}
