//! Indexer registry. Each indexer turns a workspace + its tracked files into
//! zero or more [`ProjectUnit`]s. The orchestrator runs the specialized
//! indexers selected by `detect_project_stack`, then falls back to the Generic
//! indexer if none produced a unit. Indexer errors are collected as warnings
//! and never abort the rebuild.

mod cmake;
mod generic;
mod go;
mod jai;
mod make;
mod node;
mod python;
mod rust;
mod zig;

use std::collections::BTreeSet;
use std::path::Path;

use super::detect::{detect_project_stack, has_makefile};
use super::unit::{ProjectUnit, UnitKind};

/// Inputs shared by every indexer.
pub struct IndexContext<'a> {
    pub workspace_root: &'a Path,
    pub tracked: &'a [String],
}

/// A language/build-system indexer.
pub trait Indexer {
    fn kind(&self) -> UnitKind;
    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String>;
}

fn indexer_for(kind: UnitKind) -> Box<dyn Indexer> {
    match kind {
        UnitKind::Rust => Box::new(rust::RustIndexer),
        UnitKind::Node => Box::new(node::NodeIndexer),
        UnitKind::Python => Box::new(python::PythonIndexer),
        UnitKind::Cmake => Box::new(cmake::CmakeIndexer),
        UnitKind::Make => Box::new(make::MakeIndexer),
        UnitKind::Go => Box::new(go::GoIndexer),
        UnitKind::Zig => Box::new(zig::ZigIndexer),
        UnitKind::Jai => Box::new(jai::JaiIndexer),
        UnitKind::Generic => Box::new(generic::GenericIndexer),
    }
}

/// Run all applicable indexers. Returns the collected units (slug-deduplicated)
/// and any non-fatal warnings. Never returns `Err` for a missing manifest.
pub fn run_all_indexers(
    workspace_root: &Path,
    tracked: &[String],
) -> (Vec<ProjectUnit>, Vec<String>) {
    let ctx = IndexContext {
        workspace_root,
        tracked,
    };
    let mut units = Vec::new();
    let mut warnings = Vec::new();

    for kind in detect_project_stack(tracked) {
        let indexer = indexer_for(kind);
        match indexer.index(&ctx) {
            Ok(mut produced) => units.append(&mut produced),
            Err(e) => warnings.push(format!("{} indexer: {e}", indexer.kind().as_str())),
        }
    }

    // Fallback tier: a plain Makefile project (e.g. C without CMake) before the
    // language-agnostic Generic map, so a rebuild always yields at least one unit.
    if units.is_empty() && has_makefile(tracked) {
        let indexer = indexer_for(UnitKind::Make);
        match indexer.index(&ctx) {
            Ok(mut produced) => units.append(&mut produced),
            Err(e) => warnings.push(format!("{} indexer: {e}", indexer.kind().as_str())),
        }
    }

    if units.is_empty() {
        let indexer = indexer_for(UnitKind::Generic);
        match indexer.index(&ctx) {
            Ok(mut produced) => units.append(&mut produced),
            Err(e) => warnings.push(format!("{} indexer: {e}", indexer.kind().as_str())),
        }
    }

    dedup_slugs(&mut units);
    units.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
    (units, warnings)
}

/// Ensure module-note filenames don't collide by suffixing duplicate slugs.
fn dedup_slugs(units: &mut [ProjectUnit]) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for unit in units.iter_mut() {
        let base = unit.slug();
        if seen.insert(base.clone()) {
            continue;
        }
        let mut n = 2;
        loop {
            let candidate = format!("{base}-{n}");
            if seen.insert(candidate.clone()) {
                // Adjust the name so slug() recomputes to the unique value.
                unit.name = format!("{} ({n})", unit.name);
                break;
            }
            n += 1;
        }
    }
}
