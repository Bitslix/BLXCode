//! Python indexer. One [`ProjectUnit`] per `pyproject.toml` (or `setup.py`).
//! Top-level modules are the importable packages (directories with an
//! `__init__.py`) directly under the project's source base, where the base is
//! `src/` when present, otherwise the project root.

use super::{IndexContext, Indexer};
use crate::memory::architecture::common::extension_of;
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};
use std::collections::BTreeSet;
use std::fs;

pub struct PythonIndexer;

impl Indexer for PythonIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Python
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let manifests: Vec<&String> = ctx
            .tracked
            .iter()
            .filter(|p| {
                matches!(p.rsplit('/').next(), Some("pyproject.toml") | Some("setup.py"))
            })
            .collect();
        // Prefer one unit per directory, favoring pyproject.toml over setup.py.
        let mut by_dir: std::collections::BTreeMap<String, &String> = std::collections::BTreeMap::new();
        for m in manifests {
            let dir = directory_of(m);
            let is_pyproject = m.ends_with("pyproject.toml");
            by_dir
                .entry(dir)
                .and_modify(|cur| {
                    if is_pyproject && !cur.ends_with("pyproject.toml") {
                        *cur = m;
                    }
                })
                .or_insert(m);
        }

        let tracked_set: BTreeSet<&String> = ctx.tracked.iter().collect();
        let mut units = Vec::new();
        for (dir, manifest_rel) in by_dir {
            let name = fs::read_to_string(ctx.workspace_root.join(manifest_rel))
                .ok()
                .and_then(|body| pyproject_name(&body))
                .unwrap_or_else(|| fallback_name(&dir));

            let py_files: Vec<String> = ctx
                .tracked
                .iter()
                .filter(|rel| extension_of(rel).as_deref() == Some("py") && under(&dir, rel))
                .cloned()
                .collect();

            let src_base = join_rel(&dir, "src");
            let has_src = py_files.iter().any(|rel| under(&src_base, rel));
            let base = if has_src { src_base.clone() } else { dir.clone() };

            let mut unit = ProjectUnit::new(UnitKind::Python, name);
            unit.root_rel = dir.clone();
            unit.manifest_rel = Some(manifest_rel.clone());
            unit.source_root_rel = Some(base.clone());

            for rel in py_files {
                let inner = strip_base(&rel, &base);
                let parts: Vec<&str> = inner.split('/').filter(|s| !s.is_empty()).collect();
                unit.source_paths.push(rel.clone());
                match parts.as_slice() {
                    [] | [_] => {
                        // module file directly under the base
                        if let Some(file) = parts.first() {
                            unit.root_declarations.push((*file).to_owned());
                        }
                    }
                    [first, rest @ ..] => {
                        let pkg_init = join_rel(&base, &format!("{first}/__init__.py"));
                        if !tracked_set.contains(&pkg_init) {
                            // not an importable package; treat as a loose file group
                            continue;
                        }
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
            units.push(unit);
        }
        Ok(units)
    }
}

/// Extract `name` from the `[project]` table of a pyproject.toml.
fn pyproject_name(body: &str) -> Option<String> {
    let mut in_project = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_project = trimmed == "[project]";
            continue;
        }
        if in_project {
            if let Some(rest) = trimmed.strip_prefix("name") {
                if let Some(eq) = rest.find('=') {
                    let value = rest[eq + 1..].trim();
                    let value = value.trim_matches(|c| c == '"' || c == '\'');
                    if !value.is_empty() {
                        return Some(value.to_owned());
                    }
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
