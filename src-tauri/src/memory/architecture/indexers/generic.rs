//! Generic fallback indexer. Always produces exactly one [`ProjectUnit`] for
//! the whole workspace tree: modules are the first one or two path segments,
//! plus a file-extension breakdown. Used when no specialized indexer matched,
//! so a rebuild never fails just because a manifest is missing.

use super::{IndexContext, Indexer};
use crate::memory::architecture::common::{dominant_languages, extension_of};
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};
use std::collections::BTreeMap;

pub struct GenericIndexer;

/// Cap on the number of source paths recorded, to keep notes bounded.
const FILE_CAP: usize = 2000;

impl Indexer for GenericIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Generic
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let name = ctx
            .workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_owned();

        let mut unit = ProjectUnit::new(UnitKind::Generic, name);
        unit.source_root_rel = Some(".".to_owned());

        let mut ext_counts: BTreeMap<String, usize> = BTreeMap::new();
        let files = &ctx.tracked[..ctx.tracked.len().min(FILE_CAP)];
        for rel in files {
            *ext_counts
                .entry(extension_of(rel).unwrap_or_else(|| "(none)".to_owned()))
                .or_default() += 1;

            let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
            unit.source_paths.push(rel.clone());
            match parts.as_slice() {
                [] | [_] => {} // root-level file
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

        if unit.source_paths.is_empty() {
            unit.extra_notes
                .push("No tracked files found; the workspace appears empty.".to_owned());
        } else {
            let langs = dominant_languages(files);
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
            unit.extra_notes.push(format!(
                "File types: {}",
                ext_counts
                    .iter()
                    .rev()
                    .map(|(ext, n)| format!("`{ext}` × {n}"))
                    .take(12)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if ctx.tracked.len() > FILE_CAP {
                unit.extra_notes.push(format!(
                    "{} tracked files total; capped at {FILE_CAP} for this map.",
                    ctx.tracked.len()
                ));
            }
        }

        Ok(vec![unit])
    }
}
