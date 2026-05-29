//! Node/TypeScript indexer. One [`ProjectUnit`] per `package.json`. Modules are
//! the direct children of the package's `src/` directory (falling back to the
//! package root when there is no `src/`). Files are attributed to the nearest
//! enclosing package so workspace member packages don't fold into the root.

use super::{IndexContext, Indexer};
use crate::memory::architecture::common::extension_of;
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};
use std::fs;

pub struct NodeIndexer;

const CODE_EXTS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "vue", "svelte", "astro",
];

impl Indexer for NodeIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Node
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let package_dirs: Vec<String> = ctx
            .tracked
            .iter()
            .filter(|p| p.rsplit('/').next() == Some("package.json"))
            .map(|p| directory_of(p))
            .collect();

        let mut units = Vec::new();
        for manifest_rel in ctx
            .tracked
            .iter()
            .filter(|p| p.rsplit('/').next() == Some("package.json"))
        {
            let dir = directory_of(manifest_rel);
            let name = fs::read_to_string(ctx.workspace_root.join(manifest_rel))
                .ok()
                .and_then(|body| package_json_name(&body))
                .unwrap_or_else(|| fallback_name(&dir));

            // Files belonging to this package: under its dir, but not under a
            // deeper package dir (a nested workspace member).
            let owned: Vec<String> = ctx
                .tracked
                .iter()
                .filter(|rel| {
                    is_code_file(rel)
                        && under(&dir, rel)
                        && nearest_package_dir(&package_dirs, rel) == dir
                })
                .cloned()
                .collect();

            let src_root = join_rel(&dir, "src");
            let has_src = owned.iter().any(|rel| under(&src_root, rel));
            let (module_base, source_root_rel) = if has_src {
                (src_root.clone(), src_root)
            } else {
                (dir.clone(), dir.clone())
            };

            let mut unit = ProjectUnit::new(UnitKind::Node, name);
            unit.root_rel = dir;
            unit.manifest_rel = Some(manifest_rel.clone());
            unit.source_root_rel = Some(source_root_rel);

            for rel in owned {
                attribute(&mut unit, &module_base, rel);
            }
            units.push(unit);
        }
        Ok(units)
    }
}

/// Group one file into the unit by its first path segment below `module_base`.
fn attribute(unit: &mut ProjectUnit, module_base: &str, rel: String) {
    let parts: Vec<String> = strip_base(&rel, module_base)
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    unit.source_paths.push(rel);
    match parts.as_slice() {
        [] => {}
        [file] => {
            // A file sitting directly in the module base is a root entry.
            unit.root_declarations.push((*file).to_owned());
        }
        [first, rest @ ..] => {
            let summary = unit.top_modules.entry((*first).to_owned()).or_default();
            summary.file_count += 1;
            if let Some(second) = rest.first() {
                summary.second_level.insert((*second).to_owned());
                if rest.len() > 1 {
                    summary.deeper_count += 1;
                }
            }
        }
    }
}

fn is_code_file(rel: &str) -> bool {
    extension_of(rel)
        .map(|ext| CODE_EXTS.contains(&ext.as_str()))
        .unwrap_or(false)
}

/// Minimal `"name": "..."` extraction from a package.json body.
fn package_json_name(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .filter(|s| !s.is_empty())
}

fn fallback_name(dir: &str) -> String {
    if dir.is_empty() {
        "root".to_owned()
    } else {
        dir.rsplit('/').next().unwrap_or(dir).to_owned()
    }
}

fn nearest_package_dir(package_dirs: &[String], rel: &str) -> String {
    package_dirs
        .iter()
        .filter(|dir| under(dir, rel))
        .max_by_key(|dir| dir.len())
        .cloned()
        .unwrap_or_default()
}

/// Whether `rel` lives under directory `dir` (`dir == ""` is the workspace root).
fn under(dir: &str, rel: &str) -> bool {
    if dir.is_empty() {
        true
    } else {
        rel.starts_with(&format!("{dir}/"))
    }
}

fn strip_base<'a>(rel: &'a str, base: &str) -> &'a str {
    if base.is_empty() {
        rel
    } else {
        rel.strip_prefix(&format!("{base}/")).unwrap_or(rel)
    }
}

fn directory_of(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_owned(),
        None => String::new(),
    }
}

fn join_rel(dir: &str, child: &str) -> String {
    if dir.is_empty() {
        child.to_owned()
    } else {
        format!("{dir}/{child}")
    }
}
