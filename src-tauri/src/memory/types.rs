use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MemoryScope {
    Workspace,
    Global,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    pub scope: MemoryScope,
    pub path: String,
    pub name: String,
    pub title: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub size: u64,
    pub modified: i64,
    pub is_template: bool,
    pub is_learning: bool,
    pub is_overview: bool,
    pub category: String,
    pub managed: Option<String>,
    pub stale: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoteContent {
    pub scope: MemoryScope,
    pub path: String,
    pub content: String,
    pub modified: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub scope: MemoryScope,
    pub path: String,
    pub label: String,
    pub tags: Vec<String>,
    pub orphan: bool,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_category_hub: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hub_scopes: Option<Vec<MemoryScope>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub cross_scope: bool,
    /// Display label from `[[target|alias]]` wikilink syntax; `None` when no alias was written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
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
    pub scope: MemoryScope,
    pub path: String,
    pub line: u32,
    pub snippet: String,
    pub category: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BacklinkRef {
    pub scope: MemoryScope,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MemoryFolderStatus {
    pub memory: bool,
    pub learnings: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatusResponse {
    pub workspace: MemoryFolderStatus,
    pub global: MemoryFolderStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MemorySubcategories {
    pub workspace: Vec<String>,
    pub global: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MemoryListResponse {
    pub notes: Vec<NoteMeta>,
    pub memory_subcategories: MemorySubcategories,
}

/// Re-exported from the generic `crate::pointers` module so the wire
/// format stays identical to what the frontend has historically seen.
pub use crate::pointers::PointerResult;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RenameReport {
    pub old_path: String,
    pub new_path: String,
    pub link_rewrites: u32,
    pub files_changed: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RebuildReport {
    pub git_rev: Option<String>,
    pub crate_count: u32,
    pub module_count: u32,
    pub files_changed: u32,
    pub generated_paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureLintReport {
    pub git_rev: Option<String>,
    pub state_git_rev: Option<String>,
    pub stale: bool,
    pub stale_paths: Vec<String>,
}
