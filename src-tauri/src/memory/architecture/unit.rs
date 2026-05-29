//! Language-agnostic model produced by the architecture indexers.
//!
//! Every indexer (Rust, Node, Python, CMake, Generic) emits one or more
//! [`ProjectUnit`]s. The renderer in `static_index.rs` turns these into the
//! `ARCHITECTURE.md` generated table and the per-unit module notes, without
//! caring which language the unit came from.

use std::collections::{BTreeMap, BTreeSet};

/// The kind of project unit, used for slugs, frontmatter, and the index table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnitKind {
    Rust,
    Node,
    Python,
    Cmake,
    Make,
    Go,
    Zig,
    Jai,
    Generic,
}

impl UnitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            UnitKind::Rust => "rust",
            UnitKind::Node => "node",
            UnitKind::Python => "python",
            UnitKind::Cmake => "cmake",
            UnitKind::Make => "make",
            UnitKind::Go => "go",
            UnitKind::Zig => "zig",
            UnitKind::Jai => "jai",
            UnitKind::Generic => "generic",
        }
    }
}

/// Summary of a single top-level module within a unit.
#[derive(Debug, Default, Clone)]
pub struct ModuleSummary {
    /// Direct child segments (one level below the module root).
    pub second_level: BTreeSet<String>,
    /// Language-specific declarations attributed to this module
    /// (e.g. Rust `mod` names, Node entry files).
    pub declarations: BTreeSet<String>,
    /// Number of source files nested deeper than the second level.
    pub deeper_count: usize,
    /// Total number of source files under this module.
    pub file_count: usize,
}

/// One indexable unit of a workspace (a crate, package, CMake project, or the
/// whole tree for the generic fallback).
#[derive(Debug, Clone)]
pub struct ProjectUnit {
    pub kind: UnitKind,
    /// Human-facing name (package name, project name, or directory name).
    pub name: String,
    /// Unit root relative to the workspace root. Empty string = workspace root.
    pub root_rel: String,
    /// Manifest path relative to the workspace root, if any.
    pub manifest_rel: Option<String>,
    /// Display hint for where sources live (e.g. `src`, `src-tauri/src`).
    pub source_root_rel: Option<String>,
    /// Workspace-relative source file paths attributed to this unit.
    pub source_paths: Vec<String>,
    /// Top-level modules keyed by name.
    pub top_modules: BTreeMap<String, ModuleSummary>,
    /// Declarations attributed to the unit root (outside any module).
    pub root_declarations: Vec<String>,
    /// Extra rendered bullet lines (e.g. a file-extension breakdown).
    pub extra_notes: Vec<String>,
}

impl ProjectUnit {
    pub fn new(kind: UnitKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            root_rel: String::new(),
            manifest_rel: None,
            source_root_rel: None,
            source_paths: Vec::new(),
            top_modules: BTreeMap::new(),
            root_declarations: Vec::new(),
            extra_notes: Vec::new(),
        }
    }

    /// Stable slug used for the module note filename, e.g. `rust-blxcode`.
    pub fn slug(&self) -> String {
        format!("{}-{}", self.kind.as_str(), sanitize_slug(&self.name))
    }
}

/// Lowercase a name and collapse runs of non-alphanumeric characters into a
/// single dash, suitable for a filename slug.
pub fn sanitize_slug(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "unit".to_owned()
    } else {
        out
    }
}
