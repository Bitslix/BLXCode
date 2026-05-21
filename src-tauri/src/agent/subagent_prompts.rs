//! Role profiles and subagent system prompts.

use crate::agent::tool_groups::{self, parse_allowed_groups, ToolGroup};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubagentRole {
    Scout,
    Review,
    SecurityAnalyst,
}

impl SubagentRole {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "scout" => Some(Self::Scout),
            "review" => Some(Self::Review),
            "security_analyst" => Some(Self::SecurityAnalyst),
            _ => None,
        }
    }

    pub fn default_groups(self) -> Vec<ToolGroup> {
        match self {
            Self::Scout => parse_allowed_groups(&[
                "environment_read".into(),
                "workspace_read".into(),
                "git_read".into(),
                "memory_read".into(),
                "plans_read".into(),
                "rules_skills_read".into(),
            ]),
            Self::Review => parse_allowed_groups(&[
                "environment_read".into(),
                "workspace_read".into(),
                "diff_read".into(),
                "tasks_read".into(),
            ]),
            Self::SecurityAnalyst => parse_allowed_groups(&[
                "environment_read".into(),
                "workspace_read".into(),
                "diff_read".into(),
                "git_read".into(),
            ]),
        }
    }
}

pub fn role_id(role: SubagentRole) -> &'static str {
    match role {
        SubagentRole::Scout => "scout",
        SubagentRole::Review => "review",
        SubagentRole::SecurityAnalyst => "security_analyst",
    }
}

pub fn display_name_en(role: SubagentRole) -> &'static str {
    match role {
        SubagentRole::Scout => "Scout",
        SubagentRole::Review => "Review",
        SubagentRole::SecurityAnalyst => "Security Analyst",
    }
}

/// Render the tool inventory that the subagent will actually receive in this
/// run, grouped by purpose. Listing tools explicitly in the system prompt
/// stops weaker models from claiming "I don't have file-system access" and
/// surrendering before they try the schema they were just handed.
fn render_tool_inventory(groups: &[ToolGroup]) -> String {
    let names: std::collections::HashSet<&'static str> =
        tool_groups::registry_filtered(groups, true)
            .into_iter()
            .map(|t| t.name)
            .collect();

    let buckets: &[(&str, &[&str])] = &[
        ("Workspace (read repo files)", &[
            "list_workspace_files",
            "read_workspace_file",
            "workspace_search",
        ]),
        ("Diffs", &["workspace_git_status", "workspace_diff"]),
        ("Git", &[
            "git_status",
            "git_diff",
            "git_log",
            "git_show",
            "git_branch_info",
            "git_ls_files",
        ]),
        ("Environment / shell", &["environment_detect", "shell_exec"]),
        ("Memory", &[
            "memory_list",
            "memory_read",
            "memory_search",
            "memory_graph",
            "memory_backlinks",
            "memory_category_list",
            "memory_context_list",
        ]),
        ("Plans", &["plan_list", "plan_read", "plan_load", "plan_context_list"]),
        ("Tasks", &["task_list", "task_get"]),
        ("Rules & Skills", &[
            "rules_list",
            "rules_read",
            "skills_list",
            "skills_read",
        ]),
        ("Web", &["web_search", "web_fetch"]),
    ];

    let mut lines = Vec::new();
    for (heading, candidates) in buckets {
        let available: Vec<&str> = candidates
            .iter()
            .copied()
            .filter(|n| names.contains(n))
            .collect();
        if available.is_empty() {
            continue;
        }
        lines.push(format!("- {heading}: {}", available.join(", ")));
    }
    if lines.is_empty() {
        return String::from("- (no tools provisioned)");
    }
    lines.join("\n")
}

pub fn subagent_system_prompt(
    workspace_root: &str,
    role: SubagentRole,
    display_name: &str,
    task: &str,
    success_criteria: &[String],
    groups: &[ToolGroup],
) -> String {
    let role_line = match role {
        SubagentRole::Scout => "exploring codebase structure, files, rules, and skills",
        SubagentRole::Review => "finding bugs, regressions, UX issues, and diff risks",
        SubagentRole::SecurityAnalyst => {
            "reviewing secrets, injection risks, tool scope, and dangerous data flows"
        }
    };
    let criteria = if success_criteria.is_empty() {
        String::new()
    } else {
        format!(
            "\nSuccess criteria:\n{}\n",
            success_criteria
                .iter()
                .map(|c| format!("- {c}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let inventory = render_tool_inventory(groups);
    format!(
        "You are {display_name}, a BLXCode subagent specialized in {role_line}.\n\
         Workspace: {workspace_root}\n\
         Task: {task}\n\
         {criteria}\n\
         Tools available to you in this run:\n{inventory}\n\
         \n\
         You DO have file-system access via the workspace tools listed above. \
         Never claim a lack of tools without first attempting `list_workspace_files` \
         and `read_workspace_file` against the workspace root. If a tool errors, \
         report the exact error in `submit_result`.\n\
         Call `environment_detect` before any shell or git tool.\n\
         You MUST finish by calling the `submit_result` tool exactly once with structured JSON. \
         Free-form assistant text is ignored.\n\
         Do not call `subagents.run`.\n"
    )
}

pub fn resolve_display_names(specs: &[(SubagentRole, Option<String>)]) -> Vec<String> {
    let mut counts = std::collections::HashMap::<&str, u32>::new();
    let mut out = Vec::new();
    for (role, title) in specs {
        let base = title
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(display_name_en(*role));
        let n = counts.entry(base).or_insert(0);
        *n += 1;
        let name = if *n == 1 {
            base.to_owned()
        } else {
            format!("{base} {}", n)
        };
        out.push(name);
    }
    out
}

pub fn truncate_submit_result(mut value: Value) -> Value {
    const MAX_FINDINGS: usize = 20;
    const MAX_ARTIFACTS: usize = 10;
    if let Some(findings) = value.get_mut("findings").and_then(|v| v.as_array_mut()) {
        findings.truncate(MAX_FINDINGS);
        for f in findings.iter_mut() {
            if let Some(ev) = f.get_mut("evidence").and_then(|v| v.as_str()) {
                if ev.len() > 2000 {
                    f["evidence"] = Value::String(format!("{}…", &ev[..2000]));
                }
            }
        }
    }
    if let Some(artifacts) = value.get_mut("artifacts").and_then(|v| v.as_array_mut()) {
        artifacts.truncate(MAX_ARTIFACTS);
        for a in artifacts.iter_mut() {
            if let Some(c) = a.get_mut("content").and_then(|v| v.as_str()) {
                if c.len() > 4000 {
                    a["content"] = Value::String(format!("{}…", &c[..4000]));
                }
            }
        }
    }
    value
}
