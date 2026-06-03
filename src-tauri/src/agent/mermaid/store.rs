//! Persistence for plan-/task-linked Mermaid diagrams.
//!
//! Layout per plan:
//!
//! ```text
//! <workspace_cwd>/.agents/plans/<slug>/diagrams/
//!   diagrams.json          — manifest (metadata, no source)
//!   <id>.mmd               — one Mermaid source file per diagram
//! ```
//!
//! Diagrams travel in git with their plan, so deleting a plan folder removes
//! its diagrams too. Ad-hoc chat diagrams that are not tied to a plan are NOT
//! persisted here — they live in the conversation and are only materialised on
//! explicit export.

use crate::plans::plan_folder_abs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

const DIAGRAMS_DIRNAME: &str = "diagrams";
const MANIFEST_FILE: &str = "diagrams.json";
const MANIFEST_VERSION: u32 = 1;

/// Metadata for a single diagram, persisted in `diagrams.json`. The Mermaid
/// source itself lives in the sibling `<id>.mmd` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramMeta {
    /// Stable, unique, kebab-case id within the plan. Also the `.mmd` stem.
    pub id: String,
    /// Human title shown in the gallery / cards.
    pub title: String,
    /// Mermaid diagram kind hint (e.g. `flowchart`, `sequence`, `class`).
    /// Free-form; used only for labels/thumbnails.
    #[serde(default)]
    pub kind: String,
    /// Optional plan task id this diagram illustrates.
    #[serde(default)]
    pub task_id: Option<String>,
    /// Creation time, ms since UNIX epoch.
    pub created_ms: u64,
}

/// A diagram with its source code loaded. Returned to the frontend gallery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramRecord {
    #[serde(flatten)]
    pub meta: DiagramMeta,
    /// Mermaid source.
    pub code: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    diagrams: Vec<DiagramMeta>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn diagrams_dir(ws: &str, slug: &str) -> Result<PathBuf, String> {
    Ok(plan_folder_abs(ws, slug)?.join(DIAGRAMS_DIRNAME))
}

/// Validate an externally-supplied diagram id: kebab-case, no path separators.
fn validate_id(id: &str) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("empty diagram id".into());
    }
    if id == "." || id == ".." {
        return Err("disallowed diagram id".into());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("diagram ids may only contain [a-zA-Z0-9_-]".into());
    }
    Ok(())
}

fn read_manifest(dir: &PathBuf) -> Manifest {
    let path = dir.join(MANIFEST_FILE);
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Manifest::default(),
    }
}

fn write_manifest(dir: &PathBuf, manifest: &Manifest) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("create diagrams dir: {e}"))?;
    let path = dir.join(MANIFEST_FILE);
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("serialize manifest: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("write manifest: {e}"))
}

/// Derive a unique kebab-case id from a title, avoiding collisions with
/// `existing`.
fn slugify_unique(title: &str, existing: &[DiagramMeta]) -> String {
    let base: String = title
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() { "diagram".to_string() } else { base };
    let taken = |id: &str| existing.iter().any(|d| d.id == id);
    if !taken(&base) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Persist a new diagram under the given plan. Returns the stored record
/// (with a freshly-allocated id when `id` is `None`).
pub fn create_diagram(
    ws: &str,
    slug: &str,
    title: &str,
    code: &str,
    kind: &str,
    task_id: Option<String>,
    id: Option<String>,
) -> Result<DiagramRecord, String> {
    let dir = diagrams_dir(ws, slug)?;
    let mut manifest = read_manifest(&dir);

    let id = match id {
        Some(id) => {
            validate_id(&id)?;
            id
        }
        None => slugify_unique(title, &manifest.diagrams),
    };
    // Replace existing meta with the same id (idempotent re-create).
    manifest.diagrams.retain(|d| d.id != id);

    let meta = DiagramMeta {
        id: id.clone(),
        title: title.trim().to_string(),
        kind: kind.trim().to_string(),
        task_id: task_id.filter(|t| !t.trim().is_empty()),
        created_ms: now_ms(),
    };

    fs::create_dir_all(&dir).map_err(|e| format!("create diagrams dir: {e}"))?;
    fs::write(dir.join(format!("{id}.mmd")), code)
        .map_err(|e| format!("write diagram source: {e}"))?;
    manifest.diagrams.push(meta.clone());
    manifest.version = MANIFEST_VERSION;
    write_manifest(&dir, &manifest)?;

    Ok(DiagramRecord {
        meta,
        code: code.to_string(),
    })
}

/// List all diagrams for a plan, source code included. Missing `.mmd` files
/// are skipped (manifest self-heals on next write).
pub fn list_diagrams(ws: &str, slug: &str) -> Result<Vec<DiagramRecord>, String> {
    let dir = diagrams_dir(ws, slug)?;
    let manifest = read_manifest(&dir);
    let mut out = Vec::with_capacity(manifest.diagrams.len());
    for meta in manifest.diagrams {
        let code = match fs::read_to_string(dir.join(format!("{}.mmd", meta.id))) {
            Ok(c) => c,
            Err(_) => continue,
        };
        out.push(DiagramRecord { meta, code });
    }
    Ok(out)
}

/// Delete a single diagram (source + manifest entry).
pub fn delete_diagram(ws: &str, slug: &str, id: &str) -> Result<(), String> {
    validate_id(id)?;
    let dir = diagrams_dir(ws, slug)?;
    let mut manifest = read_manifest(&dir);
    let before = manifest.diagrams.len();
    manifest.diagrams.retain(|d| d.id != id);
    if manifest.diagrams.len() == before {
        return Err(format!("diagram '{id}' not found"));
    }
    let _ = fs::remove_file(dir.join(format!("{id}.mmd")));
    write_manifest(&dir, &manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_ws() -> String {
        let dir = std::env::temp_dir().join(format!("blx-mmd-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().to_string()
    }

    #[test]
    fn create_list_delete_roundtrip() {
        let ws = tmp_ws();
        let rec = create_diagram(
            &ws,
            "my-plan",
            "Auth Flow",
            "flowchart TD\n A-->B",
            "flowchart",
            Some("setup-auth".into()),
            None,
        )
        .unwrap();
        assert_eq!(rec.meta.id, "auth-flow");
        assert_eq!(rec.meta.task_id.as_deref(), Some("setup-auth"));

        let listed = list_diagrams(&ws, "my-plan").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].code, "flowchart TD\n A-->B");

        delete_diagram(&ws, "my-plan", "auth-flow").unwrap();
        assert!(list_diagrams(&ws, "my-plan").unwrap().is_empty());
    }

    #[test]
    fn id_collisions_get_suffixed() {
        let ws = tmp_ws();
        let a = create_diagram(&ws, "p", "Flow", "a", "flowchart", None, None).unwrap();
        let b = create_diagram(&ws, "p", "Flow", "b", "flowchart", None, None).unwrap();
        assert_eq!(a.meta.id, "flow");
        assert_eq!(b.meta.id, "flow-2");
    }

    #[test]
    fn rejects_path_traversal_id() {
        let ws = tmp_ws();
        assert!(create_diagram(&ws, "p", "x", "c", "k", None, Some("../evil".into())).is_err());
    }
}
