//! CMake indexer. One [`ProjectUnit`] per `CMakeLists.txt` that contains a
//! `project(...)` declaration. Modules are the conventional source roots
//! (`src/`, `include/`, `lib/`) and their direct children.

use super::{IndexContext, Indexer};
use crate::memory::architecture::common::extension_of;
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};
use std::fs;

pub struct CmakeIndexer;

const SOURCE_DIRS: &[&str] = &["src", "include", "lib", "source"];
const CODE_EXTS: &[&str] = &[
    "c", "cc", "cpp", "cxx", "c++", "h", "hh", "hpp", "hxx", "h++", "inl", "ipp",
];

impl Indexer for CmakeIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Cmake
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let mut units = Vec::new();
        for manifest_rel in ctx
            .tracked
            .iter()
            .filter(|p| p.rsplit('/').next() == Some("CMakeLists.txt"))
        {
            let body = match fs::read_to_string(ctx.workspace_root.join(manifest_rel)) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let Some(name) = project_name(&body) else {
                continue; // sub-list without project(); fold into the root unit
            };
            let dir = directory_of(manifest_rel);

            let mut unit = ProjectUnit::new(UnitKind::Cmake, name);
            unit.root_rel = dir.clone();
            unit.manifest_rel = Some(manifest_rel.clone());
            unit.source_root_rel = Some(if dir.is_empty() { ".".to_owned() } else { dir.clone() });

            for rel in ctx.tracked.iter().filter(|rel| {
                under(&dir, rel)
                    && extension_of(rel)
                        .map(|e| CODE_EXTS.contains(&e.as_str()))
                        .unwrap_or(false)
            }) {
                let inner = strip_base(rel, &dir);
                let parts: Vec<&str> = inner.split('/').filter(|s| !s.is_empty()).collect();
                unit.source_paths.push(rel.clone());
                match parts.as_slice() {
                    [] | [_] => {}
                    [first, rest @ ..] => {
                        if !SOURCE_DIRS.contains(first) {
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

/// Extract the first `project(Name ...)` argument.
fn project_name(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if let Some(idx) = lower.find("project(") {
            let after = &trimmed[idx + "project(".len()..];
            let name: String = after
                .trim_start()
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != ')' && *c != ',')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
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
