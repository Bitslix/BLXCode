//! Tool group membership and filtered catalogs for coordinator vs subagent runs.

use crate::agent::tools::{registry, ToolDef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolGroup {
    EnvironmentRead,
    WorkspaceRead,
    DiffRead,
    GitRead,
    GitWrite,
    ShellRead,
    ShellWrite,
    WebRead,
    MemoryRead,
    MemoryWrite,
    PlansRead,
    PlansWrite,
    TasksRead,
    TasksWrite,
    RulesSkillsRead,
    RulesSkillsWrite,
    CoordinatorHarness,
    CoordinatorMeta,
    SubagentsRun,
    SubagentSubmit,
}

impl ToolGroup {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "environment_read" => Some(Self::EnvironmentRead),
            "workspace_read" => Some(Self::WorkspaceRead),
            "diff_read" => Some(Self::DiffRead),
            "git_read" => Some(Self::GitRead),
            "git_write" => Some(Self::GitWrite),
            "shell_read" => Some(Self::ShellRead),
            "shell_write" => Some(Self::ShellWrite),
            "web_read" => Some(Self::WebRead),
            "memory_read" => Some(Self::MemoryRead),
            "memory_write" => Some(Self::MemoryWrite),
            "plans_read" => Some(Self::PlansRead),
            "plans_write" => Some(Self::PlansWrite),
            "tasks_read" => Some(Self::TasksRead),
            "tasks_write" => Some(Self::TasksWrite),
            "rules_skills_read" => Some(Self::RulesSkillsRead),
            "rules_skills_write" => Some(Self::RulesSkillsWrite),
            _ => None,
        }
    }

    pub fn tool_names(self) -> &'static [&'static str] {
        match self {
            Self::EnvironmentRead => &["environment_detect"],
            Self::WorkspaceRead => &[
                "list_workspace_files",
                "read_workspace_file",
                "workspace_search",
            ],
            Self::DiffRead => &[
                "workspace_git_status",
                "workspace_diff",
                "git_status",
                "git_diff",
                "git_show",
            ],
            Self::GitRead => &[
                "git_status",
                "git_diff",
                "git_log",
                "git_show",
                "git_branch_info",
                "git_ls_files",
            ],
            Self::GitWrite => &["git_apply_patch", "git_add", "git_commit"],
            Self::ShellRead => &["shell_exec"],
            Self::ShellWrite => &["shell_exec"],
            Self::WebRead => &["web_search", "web_fetch"],
            Self::MemoryRead => &[
                "memory_list",
                "memory_read",
                "memory_search",
                "memory_graph",
                "memory_backlinks",
                "memory_category_list",
                "memory_context_list",
            ],
            Self::MemoryWrite => &[
                "memory_create",
                "memory_write",
                "memory_delete",
                "memory_rename",
                "memory_category_update",
                "memory_context_attach",
                "memory_context_detach",
            ],
            Self::PlansRead => &[
                "plan_list",
                "plan_read",
                "plan_load",
                "plan_context_list",
            ],
            Self::PlansWrite => &[
                "plan_create",
                "plan_write",
                "plan_delete",
                "plan_rename",
                "plan_sync_from_tasks",
                "plan_context_attach",
                "plan_context_detach",
            ],
            Self::TasksRead => &["task_list", "task_get"],
            Self::TasksWrite => &[
                "task_create",
                "task_update",
                "task_delete",
                "task_reorder",
            ],
            Self::RulesSkillsRead => &["rules_list", "rules_read", "skills_list", "skills_read"],
            Self::RulesSkillsWrite => &[
                "rules_write",
                "rules_set_enabled",
                "rules_remove",
                "skills_write",
                "skills_set_enabled",
                "skills_remove",
                "skills_install",
            ],
            Self::CoordinatorHarness => &[
                "harness.create_workspace",
                "harness.open_terminal",
                "harness.list_terminals",
                "harness.send_terminal_keys",
                "harness.send_agent_context",
                "harness.read_terminal_output",
            ],
            Self::CoordinatorMeta => &["list_tools"],
            Self::SubagentsRun => &["subagents.run"],
            Self::SubagentSubmit => &["submit_result"],
        }
    }
}

/// All groups available to the main coordinator (excludes subagent-only submit).
pub fn coordinator_groups(web_enabled: bool) -> Vec<ToolGroup> {
    let mut g = vec![
        ToolGroup::EnvironmentRead,
        ToolGroup::WorkspaceRead,
        ToolGroup::DiffRead,
        ToolGroup::GitRead,
        ToolGroup::GitWrite,
        ToolGroup::ShellRead,
        ToolGroup::ShellWrite,
        ToolGroup::MemoryRead,
        ToolGroup::MemoryWrite,
        ToolGroup::PlansRead,
        ToolGroup::PlansWrite,
        ToolGroup::TasksRead,
        ToolGroup::TasksWrite,
        ToolGroup::RulesSkillsRead,
        ToolGroup::RulesSkillsWrite,
        ToolGroup::CoordinatorHarness,
        ToolGroup::CoordinatorMeta,
        ToolGroup::SubagentsRun,
    ];
    if web_enabled {
        g.push(ToolGroup::WebRead);
    }
    g
}

pub fn parse_allowed_groups(names: &[String]) -> Vec<ToolGroup> {
    names
        .iter()
        .filter_map(|s| ToolGroup::parse(s))
        .collect()
}

