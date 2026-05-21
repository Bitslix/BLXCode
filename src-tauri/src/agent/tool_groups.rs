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

pub fn render_for_openai_filtered(groups: &[ToolGroup], web_enabled: bool) -> Value {
    let items: Vec<Value> = registry_filtered(groups, web_enabled)
        .into_iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
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
}
