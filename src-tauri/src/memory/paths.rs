use crate::agents_layout::{
    validate_workspace_cwd, LEARNINGS_API_PREFIX, LEARNINGS_REL, MEMORY_REL,
};
use std::fs;
use std::path::{Path, PathBuf};

use super::types::MemoryScope;

pub const TEMPLATES_DIRNAME: &str = "_templates";
pub const CATEGORY_PLACEHOLDER: &str = ".gitkeep";
pub const CATEGORY_HUB_ID_PREFIX: &str = "hub:";
pub const CATEGORY_HUB_PATH_PREFIX: &str = "@category/";

#[derive(Debug, Clone)]
pub struct MemoryRoots {
    pub memory: PathBuf,
    pub learnings: PathBuf,
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

pub fn get_global_roots() -> MemoryRoots {
    let base = home_dir().join(".blxcode");
    MemoryRoots {
        memory: base.join("memory"),
        learnings: base.join("learnings"),
    }
}

pub fn get_workspace_roots(ws: &str) -> Result<MemoryRoots, String> {
    let path = validate_workspace_cwd(ws)?;
    Ok(MemoryRoots {
        memory: path.join(MEMORY_REL),
        learnings: path.join(LEARNINGS_REL),
    })
}

pub fn get_roots_for_scope(
    scope: &MemoryScope,
    workspace_cwd: &str,
) -> Result<MemoryRoots, String> {
    match scope {
        MemoryScope::Global => Ok(get_global_roots()),
        MemoryScope::Workspace => get_workspace_roots(workspace_cwd),
    }
}

pub fn folder_exists(dir: &Path) -> bool {
    dir.is_dir()
}

pub fn node_id(scope: &MemoryScope, api_path: &str) -> String {
    let s = scope_str(scope);
    format!("{s}:{api_path}")
}

pub fn scope_str(scope: &MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Workspace => "workspace",
        MemoryScope::Global => "global",
    }
}

pub fn parse_node_id(id: &str) -> Option<(MemoryScope, String)> {
    let idx = id.find(':')?;
    if idx == 0 {
        return None;
    }
    let scope = match &id[..idx] {
        "workspace" => MemoryScope::Workspace,
        "global" => MemoryScope::Global,
        _ => return None,
    };
    Some((scope, id[idx + 1..].to_owned()))
}

pub fn graph_category_for(api_path: &str) -> String {
    if api_path.starts_with(LEARNINGS_API_PREFIX) {
        return "learnings".to_string();
    }
    if let Some((head, _)) = api_path.split_once('/') {
        if !head.is_empty() && !head.ends_with(".md") {
            return head.to_string();
        }
    }
    "memory".to_string()
}

pub fn list_memory_subcategories(memory_root: &Path) -> Vec<String> {
    if !memory_root.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let Ok(read) = fs::read_dir(memory_root) else {
        return out;
    };
    for entry in read.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name.starts_with('.') || name.eq_ignore_ascii_case(TEMPLATES_DIRNAME) {
            continue;
        }
        out.push(name);
    }
    out.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    out
}

pub fn validate_category_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("empty category name".into());
    }
    if trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.starts_with('.')
    {
        return Err("invalid category name".into());
    }
    if is_reserved_category(trimmed) {
        return Err(format!("reserved category name: {trimmed}"));
    }
    Ok(trimmed.to_owned())
}

fn is_reserved_category(name: &str) -> bool {
    name.eq_ignore_ascii_case(TEMPLATES_DIRNAME)
        || name.eq_ignore_ascii_case("memory")
        || name.eq_ignore_ascii_case("learnings")
}

pub fn category_hub_node_id(category: &str) -> String {
    format!("{CATEGORY_HUB_ID_PREFIX}{category}")
}

pub fn category_hub_path(category: &str) -> String {
    format!("{CATEGORY_HUB_PATH_PREFIX}{category}")
}
