//! Go indexer. One [`ProjectUnit`] per `go.mod` (the module name comes from its
//! `module` directive); files are attributed to the nearest enclosing module so
//! nested modules don't fold into a parent. A Go project without any `go.mod`
//! (GOPATH-style) still gets a single whole-tree `go` unit rather than falling
//! through to the Generic map.

use super::{IndexContext, Indexer};
use crate::memory::architecture::common::{
    attribute_by_top_segment, directory_of, ext_in, under, whole_tree_unit,
};
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};
use std::fs;

pub struct GoIndexer;

impl Indexer for GoIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Go
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let module_dirs: Vec<String> = ctx
            .tracked
            .iter()
            .filter(|p| p.rsplit('/').next() == Some("go.mod"))
            .map(|p| directory_of(p))
            .collect();

        if module_dirs.is_empty() {
            let unit = whole_tree_unit(ctx.workspace_root, ctx.tracked, UnitKind::Go, &["go"]);
            return Ok(if unit.source_paths.is_empty() {
                Vec::new()
            } else {
                vec![unit]
            });
        }

        let mut units = Vec::new();
        for manifest_rel in ctx
            .tracked
            .iter()
            .filter(|p| p.rsplit('/').next() == Some("go.mod"))
        {
            let dir = directory_of(manifest_rel);
            let name = fs::read_to_string(ctx.workspace_root.join(manifest_rel))
                .ok()
                .and_then(|body| module_name(&body))
                .unwrap_or_else(|| fallback_name(&dir));

            let mut unit = ProjectUnit::new(UnitKind::Go, name);
            unit.root_rel = dir.clone();
            unit.manifest_rel = Some(manifest_rel.clone());
            unit.source_root_rel = Some(if dir.is_empty() { ".".to_owned() } else { dir.clone() });

            let owned: Vec<String> = ctx
                .tracked
                .iter()
                .filter(|rel| {
                    ext_in(rel, &["go"])
                        && under(&dir, rel)
                        && nearest_dir(&module_dirs, rel) == dir
                })
                .cloned()
                .collect();
            for rel in owned {
                attribute_by_top_segment(&mut unit, &dir, rel);
            }
            units.push(unit);
        }
        Ok(units)
    }
}

/// The last path segment of the `module <path>` directive.
fn module_name(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("module ") {
            let path = rest.trim().trim_matches('"');
            let name = path.rsplit('/').next().unwrap_or(path);
            if !name.is_empty() {
                return Some(name.to_owned());
            }
        }
    }
    None
}

fn fallback_name(dir: &str) -> String {
    if dir.is_empty() {
        "root".to_owned()
    } else {
        dir.rsplit('/').next().unwrap_or(dir).to_owned()
    }
}

fn nearest_dir(dirs: &[String], rel: &str) -> String {
    dirs.iter()
        .filter(|dir| under(dir, rel))
        .max_by_key(|dir| dir.len())
        .cloned()
        .unwrap_or_default()
}
