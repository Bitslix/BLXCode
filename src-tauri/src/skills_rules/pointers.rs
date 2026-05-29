//! Rules-scope pointer block (`<!-- blxcode-rules:begin/end -->`).
//!
//! Thin wrapper over [`crate::pointers`] that supplies the rules-specific
//! marker pair and body. The body intentionally stays a short pointer
//! (path only) — no enumeration of individual rules — because the rule
//! list changes often and we don't want CLAUDE.md to churn on every
//! toggle.

use std::path::Path;

use crate::pointers::{
    install_pointer_block, pointer_status as generic_pointer_status, uninstall_pointer_block,
    Markers, PointerResult,
};
use crate::skills_rules::store::RULES_REL;

const RULES_POINTER_BEGIN: &str = "<!-- blxcode-rules:begin -->";
const RULES_POINTER_END: &str = "<!-- blxcode-rules:end -->";
const RULES_POINTER_BEGIN_CURSOR: &str = "# blxcode-rules:begin";
const RULES_POINTER_END_CURSOR: &str = "# blxcode-rules:end";

const RULES_MARKERS: Markers<'static> = Markers {
    html: (RULES_POINTER_BEGIN, RULES_POINTER_END),
    cursor: (RULES_POINTER_BEGIN_CURSOR, RULES_POINTER_END_CURSOR),
};

fn rules_body(workspace_cwd: &Path, cursor_style: bool) -> String {
    let rules_dir = workspace_cwd.join(RULES_REL);
    let mut s = String::new();
    if cursor_style {
        s.push_str("blxcode tracks project rules at the path below.\n");
    } else {
        s.push_str("## blxcode workspace rules\n\n");
        s.push_str(
            "This workspace ships project rules under the directory below. \
Treat each `rule-*.md` as binding before changing code — read them at the \
start of every task and follow the constraints exactly.\n\n",
        );
    }
    s.push_str(&format!("Rules directory: `{}`\n", rules_dir.display()));
    s.push_str(&format!(
        "List rules: `ls {}/rule-*.md` and read each file before editing.\n",
        rules_dir.display()
    ));
    s.push('\n');
    s
}

pub fn rules_install_pointers_impl(
    workspace_cwd: &str,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    install_pointer_block(workspace_cwd, agents, &RULES_MARKERS, rules_body)
}

pub fn rules_uninstall_pointers_impl(
    workspace_cwd: &str,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    uninstall_pointer_block(workspace_cwd, agents, &RULES_MARKERS)
}

pub fn rules_pointer_status_impl(workspace_cwd: &str) -> Result<Vec<PointerResult>, String> {
    generic_pointer_status(workspace_cwd, &RULES_MARKERS)
}

#[cfg(test)]
mod tests {
    use super::*;
    // The Tauri commands `memory_install_pointers` / `memory_pointer_status`
    // are the public entry points (impl modules are private). Calling
    // them keeps the coexistence test honest to the user-visible path.
    use crate::memory::{memory_install_pointers, memory_pointer_status};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_ws(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("blxcode-rules-pointers-{label}-{nonce}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn install_writes_rules_block() {
        let ws = temp_ws("install");
        fs::write(ws.join("CLAUDE.md"), "").unwrap();
        let result =
            rules_install_pointers_impl(ws.to_str().unwrap(), vec!["claude".into()]).unwrap();
        assert!(result[0].installed, "claude install failed: {:?}", result);
        let body = fs::read_to_string(ws.join("CLAUDE.md")).unwrap();
        assert!(body.contains(RULES_POINTER_BEGIN));
        assert!(body.contains(RULES_POINTER_END));
        assert!(body.contains("blxcode workspace rules"));
        assert!(body.contains(".agents/rules"));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn rules_block_coexists_with_memory_block() {
        let ws = temp_ws("coexist");
        fs::write(ws.join("CLAUDE.md"), "").unwrap();
        // memory_install_pointers_impl expects `.agents/memory/` to exist
        // when it tries to collect notes; create an empty memory dir so
        // it doesn't bail.
        fs::create_dir_all(ws.join(".agents").join("memory")).unwrap();

        memory_install_pointers(ws.to_string_lossy().into_owned(), vec!["claude".into()]).unwrap();
        rules_install_pointers_impl(ws.to_str().unwrap(), vec!["claude".into()]).unwrap();

        let body = fs::read_to_string(ws.join("CLAUDE.md")).unwrap();
        assert!(body.contains("<!-- blxcode-memory:begin -->"));
        assert!(body.contains("<!-- blxcode-memory:end -->"));
        assert!(body.contains(RULES_POINTER_BEGIN));
        assert!(body.contains(RULES_POINTER_END));

        // Uninstall only rules — memory survives.
        rules_uninstall_pointers_impl(ws.to_str().unwrap(), vec!["claude".into()]).unwrap();
        let after = fs::read_to_string(ws.join("CLAUDE.md")).unwrap();
        assert!(after.contains("<!-- blxcode-memory:begin -->"));
        assert!(!after.contains(RULES_POINTER_BEGIN));

        // Memory status still reports it as installed.
        let mem_status = memory_pointer_status(ws.to_string_lossy().into_owned()).unwrap();
        assert!(mem_status
            .iter()
            .any(|r| r.agent == "claude" && r.installed));
        let _ = fs::remove_dir_all(ws);
    }

    #[test]
    fn status_reports_uninstalled_when_no_block() {
        let ws = temp_ws("status");
        fs::write(ws.join("CLAUDE.md"), "hello\n").unwrap();
        let status = rules_pointer_status_impl(ws.to_str().unwrap()).unwrap();
        let claude = status.iter().find(|r| r.agent == "claude").unwrap();
        assert!(!claude.installed);
        let _ = fs::remove_dir_all(ws);
    }
}
