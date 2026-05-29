//! Shared helpers for the architecture indexers: workspace file enumeration
//! (git-aware with a filesystem-walk fallback) and small path utilities.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::git_info::is_git_repository;
use crate::proc::command;
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};

/// Directory names skipped during enumeration. Any dot-prefixed directory is
/// also skipped (covers `.git`, `.tauri`, `.venv`, `.next`, `.idea`, …).
const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "build",
    "out",
    "vendor",
    "__pycache__",
    "coverage",
];

/// Whether a directory segment should be skipped during enumeration.
pub fn is_skipped_dir(name: &str) -> bool {
    name.starts_with('.') || SKIP_DIRS.contains(&name)
}

fn path_is_skipped(rel: &str) -> bool {
    let mut iter = rel.split('/').peekable();
    while let Some(seg) = iter.next() {
        // Only treat intermediate segments as directories; the final segment
        // is the file name and a leading dot there (e.g. `.gitignore`) is fine.
        if iter.peek().is_some() && is_skipped_dir(seg) {
            return true;
        }
    }
    false
}

/// All workspace-relative tracked files (forward-slash separated, sorted).
///
/// Uses `git ls-files` when the workspace is a git repository, otherwise walks
/// the filesystem. In both cases skip directories are excluded. This never
/// fails: a missing git binary or unreadable directory degrades to a walk or
/// an empty list.
pub fn enumerate_tracked_files(workspace_root: &Path) -> Vec<String> {
    if is_git_repository(workspace_root) {
        if let Ok(output) = command("git")
            .arg("-C")
            .arg(workspace_root)
            .arg("ls-files")
            .output()
        {
            if output.status.success() {
                let mut files: Vec<String> = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(|line| line.replace('\\', "/"))
                    .filter(|line| !path_is_skipped(line))
                    .collect();
                files.sort();
                files.dedup();
                return files;
            }
        }
    }

    let mut files = Vec::new();
    walk_files(workspace_root, workspace_root, &mut files);
    files.sort();
    files.dedup();
    files
}

fn walk_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = entry.path();
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            if is_skipped_dir(&name) {
                continue;
            }
            walk_files(root, &path, out);
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// File extension (lowercased, no dot) of a forward-slash path, if any.
pub fn extension_of(path: &str) -> Option<String> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let dot = file.rfind('.')?;
    if dot == 0 {
        return None;
    }
    Some(file[dot + 1..].to_ascii_lowercase())
}

/// Human-readable language label for a source extension. This is deliberately
/// broad (well beyond the languages with dedicated indexers) so the Generic and
/// Make maps can name whatever a workspace actually contains — Ada, OCaml, Go,
/// Zig, Haskell, plain JavaScript, and so on.
pub fn language_for_extension(ext: &str) -> Option<&'static str> {
    let lang = match ext {
        "rs" => "Rust",
        "ts" | "tsx" | "mts" | "cts" => "TypeScript",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "vue" => "Vue",
        "svelte" => "Svelte",
        "astro" => "Astro",
        "py" | "pyi" | "pyx" => "Python",
        "rb" => "Ruby",
        "go" => "Go",
        "zig" => "Zig",
        "jai" => "Jai",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "c++" | "hpp" | "hh" | "hxx" | "h++" | "inl" | "ipp" => "C++",
        "m" | "mm" => "Objective-C",
        "cs" => "C#",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "scala" | "sc" => "Scala",
        "swift" => "Swift",
        "ml" | "mli" => "OCaml",
        "re" | "rei" => "ReasonML",
        "hs" | "lhs" => "Haskell",
        "elm" => "Elm",
        "erl" | "hrl" => "Erlang",
        "ex" | "exs" => "Elixir",
        "clj" | "cljs" | "cljc" => "Clojure",
        "ada" | "adb" | "ads" => "Ada",
        "f" | "for" | "f90" | "f95" | "f03" | "f08" => "Fortran",
        "pas" | "pp" => "Pascal",
        "d" => "D",
        "nim" => "Nim",
        "cr" => "Crystal",
        "jl" => "Julia",
        "lua" => "Lua",
        "pl" | "pm" => "Perl",
        "php" => "PHP",
        "dart" => "Dart",
        "r" => "R",
        "sh" | "bash" | "zsh" => "Shell",
        "ps1" | "psm1" => "PowerShell",
        "asm" | "s" => "Assembly",
        "sql" => "SQL",
        "tf" => "Terraform",
        "proto" => "Protobuf",
        _ => return None,
    };
    Some(lang)
}

/// Directory portion of a forward-slash path (`""` for a root-level entry).
pub fn directory_of(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_owned(),
        None => String::new(),
    }
}

/// Join a relative directory with a child segment (`""` dir = the child itself).
pub fn join_rel(dir: &str, child: &str) -> String {
    if dir.is_empty() {
        child.to_owned()
    } else {
        format!("{dir}/{child}")
    }
}

/// Whether `rel` lives under directory `dir` (`dir == ""` is the workspace root).
pub fn under(dir: &str, rel: &str) -> bool {
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

/// Whether a path's extension is one of `exts` (case-insensitive, no dot).
pub fn ext_in(rel: &str, exts: &[&str]) -> bool {
    extension_of(rel)
        .map(|e| exts.contains(&e.as_str()))
        .unwrap_or(false)
}

/// Attribute one source file to a unit by its first path segment below `base`.
/// A file directly under `base` is recorded as a root declaration; otherwise the
/// first segment becomes a top-level module and the second its submodule.
pub fn attribute_by_top_segment(unit: &mut ProjectUnit, base: &str, rel: String) {
    let parts: Vec<String> = strip_base(&rel, base)
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    unit.source_paths.push(rel);
    match parts.as_slice() {
        [] => {}
        [file] => unit.root_declarations.push(file.clone()),
        [first, rest @ ..] => {
            let summary = unit.top_modules.entry(first.clone()).or_default();
            summary.file_count += 1;
            if let Some(second) = rest.first() {
                summary.second_level.insert(second.clone());
                if rest.len() > 1 {
                    summary.deeper_count += 1;
                }
            }
        }
    }
}

/// Build a single whole-tree unit of `kind`, named after the workspace
/// directory, grouping every tracked file with one of `exts` by its first path
/// segment. Used as the no-manifest path for language indexers (e.g. Jai, or Go
/// without a `go.mod`) so those languages still get a dedicated, named unit
/// rather than falling through to the Generic map.
pub fn whole_tree_unit(
    workspace_root: &Path,
    tracked: &[String],
    kind: UnitKind,
    exts: &[&str],
) -> ProjectUnit {
    let name = workspace_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_owned();
    let mut unit = ProjectUnit::new(kind, name);
    unit.source_root_rel = Some(".".to_owned());
    for rel in tracked.iter().filter(|r| ext_in(r, exts)) {
        attribute_by_top_segment(&mut unit, "", rel.clone());
    }
    unit
}

/// Count source files per language label, sorted by descending count then name.
pub fn dominant_languages(files: &[String]) -> Vec<(&'static str, usize)> {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for rel in files {
        if let Some(lang) = extension_of(rel).as_deref().and_then(language_for_extension) {
            *counts.entry(lang).or_default() += 1;
        }
    }
    let mut out: Vec<(&'static str, usize)> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    out
}
