//! Project-stack detection: decides which specialized indexers to run for a
//! workspace based on the manifests present in its tracked files.
//!
//! The Generic indexer is intentionally *not* returned here. It is a fallback
//! the orchestrator runs only when no specialized indexer produced a unit, so
//! a recognized single-language workspace does not also get a redundant
//! whole-tree generic map.

use super::unit::UnitKind;

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn has_manifest(tracked: &[String], filename: &str) -> bool {
    tracked.iter().any(|p| basename(p) == filename)
}

/// Specialized unit kinds to index, in priority order. May be empty (e.g. a
/// repository with no recognized manifest), in which case the orchestrator
/// falls back to the Generic indexer.
pub fn detect_project_stack(tracked: &[String]) -> Vec<UnitKind> {
    let mut kinds = Vec::new();
    if has_manifest(tracked, "Cargo.toml") {
        kinds.push(UnitKind::Rust);
    }
    if has_manifest(tracked, "package.json") {
        kinds.push(UnitKind::Node);
    }
    if has_manifest(tracked, "pyproject.toml") || has_manifest(tracked, "setup.py") {
        kinds.push(UnitKind::Python);
    }
    if has_manifest(tracked, "CMakeLists.txt") {
        kinds.push(UnitKind::Cmake);
    }
    kinds
}

/// Whether the workspace has a GNU-style Makefile. Treated as a fallback tier
/// (below the four manifest-based stacks) so a Makefile that merely wraps a
/// Cargo/Node/CMake build does not spawn a redundant unit, while a plain C
/// project built only with Make is still recognized instead of falling all the
/// way through to the Generic map.
pub fn has_makefile(tracked: &[String]) -> bool {
    tracked
        .iter()
        .any(|p| matches!(basename(p), "Makefile" | "makefile" | "GNUmakefile"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_and_node() {
        let tracked = vec![
            "Cargo.toml".to_owned(),
            "src/lib.rs".to_owned(),
            "package.json".to_owned(),
        ];
        assert_eq!(
            detect_project_stack(&tracked),
            vec![UnitKind::Rust, UnitKind::Node]
        );
    }

    #[test]
    fn empty_when_no_manifest() {
        let tracked = vec!["README.md".to_owned(), "src/main.ts".to_owned()];
        assert!(detect_project_stack(&tracked).is_empty());
    }

    #[test]
    fn ignores_nested_manifest_basename_match() {
        // A path whose basename is not exactly a manifest must not trigger.
        let tracked = vec!["docs/Cargo.toml.md".to_owned()];
        assert!(detect_project_stack(&tracked).is_empty());
    }
}
