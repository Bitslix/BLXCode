//! Auto-maintained plan index inside `.agents/plans/PLANS.md`.
//!
//! The index is a Markdown table with one row per plan. Membership is
//! *derived* from the files on disk, so creating, deleting, or renaming a
//! plan keeps the table in sync without manual edits. The human-curated
//! `Status` and `Description` cells are preserved across syncs. A freshly
//! discovered file gets a default `planned` status and its `# Heading` as the
//! description.
//!
//! Everything in `PLANS.md` *outside* the table — the intro prose, the
//! `## Index` heading — is left untouched. The table is treated as a
//! generated block, mirroring the `ARCHITECTURE.md` pattern.

use crate::agents_layout::PLANS_INDEX;
use crate::plans;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const TABLE_HEADER: &str = "| Status | Plan | Description |";
const TABLE_SEP: &str = "|--------|------|-------------|";
const DEFAULT_STATUS: &str = "planned";

/// A preserved row of the index table, keyed elsewhere by plan path.
#[derive(Clone)]
struct IndexRow {
    status: String,
    description: String,
}

/// Re-derive the index table in `PLANS.md` from the plan files under `root`
/// and write it back if it changed. Best-effort by design: callers treat a
/// failure here as non-fatal so a plan create/delete/rename never fails just
/// because the index could not be rewritten.
pub fn sync_plans_index(root: &Path) -> Result<(), String> {
    sync_inner(root, None)
}

/// Like [`sync_plans_index`], but carry the curated `Status`/`Description`
/// of `old_rel` over to `new_rel` so a rename does not reset them to the
/// defaults.
pub fn sync_plans_index_after_rename(
    root: &Path,
    old_rel: &str,
    new_rel: &str,
) -> Result<(), String> {
    sync_inner(root, Some((old_rel.to_owned(), new_rel.to_owned())))
}

fn sync_inner(root: &Path, rename: Option<(String, String)>) -> Result<(), String> {
    let index_path = root.join(PLANS_INDEX);

    let mut files = Vec::new();
    plans::walk_plan_markdown(root, &mut files);
    let mut entries: Vec<(String, String)> = files
        .iter()
        .filter_map(|abs| {
            let meta = plans::meta_from_plan_file(root, abs)?;
            if meta.is_index {
                return None;
            }
            let body = fs::read_to_string(abs).unwrap_or_default();
            let title = plans::extract_title(&body, &meta.name);
            Some((meta.path, title))
        })
        .collect();
    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let existing = fs::read_to_string(&index_path).unwrap_or_default();
    let rendered = render_index(&existing, &entries, rename.as_ref());
    if rendered != existing {
        fs::write(&index_path, rendered.as_bytes())
            .map_err(|e| format!("write {PLANS_INDEX}: {e}"))?;
    }
    Ok(())
}

