//! Rust indexer. Produces one [`ProjectUnit`] per `Cargo.toml` that declares a
//! `[package]`, with its `src/` module tree. No root `Cargo.toml` is required:
//! virtual workspaces, member crates, and standalone crates all work because
//! every package manifest is treated independently.

use super::{IndexContext, Indexer};
use crate::memory::architecture::unit::{ProjectUnit, UnitKind};
use std::fs;

pub struct RustIndexer;

impl Indexer for RustIndexer {
    fn kind(&self) -> UnitKind {
        UnitKind::Rust
    }

    fn index(&self, ctx: &IndexContext) -> Result<Vec<ProjectUnit>, String> {
        let mut units = Vec::new();
        for manifest_rel in ctx
            .tracked
            .iter()
            .filter(|p| p.rsplit('/').next() == Some("Cargo.toml"))
        {
            let manifest_abs = ctx.workspace_root.join(manifest_rel);
            let Ok(body) = fs::read_to_string(&manifest_abs) else {
                continue;
            };
            let Some(name) = package_name(&body) else {
                continue; // virtual workspace manifest, no [package]
            };
            let dir = directory_of(manifest_rel);
            let source_root = join_rel(&dir, "src");
            let mut unit = ProjectUnit::new(UnitKind::Rust, name);
            unit.root_rel = dir;
            unit.manifest_rel = Some(manifest_rel.clone());
            unit.source_root_rel = Some(source_root.clone());

            let source_prefix = format!("{source_root}/");
            for rel in ctx
                .tracked
                .iter()
                .filter(|rel| rel.ends_with(".rs") && rel.starts_with(&source_prefix))
                .cloned()
            {
                add_file(ctx, &mut unit, &source_root, rel);
            }
            units.push(unit);
        }
        Ok(units)
    }
}

fn add_file(ctx: &IndexContext, unit: &mut ProjectUnit, source_root: &str, rel: String) {
    let module_rel = rel
        .strip_prefix(&format!("{source_root}/"))
        .unwrap_or(rel.as_str());
    let parts = module_path_parts(module_rel);
    let declarations = fs::read_to_string(ctx.workspace_root.join(&rel))
        .map(|body| parse_mod_declarations(&body))
        .unwrap_or_default();
    unit.source_paths.push(rel);
    if parts.is_empty() {
        unit.root_declarations.extend(declarations);
        return;
    }
    let summary = unit.top_modules.entry(parts[0].clone()).or_default();
    summary.file_count += 1;
    if let Some(second) = parts.get(1) {
        summary.second_level.insert(second.clone());
        if parts.len() > 2 {
            summary.deeper_count += 1;
        }
    }
    for decl in declarations {
        summary.declarations.insert(decl);
    }
}

fn module_path_parts(module_rel: &str) -> Vec<String> {
    let without_ext = module_rel.trim_end_matches(".rs");
    if without_ext == "lib" || without_ext == "main" || without_ext == "mod" {
        return Vec::new();
    }
    let mut parts: Vec<String> = without_ext
        .split('/')
        .filter(|part| *part != "mod" && !part.is_empty())
        .map(str::to_owned)
        .collect();
    if parts
        .last()
        .is_some_and(|part| part == "lib" || part == "main")
    {
        parts.pop();
    }
    parts
}

fn parse_mod_declarations(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        let trimmed = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
        let Some(rest) = trimmed.strip_prefix("mod ") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect();
        if !name.is_empty() {
            out.push(name);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn package_name(toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package {
            if let Some(value) = parse_string_assignment(trimmed, "name") {
                return Some(value);
            }
        }
    }
    None
}

fn parse_string_assignment(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let value = rest.strip_prefix('=')?.trim();
    parse_quoted(value)
}

fn parse_quoted(value: &str) -> Option<String> {
    let value = value.trim();
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let end = value[1..].find(quote)? + 1;
    Some(value[1..end].to_owned())
}

/// Directory portion of a manifest path (`""` for a root manifest).
fn directory_of(manifest_rel: &str) -> String {
    match manifest_rel.rfind('/') {
        Some(i) => manifest_rel[..i].to_owned(),
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
