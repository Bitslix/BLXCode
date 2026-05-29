use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::process::Command;

use crate::agents_layout::{validate_workspace_cwd, MEMORY_REL};
use crate::git_info::{head_commit, is_git_repository};
use crate::memory::frontmatter::{serialize_frontmatter, MemoryFrontmatter};
use crate::memory::paths::{ARCHITECTURE_CATEGORY, ARCHITECTURE_INDEX};
use crate::memory::types::{ArchitectureLintReport, RebuildReport};
use crate::pointers::splice_block;

use super::state::{now_unix_string, read_state, write_state, ArchitectureState};

pub const STATIC_BEGIN: &str = "<!-- architecture:static:begin -->";
pub const STATIC_END: &str = "<!-- architecture:static:end -->";
const MODULES_DIR: &str = "architecture/modules";
const FLOWS_DIR: &str = "architecture/flows";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CrateInfo {
    name: String,
    manifest_rel: String,
    source_rel: String,
}

#[derive(Debug)]
struct CrateIndex {
    info: CrateInfo,
    source_paths: Vec<String>,
    top_modules: BTreeMap<String, ModuleSummary>,
    root_declarations: Vec<String>,
}

impl CrateIndex {
    fn new(info: CrateInfo) -> Self {
        Self {
            info,
            source_paths: Vec::new(),
            top_modules: BTreeMap::new(),
            root_declarations: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
struct ModuleSummary {
    second_level: BTreeSet<String>,
    declarations: BTreeSet<String>,
    deeper_count: usize,
}

pub fn rebuild_architecture_impl(workspace_cwd: &str) -> Result<RebuildReport, String> {
    let workspace_root = validate_workspace_cwd(workspace_cwd)?;
    rebuild_architecture_at(&workspace_root)
}

pub fn lint_architecture_impl(workspace_cwd: &str) -> Result<ArchitectureLintReport, String> {
    let workspace_root = validate_workspace_cwd(workspace_cwd)?;
    let current = head_commit(&workspace_root);
    let state = read_state(&workspace_root);
    let stale = state.as_ref().map(|s| s.git_rev != current).unwrap_or(true);
    let mut stale_paths = Vec::new();
    if stale {
        stale_paths.push(ARCHITECTURE_INDEX.to_owned());
        let modules = workspace_root.join(MEMORY_REL).join(MODULES_DIR);
        if let Ok(read) = fs::read_dir(&modules) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                    stale_paths.push(format!(
                        "{MODULES_DIR}/{}",
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                    ));
                }
            }
        }
        mark_stale_frontmatter(&workspace_root, &stale_paths)?;
    }
    Ok(ArchitectureLintReport {
        git_rev: current,
        state_git_rev: state.and_then(|s| s.git_rev),
        stale,
        stale_paths,
    })
}

pub fn generated_section_from_architecture_index(workspace_root: &Path) -> Option<String> {
    let path = workspace_root.join(MEMORY_REL).join(ARCHITECTURE_INDEX);
    let body = fs::read_to_string(path).ok()?;
    extract_static_block_inner(&body)
        .map(str::trim)
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
}

fn rebuild_architecture_at(workspace_root: &Path) -> Result<RebuildReport, String> {
    let memory_root = workspace_root.join(MEMORY_REL);
    fs::create_dir_all(memory_root.join(MODULES_DIR))
        .map_err(|e| format!("create {MODULES_DIR}: {e}"))?;
    fs::create_dir_all(memory_root.join(FLOWS_DIR))
        .map_err(|e| format!("create {FLOWS_DIR}: {e}"))?;

    let git_rev = head_commit(workspace_root);
    let crates = detect_crates(workspace_root)?;
    let rust_files = enumerate_rust_sources(workspace_root)?;
    let mut indexes = Vec::new();
    for info in crates {
        indexes.push(index_crate(workspace_root, info, &rust_files));
    }

    let mut files_changed = 0u32;
    let mut generated_paths = Vec::new();
    for index in &indexes {
        let rel = format!("{MODULES_DIR}/{}.md", index.info.name);
        let rendered = render_module_note(index, git_rev.as_deref());
        if write_if_changed(&memory_root.join(&rel), &rendered)? {
            files_changed += 1;
        }
        generated_paths.push(rel);
    }

    let index_body = render_architecture_index(&indexes, &generated_paths, git_rev.as_deref());
    if write_if_changed(&memory_root.join(ARCHITECTURE_INDEX), &index_body)? {
        files_changed += 1;
    }

    let module_count = indexes
        .iter()
        .map(|idx| idx.top_modules.len() as u32)
        .sum::<u32>();
    write_state(
        workspace_root,
        &ArchitectureState {
            git_rev: git_rev.clone(),
            generated_at: now_unix_string(),
            crate_count: indexes.len() as u32,
            module_count,
        },
    )?;

    Ok(RebuildReport {
        git_rev,
        crate_count: indexes.len() as u32,
        module_count,
        files_changed,
        generated_paths,
    })
}

