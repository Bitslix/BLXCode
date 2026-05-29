//! Jai indexer. Jai has no standard package manifest, so this produces a single
//! whole-tree `jai` unit (named after the workspace directory) whenever `.jai`
//! sources are present, grouping them by their first path segment. Detection is
//! purely extension-based.

use super::{IndexContext, Indexer};
use crate::memory::architecture::common::whole_tree_unit;
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};

pub struct JaiIndexer;

impl Indexer for JaiIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Jai
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let unit = whole_tree_unit(ctx.workspace_root, ctx.tracked, UnitKind::Jai, &["jai"]);
        Ok(if unit.source_paths.is_empty() {
            Vec::new()
        } else {
            vec![unit]
        })
    }
}
