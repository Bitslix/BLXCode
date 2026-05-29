//! Makefile indexer for projects built with plain GNU Make rather than CMake
//! (typical of hand-rolled C/C++ codebases). A Makefile carries no reliable
//! project name or module list, so this behaves like a tree map labelled
//! `make`: modules are the first path segment of each recognized source file,
//! and the detected languages are reported. Runs only as a fallback tier when
//! none of the manifest-based stacks (Rust/Node/Python/CMake) matched.

use super::{IndexContext, Indexer};
use crate::memory::architecture::common::{dominant_languages, extension_of, language_for_extension};
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};

pub struct MakeIndexer;

impl Indexer for MakeIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Make
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let name = ctx
            .workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_owned();

        let mut unit = ProjectUnit::new(UnitKind::Make, name);
        unit.source_root_rel = Some(".".to_owned());

        let sources: Vec<String> = ctx
            .tracked
            .iter()
            .filter(|rel| {
                extension_of(rel)
                    .as_deref()
                    .and_then(language_for_extension)
                    .is_some()
            })
            .cloned()
            .collect();

        for rel in &sources {
            let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
            unit.source_paths.push(rel.clone());
            match parts.as_slice() {
                [] | [_] => {}
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

        let langs = dominant_languages(&sources);
        if !langs.is_empty() {
            unit.extra_notes.push(format!(
                "Languages: {}",
                langs
                    .iter()
                    .take(8)
                    .map(|(lang, n)| format!("{lang} × {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        Ok(vec![unit])
    }
}
