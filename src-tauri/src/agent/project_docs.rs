//! Reads repo-level agent instruction files (CLAUDE.md, AGENTS.md,
//! GEMINI.md) from the workspace root and bundles them into a single
//! `<project-docs>` block that the session orchestrator prepends to the
//! user's first prompt of every session.
//!
//! Designed for *idempotent first-turn priming*: the orchestrator calls
//! [`render_first_turn_block`] only when the conversation history is
//! empty, so external-agent config files become authoritative context
//! exactly once per session, mirroring how Claude Code / Codex / Gemini
//! treat them.

use std::fs;
use std::path::Path;

/// Files we look for in the workspace root, in priority order. We render
/// them all (in this order) when present; agents rarely have more than
/// one of these and concatenating is cheaper than picking favourites.
const CANDIDATE_FILES: &[&str] = &["CLAUDE.md", "AGENTS.md", "GEMINI.md"];

/// Shared preload cap across repo docs and the generated architecture map.
const PRELOAD_BUDGET_BYTES: usize = 12 * 1024;

/// Returns `Some(block)` when at least one candidate file exists under
/// `workspace_root` and is non-empty after trim. Returns `None` for an
/// unset / empty / unreadable workspace, or when every candidate is
/// missing.
pub fn render_first_turn_block(workspace_root: Option<&str>) -> Option<String> {
    let raw = workspace_root?.trim();
    if raw.is_empty() {
        return None;
    }
    let ws = Path::new(raw);
    if !ws.is_absolute() || !ws.is_dir() {
        return None;
    }

    let mut remaining = PRELOAD_BUDGET_BYTES;
    let mut sections: Vec<(String, String)> = Vec::new();
    for name in CANDIDATE_FILES {
        if remaining == 0 {
            break;
        }
        let path = ws.join(name);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let trimmed = content.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (body, used) = take_budgeted(
            trimmed,
            remaining,
            "[... truncated by BLXCode project-docs preload - open the file for full text ...]",
        )?;
        remaining = remaining.saturating_sub(used);
        sections.push(((*name).to_owned(), body));
    }

    let architecture = if remaining > 0 {
        crate::memory::architecture::generated_section_from_architecture_index(ws).and_then(
            |generated| {
                take_budgeted(
                    generated.trim(),
                    remaining,
                    "[... truncated by BLXCode architecture preload - read ARCHITECTURE.md for full text ...]",
                )
                .map(|(body, _)| body)
            },
        )
    } else {
        None
    };

    if sections.is_empty() && architecture.is_none() {
        return None;
    }

    let mut block = String::new();
    if !sections.is_empty() {
        block.push_str("<project-docs>\n");
        block.push_str(
            "The following repo-level agent instruction files were detected in this \
workspace root. Treat their content as authoritative project policy on the \
same level as active rules. They are injected once per session (this first \
turn) — rely on conversation history for subsequent turns.\n\n",
        );
        for (i, (name, body)) in sections.iter().enumerate() {
            if i > 0 {
                block.push('\n');
            }
            block.push_str(&format!("--- {name} ---\n"));
            block.push_str(body);
            if !body.ends_with('\n') {
                block.push('\n');
            }
        }
        block.push_str("</project-docs>\n\n");
    }
    if let Some(architecture) = architecture {
        block.push_str("<memory-architecture>\n");
        block.push_str(
            "Generated architecture map excerpt. Prefer reading ARCHITECTURE.md \
and architecture/modules/*.md before broad filesystem scans.\n\n",
        );
        block.push_str(&architecture);
        if !architecture.ends_with('\n') {
            block.push('\n');
        }
        block.push_str("</memory-architecture>\n\n");
    }
    Some(block)
}

fn take_budgeted(input: &str, budget: usize, marker: &str) -> Option<(String, usize)> {
    if budget == 0 || input.trim().is_empty() {
        return None;
    }
    if input.len() <= budget {
        return Some((input.to_owned(), input.len()));
    }
    let cut = input
        .char_indices()
        .take_while(|(i, _)| *i < budget)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(budget);
    let mut body = input[..cut].to_owned();
    body.push_str("\n\n");
    body.push_str(marker);
    Some((body, budget))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_ws(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("blxcode-project-docs-{label}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn none_when_workspace_missing() {
        assert!(render_first_turn_block(None).is_none());
        assert!(render_first_turn_block(Some("")).is_none());
        assert!(render_first_turn_block(Some("   ")).is_none());
        assert!(render_first_turn_block(Some("/nonexistent-blxcode-test")).is_none());
    }

    #[test]
    fn none_when_no_candidates_present() {
        let ws = temp_ws("no-files");
        assert!(render_first_turn_block(Some(ws.to_str().unwrap())).is_none());
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn includes_claude_md_when_present() {
        let ws = temp_ws("claude");
        fs::write(ws.join("CLAUDE.md"), "# Claude policy\nUse 2-space indent.").unwrap();
        let block = render_first_turn_block(Some(ws.to_str().unwrap())).unwrap();
        assert!(block.starts_with("<project-docs>\n"));
        assert!(block.contains("--- CLAUDE.md ---"));
        assert!(block.contains("Use 2-space indent."));
        assert!(block.ends_with("</project-docs>\n\n"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn concatenates_all_candidates_in_priority_order() {
        let ws = temp_ws("multi");
        fs::write(ws.join("AGENTS.md"), "agents body").unwrap();
        fs::write(ws.join("CLAUDE.md"), "claude body").unwrap();
        fs::write(ws.join("GEMINI.md"), "gemini body").unwrap();
        let block = render_first_turn_block(Some(ws.to_str().unwrap())).unwrap();
        let claude_pos = block.find("CLAUDE.md").unwrap();
        let agents_pos = block.find("AGENTS.md").unwrap();
        let gemini_pos = block.find("GEMINI.md").unwrap();
        assert!(claude_pos < agents_pos);
        assert!(agents_pos < gemini_pos);
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn truncates_files_exceeding_shared_budget() {
        let ws = temp_ws("trunc");
        let huge = "a".repeat(PRELOAD_BUDGET_BYTES * 2);
        fs::write(ws.join("CLAUDE.md"), &huge).unwrap();
        let block = render_first_turn_block(Some(ws.to_str().unwrap())).unwrap();
        assert!(block.contains("truncated by BLXCode project-docs preload"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn skips_empty_files() {
        let ws = temp_ws("empty");
        fs::write(ws.join("CLAUDE.md"), "   \n\n").unwrap();
        fs::write(ws.join("AGENTS.md"), "real body").unwrap();
        let block = render_first_turn_block(Some(ws.to_str().unwrap())).unwrap();
        assert!(!block.contains("CLAUDE.md"));
        assert!(block.contains("AGENTS.md"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn includes_architecture_after_project_docs_when_budget_remains() {
        let ws = temp_ws("architecture");
        fs::create_dir_all(ws.join(".agents/memory")).unwrap();
        fs::write(ws.join("CLAUDE.md"), "short policy").unwrap();
        fs::write(
            ws.join(".agents/memory/ARCHITECTURE.md"),
            format!(
                "# Architecture\n\n{}\n## Generated\n\n- crate map\n{}\n",
                crate::memory::architecture::STATIC_BEGIN,
                crate::memory::architecture::STATIC_END
            ),
        )
        .unwrap();
        let block = render_first_turn_block(Some(ws.to_str().unwrap())).unwrap();
        assert!(block.contains("<project-docs>"));
        assert!(block.contains("<memory-architecture>"));
        assert!(block.contains("- crate map"));
        let _ = fs::remove_dir_all(ws);
    }
}
