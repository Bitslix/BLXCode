//! Zig indexer. One [`ProjectUnit`] per `build.zig`; the name comes from a
//! sibling `build.zig.zon` (`.name = "..."`) when present, otherwise the
//! directory name. Modules are the children of `src/` (or the build root when
//! there is no `src/`). A Zig project without a `build.zig` still gets a single
//! whole-tree `zig` unit instead of falling through to the Generic map.

use super::{IndexContext, Indexer};
use crate::memory::architecture::common::{
    attribute_by_top_segment, directory_of, ext_in, join_rel, under, whole_tree_unit,
};
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};
use std::fs;

pub struct ZigIndexer;

impl Indexer for ZigIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Zig
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let build_dirs: Vec<String> = ctx
            .tracked
            .iter()
            .filter(|p| p.rsplit('/').next() == Some("build.zig"))
            .map(|p| directory_of(p))
            .collect();

        if build_dirs.is_empty() {
            let unit = whole_tree_unit(ctx.workspace_root, ctx.tracked, UnitKind::Zig, &["zig"]);
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
            .filter(|p| p.rsplit('/').next() == Some("build.zig"))
        {
            let dir = directory_of(manifest_rel);
            let zon = join_rel(&dir, "build.zig.zon");
            let name = fs::read_to_string(ctx.workspace_root.join(&zon))
                .ok()
                .and_then(|body| zon_name(&body))
                .unwrap_or_else(|| fallback_name(&dir));

            let owned: Vec<String> = ctx
                .tracked
                .iter()
                .filter(|rel| {
                    ext_in(rel, &["zig"])
                        && under(&dir, rel)
                        && nearest_dir(&build_dirs, rel) == dir
                })
                .cloned()
                .collect();

            let src_root = join_rel(&dir, "src");
            let has_src = owned.iter().any(|rel| under(&src_root, rel));
            let (base, source_root_rel) = if has_src {
                (src_root.clone(), src_root)
            } else if dir.is_empty() {
                (String::new(), ".".to_owned())
            } else {
                (dir.clone(), dir.clone())
            };

            let mut unit = ProjectUnit::new(UnitKind::Zig, name);
            unit.root_rel = dir.clone();
            unit.manifest_rel = Some(manifest_rel.clone());
            unit.source_root_rel = Some(source_root_rel);
            for rel in owned {
                attribute_by_top_segment(&mut unit, &base, rel);
            }
            units.push(unit);
        }
        Ok(units)
    }
}

/// Extract `.name = "..."` from a `build.zig.zon` body.
fn zon_name(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(".name") {
            if let Some(eq) = rest.find('=') {
                let value = rest[eq + 1..].trim().trim_end_matches(',');
                // Accept both string (`"foo"`) and enum-literal (`.foo`) forms.
                let value = value.trim_matches('"').trim_start_matches('.');
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
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