/// Rebuild the index table from `entries` (`(path, title)` pairs), reusing
/// the status/description of any surviving row in `existing`. When `rename`
/// is `Some((old, new))`, the curated cells under `old` are re-attached to
/// `new` so a renamed plan keeps its status and description.
fn render_index(
    existing: &str,
    entries: &[(String, String)],
    rename: Option<&(String, String)>,
) -> String {
    let mut preserved = parse_existing_rows(existing);
    if let Some((old, new)) = rename {
        if !preserved.contains_key(new) {
            if let Some(row) = preserved.remove(old) {
                preserved.insert(new.clone(), row);
            }
        }
    }
    let lines: Vec<&str> = existing.lines().collect();

    let header_idx = lines.iter().position(|l| is_table_header(l));
    let (prefix_end, suffix_start) = match header_idx {
        Some(h) => {
            let mut end = h + 1;
            while end < lines.len() && lines[end].trim_start().starts_with('|') {
                end += 1;
            }
            (h, end)
        }
        None => (lines.len(), lines.len()),
    };

    let mut out = String::new();
    for line in &lines[..prefix_end] {
        out.push_str(line);
        out.push('\n');
    }
    if header_idx.is_none() && !out.is_empty() && !out.ends_with("\n\n") {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str(TABLE_HEADER);
    out.push('\n');
    out.push_str(TABLE_SEP);
    out.push('\n');
    for (path, title) in entries {
        let preset = preserved.get(path);
        let status = preset.map(|r| r.status.as_str()).unwrap_or(DEFAULT_STATUS);
        let description = preset
            .map(|r| r.description.clone())
            .unwrap_or_else(|| title.clone());
        out.push_str(&format!(
            "| {status} | [{path}]({path}) | {description} |\n"
        ));
    }

    for line in &lines[suffix_start..] {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn is_table_header(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("| Status") && t.contains("Plan") && t.contains("Description")
}

/// Parse table rows out of an existing body, keyed by the plan path pulled
/// from the `[label](path)` link in the Plan column. Header and separator
/// rows are skipped, as are rows without a link target.
fn parse_existing_rows(body: &str) -> HashMap<String, IndexRow> {
    let mut map = HashMap::new();
    for line in body.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = t.trim_matches('|').split('|').collect();
        if cells.len() < 3 {
            continue;
        }
        let status = cells[0].trim();
        let plan_cell = cells[1].trim();
        let description = cells[2].trim();
        if status.eq_ignore_ascii_case("status") {
            continue;
        }
        if !status.is_empty() && status.chars().all(|c| c == '-' || c == ':') {
            continue; // separator row
        }
        let Some(path) = extract_link_target(plan_cell) else {
            continue;
        };
        let row = IndexRow {
                status: status.to_owned(),
                description: description.to_owned(),
        };
        if let Some(canonical) = legacy_path_to_canonical(&path) {
            map.entry(canonical).or_insert_with(|| row.clone());
        }
        map.insert(path, row);
    }
    map
}

/// Pull `path` out of a `[label](path)` Markdown link cell.
fn extract_link_target(cell: &str) -> Option<String> {
    let open = cell.find("](")? + 2;
    let rest = &cell[open..];
    let close = rest.find(')')?;
    Some(rest[..close].trim().to_owned())
}

fn legacy_path_to_canonical(path: &str) -> Option<String> {
    if path.contains('/') || path.eq_ignore_ascii_case(PLANS_INDEX) {
        return None;
    }
    let p = std::path::Path::new(path);
    let is_md = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("md"))
        .unwrap_or(false);
    if !is_md {
        return None;
    }
    let slug = p.file_stem()?.to_str()?;
    if slug.eq_ignore_ascii_case("README") || slug.is_empty() {
        return None;
    }
    Some(format!("{slug}/plan.md"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> String {
        format!("# Plans\n\nIntro prose.\n\n## Index\n\n{TABLE_HEADER}\n{TABLE_SEP}\n")
    }

    #[test]
    fn adds_new_plan_with_default_status_and_title_description() {
        let body = render_index(
            &seed(),
            &[("alpha/plan.md".into(), "Alpha Plan".into())],
            None,
        );
        assert!(body.contains("| planned | [alpha/plan.md](alpha/plan.md) | Alpha Plan |"));
        assert!(body.contains("# Plans"));
        assert!(body.contains("## Index"));
    }

    #[test]
    fn preserves_status_and_description_for_existing_rows() {
        let mut existing = seed();
        existing.push_str("| done | [alpha.md](alpha.md) | Hand-written summary |\n");
        let body = render_index(
            &existing,
            &[("alpha/plan.md".into(), "Ignored Title".into())],
            None,
        );
        assert!(body.contains("| done | [alpha/plan.md](alpha/plan.md) | Hand-written summary |"));
        assert!(!body.contains("Ignored Title"));
    }

    #[test]
    fn drops_rows_whose_file_no_longer_exists() {
        let mut existing = seed();
        existing.push_str("| done | [gone.md](gone.md) | Old |\n");
        existing.push_str("| planned | [keep/plan.md](keep/plan.md) | Keep |\n");
        let body = render_index(&existing, &[("keep/plan.md".into(), "Keep".into())], None);
        assert!(!body.contains("gone.md"));
        assert!(body.contains("[keep/plan.md](keep/plan.md)"));
    }

    #[test]
    fn preserves_trailing_content_after_table() {
        let mut existing = seed();
        existing.push_str("| done | [a/plan.md](a/plan.md) | A |\n");
        existing.push_str("\n## Notes\n\nKeep me.\n");
        let body = render_index(&existing, &[("a/plan.md".into(), "A".into())], None);
        assert!(body.contains("## Notes"));
        assert!(body.contains("Keep me."));
    }

    #[test]
    fn normalizes_malformed_duplicate_description_cell() {
        let mut existing = seed();
        // A row with an extra pipe-delimited duplicate description.
        existing.push_str("| done | [a/plan.md](a/plan.md) | First | First |\n");
        let body = render_index(&existing, &[("a/plan.md".into(), "A".into())], None);
        assert!(body.contains("| done | [a/plan.md](a/plan.md) | First |"));
        // The duplicate trailing cell is gone.
        assert_eq!(body.matches("[a/plan.md](a/plan.md)").count(), 1);
    }

    #[test]
    fn appends_table_when_no_header_present() {
        let body = render_index(
            "# Plans\n\nNo table yet.\n",
            &[("a/plan.md".into(), "A".into())],
            None,
        );
        assert!(body.contains(TABLE_HEADER));
        assert!(body.contains("| planned | [a/plan.md](a/plan.md) | A |"));
    }

    #[test]
    fn rename_carries_status_and_description_to_new_path() {
        let mut existing = seed();
        existing.push_str("| done | [old.md](old.md) | Curated note |\n");
        let rename = ("old.md".to_owned(), "new/plan.md".to_owned());
        let body = render_index(
            &existing,
            &[("new/plan.md".into(), "Fresh Title".into())],
            Some(&rename),
        );
        assert!(body.contains("| done | [new/plan.md](new/plan.md) | Curated note |"));
        assert!(!body.contains("old.md"));
        assert!(!body.contains("Fresh Title"));
    }

    #[test]
    fn canonical_subdir_links_are_preserved_directly() {
        let mut existing = seed();
        existing.push_str("| active | [feature/plan.md](feature/plan.md) | Curated |\n");
        let body = render_index(
            &existing,
            &[("feature/plan.md".into(), "Ignored Title".into())],
            None,
        );
        assert!(body.contains("| active | [feature/plan.md](feature/plan.md) | Curated |"));
        assert!(!body.contains("Ignored Title"));
    }
}
