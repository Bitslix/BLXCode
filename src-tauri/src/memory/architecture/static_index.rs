use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use crate::agents_layout::{validate_workspace_cwd, MEMORY_REL};
use crate::git_info::head_commit;
use crate::memory::frontmatter::{serialize_frontmatter, MemoryFrontmatter};
use crate::memory::paths::{ARCHITECTURE_CATEGORY, ARCHITECTURE_INDEX};
use crate::memory::types::{ArchitectureLintReport, RebuildReport};
use crate::pointers::splice_block;

use super::common::enumerate_tracked_files;
use super::indexers::run_all_indexers;
use super::state::{now_unix_string, read_state, write_state, ArchitectureState};
use super::unit::ProjectUnit;

pub const STATIC_BEGIN: &str = "<!-- architecture:static:begin -->";
pub const STATIC_END: &str = "<!-- architecture:static:end -->";
const MODULES_DIR: &str = "architecture/modules";
const FLOWS_DIR: &str = "architecture/flows";

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
        .and_then(|s| if s.is_empty() { None } else { Some(s.to_owned()) })
}

fn rebuild_architecture_at(workspace_root: &Path) -> Result<RebuildReport, String> {
    let memory_root = workspace_root.join(MEMORY_REL);
    fs::create_dir_all(memory_root.join(MODULES_DIR))
        .map_err(|e| format!("create {MODULES_DIR}: {e}"))?;
    fs::create_dir_all(memory_root.join(FLOWS_DIR))
        .map_err(|e| format!("create {FLOWS_DIR}: {e}"))?;

    let git_rev = head_commit(workspace_root);
    let tracked = enumerate_tracked_files(workspace_root);
    let (units, warnings) = run_all_indexers(workspace_root, &tracked);

    let mut files_changed = 0u32;
    let mut generated_paths = Vec::new();
    for unit in &units {
        let rel = format!("{MODULES_DIR}/{}.md", unit.slug());
        let rendered = render_module_note(unit, git_rev.as_deref());
        if write_if_changed(&memory_root.join(&rel), &rendered)? {
            files_changed += 1;
        }
        generated_paths.push(rel);
    }

    let index_body =
        render_architecture_index(&units, &generated_paths, &warnings, git_rev.as_deref());
    if write_if_changed(&memory_root.join(ARCHITECTURE_INDEX), &index_body)? {
        files_changed += 1;
    }

    let module_count = units
        .iter()
        .map(|unit| unit.top_modules.len() as u32)
        .sum::<u32>();
    let unit_count = units.len() as u32;
    let mut kinds: Vec<String> = units.iter().map(|u| u.kind.as_str().to_owned()).collect();
    kinds.sort();
    kinds.dedup();

    write_state(
        workspace_root,
        &ArchitectureState {
            git_rev: git_rev.clone(),
            generated_at: now_unix_string(),
            crate_count: unit_count,
            module_count,
        },
    )?;

    Ok(RebuildReport {
        git_rev,
        crate_count: unit_count,
        unit_count,
        module_count,
        files_changed,
        kinds,
        warnings,
        generated_paths,
    })
}