/// Like [`parse_allowed_groups`] but also returns the strings that failed to
/// parse, so the caller can warn the user (or the model) about typos
/// instead of silently producing an empty toolset.
#[must_use]
pub fn parse_allowed_groups_strict(names: &[String]) -> (Vec<ToolGroup>, Vec<String>) {
    let mut ok = Vec::with_capacity(names.len());
    let mut bad = Vec::new();
    for s in names {
        match ToolGroup::parse(s) {
            Some(g) => ok.push(g),
            None => bad.push(s.clone()),
        }
    }
    (ok, bad)
}

fn allowed_names(groups: &[ToolGroup], web_enabled: bool) -> HashSet<&'static str> {
    let mut set = HashSet::new();
    for g in groups {
        if matches!(g, ToolGroup::WebRead) && !web_enabled {
            continue;
        }
        for n in g.tool_names() {
            set.insert(*n);
        }
    }
    set
}

pub fn registry_filtered(groups: &[ToolGroup], web_enabled: bool) -> Vec<ToolDef> {
    let allowed = allowed_names(groups, web_enabled);
    registry()
        .into_iter()
        .filter(|t| allowed.contains(t.name))
        .collect()
}

/// Sanitize a tool name for OpenAI/Azure: Azure rejects names that don't match
/// `^[a-zA-Z0-9_-]+$`, so we replace dots with underscores. The reverse
/// mapping is performed by [`openai_tool_name_to_internal`] on the inbound
/// `tool_calls` so internal dispatch keeps the dotted names.
#[must_use]
pub fn sanitize_openai_tool_name(name: &str) -> String {
    name.replace('.', "_")
}

/// Look up the internal (dotted) tool name from a sanitized name returned by
/// the OpenAI-compatible provider. Falls back to the input when there is no
/// dotted tool whose sanitized form matches.
#[must_use]
pub fn openai_tool_name_to_internal(sanitized: &str) -> String {
    if !sanitized.contains('_') {
        return sanitized.to_string();
    }
    for t in registry() {
        if t.name.contains('.') && sanitize_openai_tool_name(t.name) == sanitized {
            return t.name.to_string();
        }
    }
    sanitized.to_string()
}

pub fn render_for_openai_filtered(groups: &[ToolGroup], web_enabled: bool) -> Value {
    let items: Vec<Value> = registry_filtered(groups, web_enabled)
        .into_iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": sanitize_openai_tool_name(t.name),
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        })
        .collect();
    Value::Array(items)
}

pub fn render_for_anthropic_filtered(groups: &[ToolGroup], web_enabled: bool) -> Value {
    let items: Vec<Value> = registry_filtered(groups, web_enabled)
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters,
            })
        })
        .collect();
    Value::Array(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinator_includes_subagents_run_not_submit() {
        let reg = registry_filtered(&coordinator_groups(true), true);
        let names: HashSet<_> = reg.iter().map(|t| t.name).collect();
        assert!(names.contains("subagents.run"));
        assert!(!names.contains("submit_result"));
    }

    #[test]
    fn subagent_catalog_has_submit_not_run() {
        let groups = [
            ToolGroup::EnvironmentRead,
            ToolGroup::WorkspaceRead,
            ToolGroup::SubagentSubmit,
        ];
        let reg = registry_filtered(&groups, false);
        let names: HashSet<_> = reg.iter().map(|t| t.name).collect();
        assert!(names.contains("submit_result"));
        assert!(!names.contains("subagents.run"));
    }

    #[test]
    fn parse_allowed_groups_strict_separates_known_from_unknown() {
        let input = vec![
            "workspace_read".to_string(),
            "file_access".to_string(),     // bogus
            "git_read".to_string(),
            "shell".to_string(),           // bogus (correct names are shell_read/shell_write)
        ];
        let (ok, bad) = parse_allowed_groups_strict(&input);
        assert_eq!(ok, vec![ToolGroup::WorkspaceRead, ToolGroup::GitRead]);
        assert_eq!(bad, vec!["file_access".to_string(), "shell".to_string()]);
    }

    #[test]
    fn no_subagent_reachable_group_contains_dotted_tool_names() {
        // Invariant: subagent toolsets never contain dotted tool names
        // (`subagents.run`, `harness.*`). If this ever flips, the Azure
        // sanitization in `render_for_openai_filtered` would start renaming
        // tools inside the subagent catalog and we'd need to add a matching
        // reverse-map in the subagent loop. Currently the bidirectional
        // mapping only exists in the coordinator path.
        let all_coordinator_controllable_strings = [
            "environment_read",
            "workspace_read",
            "diff_read",
            "git_read",
            "git_write",
            "shell_read",
            "shell_write",
            "web_read",
            "memory_read",
            "memory_write",
            "plans_read",
            "plans_write",
            "tasks_read",
            "tasks_write",
            "rules_skills_read",
            "rules_skills_write",
        ];
        for s in all_coordinator_controllable_strings {
            let g = ToolGroup::parse(s).expect("known group");
            for tool in g.tool_names() {
                assert!(
                    !tool.contains('.'),
                    "group {s:?} exposes dotted tool {tool:?} — would conflict with Azure sanitizer"
                );
            }
        }
        // SubagentSubmit is force-pushed onto every subagent's group list.
        for tool in ToolGroup::SubagentSubmit.tool_names() {
            assert!(!tool.contains('.'), "SubagentSubmit tool {tool:?} must be dotless");
        }
    }

    #[test]
    fn parse_allowed_groups_strict_returns_empty_when_all_unknown() {
        // The pathological case the subagents fix is built to catch: a
        // coordinator that invents toolgroup names. Parser returns empty
        // ok-list so the caller can detect the situation and fall back.
        let input = vec!["files".into(), "file_system".into(), "workspace".into()];
        let (ok, bad) = parse_allowed_groups_strict(&input);
        assert!(ok.is_empty());
        assert_eq!(bad.len(), 3);
    }
}