fn detect_crates(workspace_root: &Path) -> Result<Vec<CrateInfo>, String> {
    let root_manifest = workspace_root.join("Cargo.toml");
    let root_body = fs::read_to_string(&root_manifest)
        .map_err(|e| format!("read {}: {e}", root_manifest.display()))?;
    let mut crates = Vec::new();
    if let Some(name) = package_name(&root_body) {
        crates.push(CrateInfo {
            name,
            manifest_rel: "Cargo.toml".to_owned(),
            source_rel: "src".to_owned(),
        });
    }
    for member in workspace_members(&root_body) {
        let manifest_rel = format!("{}/Cargo.toml", member.trim_matches('/'));
        let manifest = workspace_root.join(&manifest_rel);
        let Ok(body) = fs::read_to_string(&manifest) else {
            continue;
        };
        let Some(name) = package_name(&body) else {
            continue;
        };
        crates.push(CrateInfo {
            name,
            manifest_rel,
            source_rel: format!("{}/src", member.trim_matches('/')),
        });
    }
    crates.sort_by(|a, b| a.name.cmp(&b.name));
    crates.dedup_by(|a, b| a.name == b.name);
    Ok(crates)
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

fn workspace_members(toml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_workspace = false;
    let mut collecting = false;
    let mut buffer = String::new();
    for line in toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace = trimmed == "[workspace]";
            collecting = false;
            buffer.clear();
            continue;
        }
        if !in_workspace {
            continue;
        }
        if collecting {
            buffer.push_str(trimmed);
            if trimmed.contains(']') {
                out.extend(parse_string_array(&buffer));
                collecting = false;
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("members") {
            let Some(eq) = rest.find('=') else { continue };
            let value = rest[eq + 1..].trim();
            if value.contains('[') && !value.contains(']') {
                collecting = true;
                buffer.push_str(value);
            } else {
                out.extend(parse_string_array(value));
            }
        }
    }
    out
}

fn parse_string_assignment(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let value = rest.strip_prefix('=')?.trim();
    parse_quoted(value)
}

fn parse_string_array(value: &str) -> Vec<String> {
    let Some(start) = value.find('[') else {
        return Vec::new();
    };
    let Some(end) = value.rfind(']') else {
        return Vec::new();
    };
    value[start + 1..end]
        .split(',')
        .filter_map(|part| parse_quoted(part.trim()))
        .collect()
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

fn enumerate_rust_sources(workspace_root: &Path) -> Result<Vec<String>, String> {
    if is_git_repository(workspace_root) {
        if let Ok(output) = Command::new("git")
            .arg("-C")
            .arg(workspace_root)
            .arg("ls-files")
            .output()
        {
            if output.status.success() {
                let mut files: Vec<String> = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(str::trim)
                    .filter(|line| line.ends_with(".rs"))
                    .map(str::to_owned)
                    .collect();
                files.sort();
                return Ok(files);
            }
        }
    }

    let mut files = Vec::new();
    walk_rust_sources(workspace_root, workspace_root, &mut files);
    files.sort();
    Ok(files)
}

fn walk_rust_sources(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            if matches!(
                name.as_ref(),
                "target" | "node_modules" | "dist" | ".git" | ".tauri"
            ) {
                continue;
            }
            walk_rust_sources(root, &path, out);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

fn index_crate(workspace_root: &Path, info: CrateInfo, rust_files: &[String]) -> CrateIndex {
    let source_prefix = format!("{}/", info.source_rel.trim_matches('/'));
    let mut index = CrateIndex::new(info);
    for rel in rust_files
        .iter()
        .filter(|rel| rel.starts_with(&source_prefix))
        .cloned()
    {
        add_file_to_index(workspace_root, &mut index, rel);
    }
    index
}

fn add_file_to_index(workspace_root: &Path, index: &mut CrateIndex, rel: String) {
    let source_rel = index.info.source_rel.trim_matches('/');
    let module_rel = rel
        .strip_prefix(&format!("{source_rel}/"))
        .unwrap_or(rel.as_str());
    let module_parts = module_path_parts(module_rel);
    let declarations = fs::read_to_string(workspace_root.join(&rel))
        .map(|body| parse_mod_declarations(&body))
        .unwrap_or_default();
    index.source_paths.push(rel);
    if module_parts.is_empty() {
        index.root_declarations.extend(declarations);
        return;
    }
    let top = module_parts[0].clone();
    let summary = index.top_modules.entry(top).or_default();
    if let Some(second) = module_parts.get(1) {
        summary.second_level.insert(second.clone());
        if module_parts.len() > 2 {
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

fn render_architecture_index(
    indexes: &[CrateIndex],
    generated_paths: &[String],
    git_rev: Option<&str>,
) -> String {
    let source_paths: Vec<String> = indexes
        .iter()
        .flat_map(|idx| idx.source_paths.iter().take(8).cloned())
        .collect();
    let fm = frontmatter("Architecture", git_rev, source_paths, false);
    let mut generated = String::new();
    generated.push_str("## Generated\n\n");
    generated.push_str("| Crate | Source root | Module map |\n");
    generated.push_str("|---|---|---|\n");
    for (idx, path) in indexes.iter().zip(generated_paths) {
        generated.push_str(&format!(
            "| `{}` | `{}` | [[{}|{}]] |\n",
            idx.info.name, idx.info.source_rel, path, idx.info.name
        ));
    }
    generated.push('\n');
    generated.push_str("### Counts\n\n");
    generated.push_str(&format!("- Crates: {}\n", indexes.len()));
    generated.push_str(&format!(
        "- Top-level modules: {}\n",
        indexes
            .iter()
            .map(|idx| idx.top_modules.len())
            .sum::<usize>()
    ));
    if let Some(rev) = git_rev {
        generated.push_str(&format!("- Git revision: `{rev}`\n"));
    }

    let mut body = serialize_frontmatter(
        &fm,
        "# Architecture\n\nThis index is maintained by BLXCode's architecture map harness.\n\n## Manual\n\nAdd curated overview notes here. The generated block below is refreshed by `memory_rebuild_architecture`.\n",
    );
    body = splice_block(&body, STATIC_BEGIN, STATIC_END, &generated);
    body
}

fn render_module_note(index: &CrateIndex, git_rev: Option<&str>) -> String {
    let fm = frontmatter(
        &format!("{} modules", index.info.name),
        git_rev,
        index.source_paths.clone(),
        false,
    );
    let mut generated = String::new();
    generated.push_str(&format!("## `{}`\n\n", index.info.name));
    generated.push_str(&format!("- Manifest: `{}`\n", index.info.manifest_rel));
    generated.push_str(&format!("- Source root: `{}`\n", index.info.source_rel));
    generated.push_str(&format!("- Rust sources: {}\n", index.source_paths.len()));
    if !index.root_declarations.is_empty() {
        generated.push_str(&format!(
            "- Root declarations: `{}`\n",
            index.root_declarations.join("`, `")
        ));
    }
    generated.push_str("\n### Top-Level Modules\n\n");
    if index.top_modules.is_empty() {
        generated.push_str("_No top-level modules detected._\n");
    } else {
        for (name, summary) in &index.top_modules {
            generated.push_str(&format!("- `{name}`"));
            if !summary.second_level.is_empty() {
                generated.push_str(&format!(
                    " — submodules: `{}`",
                    summary
                        .second_level
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("`, `")
                ));
            }
            if !summary.declarations.is_empty() {
                generated.push_str(&format!(
                    "; declarations: `{}`",
                    summary
                        .declarations
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("`, `")
                ));
            }
            if summary.deeper_count > 0 {
                generated.push_str(&format!(
                    "; {} deeper source files aggregated here",
                    summary.deeper_count
                ));
            }
            generated.push('\n');
        }
    }
    generated.push_str("\n### Source Paths\n\n");
    for source in index.source_paths.iter().take(80) {
        generated.push_str(&format!("- `{source}`\n"));
    }
    if index.source_paths.len() > 80 {
        generated.push_str(&format!(
            "- ... {} more source paths omitted\n",
            index.source_paths.len() - 80
        ));
    }

    let body = serialize_frontmatter(
        &fm,
        &format!(
            "# {} Modules\n\nManual notes about this crate can live above or below the generated block.\n",
            index.info.name
        ),
    );
    splice_block(&body, STATIC_BEGIN, STATIC_END, &generated)
}

fn frontmatter(
    title: &str,
    git_rev: Option<&str>,
    source_paths: Vec<String>,
    stale: bool,
) -> MemoryFrontmatter {
    MemoryFrontmatter {
        title: Some(title.to_owned()),
        enabled: Some(true),
        tags: Some(vec![ARCHITECTURE_CATEGORY.to_owned()]),
        managed: Some("static".to_owned()),
        stale: Some(stale),
        git_rev: git_rev.map(str::to_owned),
        source_paths: Some(source_paths),
    }
}

fn write_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    if let Ok(existing) = fs::read_to_string(path) {
        let merged = if existing.contains(STATIC_BEGIN) && existing.contains(STATIC_END) {
            match extract_static_block_inner(content) {
                Some(new_inner) => splice_block(&existing, STATIC_BEGIN, STATIC_END, new_inner),
                None => content.to_owned(),
            }
        } else {
            content.to_owned()
        };
        if existing == merged {
            return Ok(false);
        }
        fs::write(path, merged.as_bytes()).map_err(|e| format!("write {}: {e}", path.display()))?;
        return Ok(true);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    fs::write(path, content.as_bytes()).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(true)
}

fn extract_static_block_inner(content: &str) -> Option<&str> {
    let start = content.find(STATIC_BEGIN)? + STATIC_BEGIN.len();
    let end = content.find(STATIC_END)?;
    if end < start {
        return None;
    }
    Some(content[start..end].strip_prefix('\n').unwrap_or(&content[start..end]))
}

fn mark_stale_frontmatter(workspace_root: &Path, api_paths: &[String]) -> Result<(), String> {
    let memory_root = workspace_root.join(MEMORY_REL);
    for api_path in api_paths {
        let path = memory_root.join(api_path);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let (mut fm, body) = crate::memory::frontmatter::parse_frontmatter(&content);
        fm.stale = Some(true);
        let updated = serialize_frontmatter(&fm, &body);
        if updated != content {
            fs::write(&path, updated.as_bytes())
                .map_err(|e| format!("write stale {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_ws(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("blxcode-arch-{label}-{nonce}"));
        fs::create_dir_all(path.join(".agents/memory")).unwrap();
        path
    }

    #[test]
    fn detects_root_package_and_workspace_member() {
        let ws = temp_ws("crates");
        fs::write(
            ws.join("Cargo.toml"),
            r#"[package]
name = "root-ui"

[workspace]
members = ["backend"]
"#,
        )
        .unwrap();
        fs::create_dir_all(ws.join("src/workbench/agent_panel")).unwrap();
        fs::write(ws.join("src/lib.rs"), "pub mod workbench;\n").unwrap();
        fs::write(ws.join("src/workbench/mod.rs"), "pub mod agent_panel;\n").unwrap();
        fs::write(
            ws.join("src/workbench/agent_panel/mod.rs"),
            "mod timeline;\n",
        )
        .unwrap();
        fs::write(ws.join("src/workbench/agent_panel/timeline.rs"), "").unwrap();
        fs::create_dir_all(ws.join("backend/src")).unwrap();
        fs::write(
            ws.join("backend/Cargo.toml"),
            "[package]\nname = \"backend\"\n",
        )
        .unwrap();
        fs::write(ws.join("backend/src/lib.rs"), "mod api;\n").unwrap();
        fs::write(ws.join("backend/src/api.rs"), "").unwrap();

        let report = rebuild_architecture_at(&ws).unwrap();
        assert_eq!(report.crate_count, 2);
        assert!(ws
            .join(".agents/memory/architecture/modules/root-ui.md")
            .is_file());
        let body =
            fs::read_to_string(ws.join(".agents/memory/architecture/modules/root-ui.md")).unwrap();
        assert!(body.contains("`workbench`"));
        assert!(body.contains("submodules: `agent_panel`"));
        assert!(body.contains("deeper source files aggregated here"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn rebuild_is_idempotent_when_content_unchanged() {
        let ws = temp_ws("idempotent");
        fs::write(
            ws.join("Cargo.toml"),
            "[package]\nname = \"root-ui\"\n[workspace]\nmembers = []\n",
        )
        .unwrap();
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::write(ws.join("src/lib.rs"), "").unwrap();
        let first = rebuild_architecture_at(&ws).unwrap();
        let second = rebuild_architecture_at(&ws).unwrap();
        assert!(first.files_changed > 0);
        assert_eq!(second.files_changed, 0);
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn marker_merge_preserves_manual_text() {
        let ws = temp_ws("manual");
        fs::write(
            ws.join("Cargo.toml"),
            "[package]\nname = \"root-ui\"\n[workspace]\nmembers = []\n",
        )
        .unwrap();
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::write(ws.join("src/lib.rs"), "").unwrap();
        rebuild_architecture_at(&ws).unwrap();
        let arch = ws.join(".agents/memory/ARCHITECTURE.md");
        let existing = fs::read_to_string(&arch).unwrap();
        fs::write(&arch, format!("manual above\n\n{existing}\nmanual below\n")).unwrap();
        rebuild_architecture_at(&ws).unwrap();
        let after = fs::read_to_string(&arch).unwrap();
        assert!(after.contains("manual above"));
        assert!(after.contains("manual below"));
        let _ = fs::remove_dir_all(ws);
    }
}