fn render_architecture_index(
    units: &[ProjectUnit],
    generated_paths: &[String],
    warnings: &[String],
    git_rev: Option<&str>,
) -> String {
    let source_paths: Vec<String> = units
        .iter()
        .flat_map(|unit| unit.source_paths.iter().take(8).cloned())
        .collect();
    let fm = frontmatter("Architecture", None, git_rev, source_paths, false);
    let mut generated = String::new();
    generated.push_str("## Generated\n\n");
    generated.push_str("| Unit | Kind | Root | Map |\n");
    generated.push_str("|---|---|---|---|\n");
    for (unit, path) in units.iter().zip(generated_paths) {
        let root = if unit.root_rel.is_empty() {
            ".".to_owned()
        } else {
            format!("`{}`", unit.root_rel)
        };
        generated.push_str(&format!(
            "| `{}` | {} | {} | [[{}|{}]] |\n",
            unit.name,
            unit.kind.as_str(),
            root,
            path,
            unit.slug()
        ));
    }
    generated.push('\n');

    let mut kinds: Vec<&str> = units.iter().map(|u| u.kind.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    generated.push_str("### Counts\n\n");
    generated.push_str(&format!("- Units: {}\n", units.len()));
    generated.push_str(&format!("- Kinds: {}\n", kinds.join(", ")));
    generated.push_str(&format!(
        "- Top-level modules: {}\n",
        units.iter().map(|u| u.top_modules.len()).sum::<usize>()
    ));
    if let Some(rev) = git_rev {
        generated.push_str(&format!("- Git revision: `{rev}`\n"));
    }
    if !warnings.is_empty() {
        generated.push_str("\n### Warnings\n\n");
        for warning in warnings {
            generated.push_str(&format!("- {warning}\n"));
        }
    }

    let mut body = serialize_frontmatter(
        &fm,
        "# Architecture\n\nThis index is maintained by BLXCode's architecture map harness.\n\n## Manual\n\nAdd curated overview notes here. The generated block below is refreshed by `memory_rebuild_architecture`.\n",
    );
    body = splice_block(&body, STATIC_BEGIN, STATIC_END, &generated);
    body
}

fn render_module_note(unit: &ProjectUnit, git_rev: Option<&str>) -> String {
    let fm = frontmatter(
        &format!("{} ({})", unit.name, unit.kind.as_str()),
        Some(unit.kind.as_str()),
        git_rev,
        unit.source_paths.clone(),
        false,
    );
    let mut generated = String::new();
    generated.push_str(&format!("## `{}`\n\n", unit.name));
    generated.push_str(&format!("- Kind: `{}`\n", unit.kind.as_str()));
    if let Some(manifest) = &unit.manifest_rel {
        generated.push_str(&format!("- Manifest: `{manifest}`\n"));
    }
    let root = if unit.root_rel.is_empty() {
        "."
    } else {
        unit.root_rel.as_str()
    };
    generated.push_str(&format!("- Root: `{root}`\n"));
    if let Some(src) = &unit.source_root_rel {
        generated.push_str(&format!("- Source root: `{src}`\n"));
    }
    generated.push_str(&format!("- Source files: {}\n", unit.source_paths.len()));
    if !unit.root_declarations.is_empty() {
        let mut decls = unit.root_declarations.clone();
        decls.sort();
        decls.dedup();
        generated.push_str(&format!("- Root declarations: `{}`\n", decls.join("`, `")));
    }
    for note in &unit.extra_notes {
        generated.push_str(&format!("- {note}\n"));
    }

    generated.push_str("\n### Top-Level Modules\n\n");
    if unit.top_modules.is_empty() {
        generated.push_str("_No top-level modules detected._\n");
    } else {
        for (name, summary) in &unit.top_modules {
            generated.push_str(&format!("- `{name}`"));
            if summary.file_count > 0 {
                generated.push_str(&format!(" ({} files)", summary.file_count));
            }
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
    for source in unit.source_paths.iter().take(80) {
        generated.push_str(&format!("- `{source}`\n"));
    }
    if unit.source_paths.len() > 80 {
        generated.push_str(&format!(
            "- ... {} more source paths omitted\n",
            unit.source_paths.len() - 80
        ));
    }

    let body = serialize_frontmatter(
        &fm,
        &format!(
            "# {} ({})\n\nManual notes about this unit can live above or below the generated block.\n",
            unit.name,
            unit.kind.as_str()
        ),
    );
    splice_block(&body, STATIC_BEGIN, STATIC_END, &generated)
}

fn frontmatter(
    title: &str,
    kind: Option<&str>,
    git_rev: Option<&str>,
    source_paths: Vec<String>,
    stale: bool,
) -> MemoryFrontmatter {
    MemoryFrontmatter {
        title: Some(title.to_owned()),
        enabled: Some(true),
        tags: Some(vec![ARCHITECTURE_CATEGORY.to_owned()]),
        managed: Some("static".to_owned()),
        kind: kind.map(str::to_owned),
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
    Some(
        content[start..end]
            .strip_prefix('\n')
            .unwrap_or(&content[start..end]),
    )
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
        fs::write(ws.join("src/workbench/agent_panel/mod.rs"), "mod timeline;\n").unwrap();
        fs::write(ws.join("src/workbench/agent_panel/timeline.rs"), "").unwrap();
        fs::create_dir_all(ws.join("backend/src")).unwrap();
        fs::write(ws.join("backend/Cargo.toml"), "[package]\nname = \"backend\"\n").unwrap();
        fs::write(ws.join("backend/src/lib.rs"), "mod api;\n").unwrap();
        fs::write(ws.join("backend/src/api.rs"), "").unwrap();

        let report = rebuild_architecture_at(&ws).unwrap();
        assert_eq!(report.unit_count, 2);
        assert_eq!(report.crate_count, 2);
        assert_eq!(report.kinds, vec!["rust".to_owned()]);
        assert!(report.warnings.is_empty());
        assert!(ws
            .join(".agents/memory/architecture/modules/rust-root-ui.md")
            .is_file());
        let body =
            fs::read_to_string(ws.join(".agents/memory/architecture/modules/rust-root-ui.md"))
                .unwrap();
        assert!(body.contains("kind: rust"));
        assert!(body.contains("`workbench`"));
        assert!(body.contains("submodules: `agent_panel`"));
        assert!(body.contains("deeper source files aggregated here"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn node_only_workspace_without_cargo() {
        let ws = temp_ws("node");
        fs::write(ws.join("package.json"), "{\n  \"name\": \"blxcode-eb\"\n}\n").unwrap();
        fs::create_dir_all(ws.join("src/views")).unwrap();
        fs::create_dir_all(ws.join("src/bun")).unwrap();
        fs::write(ws.join("src/index.ts"), "export {};\n").unwrap();
        fs::write(ws.join("src/views/app.tsx"), "export {};\n").unwrap();
        fs::write(ws.join("src/bun/main.ts"), "export {};\n").unwrap();

        let report = rebuild_architecture_at(&ws).unwrap();
        assert_eq!(report.unit_count, 1);
        assert_eq!(report.kinds, vec!["node".to_owned()]);
        let note = ws.join(".agents/memory/architecture/modules/node-blxcode-eb.md");
        assert!(note.is_file());
        let body = fs::read_to_string(&note).unwrap();
        assert!(body.contains("kind: node"));
        assert!(body.contains("`views`"));
        assert!(body.contains("`bun`"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn no_manifest_falls_back_to_generic() {
        let ws = temp_ws("nomanifest");
        fs::create_dir_all(ws.join("scripts")).unwrap();
        fs::write(ws.join("README.md"), "# hi\n").unwrap();
        fs::write(ws.join("scripts/build.sh"), "echo hi\n").unwrap();

        let report = rebuild_architecture_at(&ws).unwrap();
        assert_eq!(report.unit_count, 1);
        assert_eq!(report.kinds, vec!["generic".to_owned()]);
        let slug = format!(
            "generic-{}",
            super::super::unit::sanitize_slug(ws.file_name().unwrap().to_str().unwrap())
        );
        assert!(ws
            .join(format!(".agents/memory/architecture/modules/{slug}.md"))
            .is_file());
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn makefile_c_project_uses_make_fallback() {
        let ws = temp_ws("makec");
        fs::write(ws.join("Makefile"), "all:\n\tcc -o app src/main.c\n").unwrap();
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::create_dir_all(ws.join("include")).unwrap();
        fs::write(ws.join("src/main.c"), "int main(){return 0;}\n").unwrap();
        fs::write(ws.join("src/util.c"), "").unwrap();
        fs::write(ws.join("include/util.h"), "").unwrap();

        let report = rebuild_architecture_at(&ws).unwrap();
        assert_eq!(report.unit_count, 1);
        assert_eq!(report.kinds, vec!["make".to_owned()]);
        let slug = format!(
            "make-{}",
            super::super::unit::sanitize_slug(ws.file_name().unwrap().to_str().unwrap())
        );
        let note = ws.join(format!(".agents/memory/architecture/modules/{slug}.md"));
        assert!(note.is_file());
        let body = fs::read_to_string(&note).unwrap();
        assert!(body.contains("kind: make"));
        assert!(body.contains("Languages: C"));
        assert!(body.contains("`src`"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn unknown_language_generic_names_it() {
        let ws = temp_ws("ocaml");
        fs::create_dir_all(ws.join("lib")).unwrap();
        fs::write(ws.join("lib/main.ml"), "let () = ()\n").unwrap();
        fs::write(ws.join("lib/types.mli"), "").unwrap();

        let report = rebuild_architecture_at(&ws).unwrap();
        assert_eq!(report.unit_count, 1);
        assert_eq!(report.kinds, vec!["generic".to_owned()]);
        let slug = format!(
            "generic-{}",
            super::super::unit::sanitize_slug(ws.file_name().unwrap().to_str().unwrap())
        );
        let body = fs::read_to_string(
            ws.join(format!(".agents/memory/architecture/modules/{slug}.md")),
        )
        .unwrap();
        assert!(body.contains("Languages: OCaml"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn mixed_rust_and_node() {
        let ws = temp_ws("mixed");
        fs::write(ws.join("Cargo.toml"), "[package]\nname = \"core\"\n").unwrap();
        fs::create_dir_all(ws.join("src")).unwrap();
        fs::write(ws.join("src/lib.rs"), "pub mod thing;\n").unwrap();
        fs::write(ws.join("src/thing.rs"), "").unwrap();
        fs::write(ws.join("package.json"), "{ \"name\": \"frontend\" }\n").unwrap();
        fs::create_dir_all(ws.join("src/ui")).unwrap();
        fs::write(ws.join("src/ui/app.ts"), "export {};\n").unwrap();

        let report = rebuild_architecture_at(&ws).unwrap();
        assert_eq!(report.unit_count, 2);
        assert_eq!(report.kinds, vec!["node".to_owned(), "rust".to_owned()]);
        assert!(ws
            .join(".agents/memory/architecture/modules/rust-core.md")
            .is_file());
        assert!(ws
            .join(".agents/memory/architecture/modules/node-frontend.md")
            .is_file());
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
