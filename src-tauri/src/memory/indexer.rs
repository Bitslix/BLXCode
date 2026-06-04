use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::agent_settings::{load_settings_pub, AgentProviderKind};
use crate::agents_layout::validate_workspace_cwd;
use crate::memory::frontmatter::{serialize_frontmatter, MemoryFrontmatter};
use crate::memory::paths::{get_global_roots, get_workspace_roots};
use crate::plans::{plan_list_inner, plan_read_inner, PlanMeta};
use crate::skills_rules::store::{list_rules, list_skills, read_rule, read_skill};
use crate::skills_rules::types::{RuleEntry, SkillEntry};

const SETTINGS_FILE: &str = "memory_index_settings.json";
const MANAGED_BY: &str = "memory-indexer";
const EXCERPT_MAX_CHARS: usize = 1200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryIndexSettings {
    pub provider: AgentProviderKind,
    pub model_id: String,
}

impl MemoryIndexSettings {
    fn from_agent_settings(app: &AppHandle) -> Self {
        match load_settings_pub(app) {
            Ok(settings) => Self {
                provider: settings.provider,
                model_id: settings.model_id,
            },
            Err(_) => Self::default(),
        }
    }
}

impl Default for MemoryIndexSettings {
    fn default() -> Self {
        Self {
            provider: AgentProviderKind::Openrouter,
            model_id: "openai/gpt-5".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemoryIndexStats {
    pub workspace_count: usize,
    pub global_count: usize,
    pub last_indexed_at: Option<u64>,
    pub generated_files: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryIndexRunReport {
    pub workspace_cwd: String,
    pub files_changed: u32,
    pub generated_paths: Vec<String>,
    pub warnings: Vec<String>,
}

fn load_memory_index_settings(app: &AppHandle) -> Result<MemoryIndexSettings, String> {
    load_settings(app)
}

#[tauri::command]
pub fn memory_index_settings_save(
    settings: MemoryIndexSettings,
) -> Result<MemoryIndexSettings, String> {
    let settings = MemoryIndexSettings {
        provider: settings.provider,
        model_id: settings.model_id.trim().to_string(),
    };
    save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn memory_index_settings_get(app: AppHandle) -> Result<MemoryIndexSettings, String> {
    load_memory_index_settings(&app)
}

#[tauri::command]
pub fn memory_index_stats() -> Result<MemoryIndexStats, String> {
    load_stats()
}

pub fn run_memory_indexer_for_workspace(
    app: AppHandle,
    workspace_cwd: String,
) -> Result<MemoryIndexRunReport, String> {
    let _settings = load_settings(&app)?;
    let ws = validate_workspace_cwd(&workspace_cwd)?;
    let workspace_cwd = ws.to_string_lossy().to_string();
    let workspace_name = ws
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string();
    let roots = get_workspace_roots(&workspace_cwd)?;
    let global = get_global_roots();

    let rules = list_rules(&workspace_cwd).unwrap_or_default();
    let skills = list_skills(&workspace_cwd).unwrap_or_default();
    let plans = plan_list_inner(&workspace_cwd).unwrap_or_default();

    let mut changed = 0u32;
    let mut generated = Vec::new();
    let mut warnings = Vec::new();

    fs::create_dir_all(&roots.memory).map_err(|e| format!("create workspace memory: {e}"))?;
    fs::create_dir_all(&global.memory).map_err(|e| format!("create global memory: {e}"))?;

    render_rules(
        &workspace_cwd,
        &workspace_name,
        &roots.memory,
        &rules,
        &mut warnings,
    )
    .map(|paths| record_paths(paths, &mut generated, &mut changed))?;
    render_skills(
        &workspace_cwd,
        &workspace_name,
        &roots.memory,
        &skills,
        &mut warnings,
    )
    .map(|paths| record_paths(paths, &mut generated, &mut changed))?;
    render_plans(
        &workspace_cwd,
        &workspace_name,
        &roots.memory,
        &plans,
        &mut warnings,
    )
    .map(|paths| record_paths(paths, &mut generated, &mut changed))?;

    render_global_aggregate(&global.memory, "rules", &workspace_name, &rules.len())
        .map(|path| record_one(path, &mut generated, &mut changed))?;
    render_global_aggregate(&global.memory, "skills", &workspace_name, &skills.len())
        .map(|path| record_one(path, &mut generated, &mut changed))?;
    render_global_aggregate(&global.memory, "plans", &workspace_name, &plans.len())
        .map(|path| record_one(path, &mut generated, &mut changed))?;

    save_stats(&MemoryIndexStats {
        workspace_count: rules.len() + skills.len() + plans.iter().filter(|p| !p.is_index).count(),
        global_count: 3,
        last_indexed_at: Some(now_secs()),
        generated_files: generated.clone(),
        warnings: warnings.clone(),
    })?;

    Ok(MemoryIndexRunReport {
        workspace_cwd,
        files_changed: changed,
        generated_paths: generated,
        warnings,
    })
}

fn render_rules(
    workspace_cwd: &str,
    workspace_name: &str,
    memory_root: &Path,
    rules: &[RuleEntry],
    warnings: &mut Vec<String>,
) -> Result<Vec<(String, bool)>, String> {
    let mut out = Vec::new();
    let mut links = Vec::new();
    for rule in rules {
        let slug = slugify(rule.name.trim_end_matches(".md"));
        let path = format!("rules/{slug}.md");
        let content = read_rule(workspace_cwd, &rule.name).unwrap_or_else(|e| {
            warnings.push(format!("read rule {}: {e}", rule.name));
            String::new()
        });
        let body = format!(
            "# {}\n\n- Workspace: `{}`\n- Source: `.agents/rules/{}`\n- Enabled: `{}`\n- Category: `{}`\n- Updated: `{}`\n\n## Summary\n\n{}\n\n## Excerpt\n\n{}\n",
            rule.title,
            workspace_name,
            rule.name,
            rule.enabled,
            rule.category.clone().unwrap_or_else(|| "uncategorized".into()),
            rule.updated_at,
            fallback_summary(&rule.summary),
            fenced_excerpt(&content),
        );
        let changed = write_managed_note(memory_root, &path, &rule.title, "rules", &body)?;
        out.push((path.clone(), changed));
        links.push(format!("- [[{path}|{}]]", rule.title));
    }
    let overview = category_overview("Rules", workspace_name, &links);
    let changed = write_managed_note(memory_root, "rules/README.md", "Rules", "rules", &overview)?;
    out.push(("rules/README.md".into(), changed));
    Ok(out)
}

fn render_skills(
    workspace_cwd: &str,
    workspace_name: &str,
    memory_root: &Path,
    skills: &[SkillEntry],
    warnings: &mut Vec<String>,
) -> Result<Vec<(String, bool)>, String> {
    let mut out = Vec::new();
    let mut links = Vec::new();
    for skill in skills {
        let slug = slugify(&skill.name);
        let path = format!("skills/{slug}.md");
        let content = read_skill(workspace_cwd, &skill.name).unwrap_or_else(|e| {
            warnings.push(format!("read skill {}: {e}", skill.name));
            String::new()
        });
        let source = serde_json::to_string(&skill.source).unwrap_or_else(|_| "{}".into());
        let body = format!(
            "# {}\n\n- Workspace: `{}`\n- Source: `.agents/skills/{}`\n- Enabled: `{}`\n- Category: `{}`\n- Missing SKILL.md: `{}`\n- Source meta: `{}`\n- Updated: `{}`\n\n## Summary\n\n{}\n\n## Excerpt\n\n{}\n",
            skill.title,
            workspace_name,
            skill.name,
            skill.enabled,
            skill.category.clone().unwrap_or_else(|| "uncategorized".into()),
            skill.missing_skill_md,
            source,
            skill.updated_at,
            fallback_summary(&skill.summary),
            fenced_excerpt(&content),
        );
        let changed = write_managed_note(memory_root, &path, &skill.title, "skills", &body)?;
        out.push((path.clone(), changed));
        links.push(format!("- [[{path}|{}]]", skill.title));
    }
    let overview = category_overview("Skills", workspace_name, &links);
    let changed = write_managed_note(
        memory_root,
        "skills/README.md",
        "Skills",
        "skills",
        &overview,
    )?;
    out.push(("skills/README.md".into(), changed));
    Ok(out)
}

fn render_plans(
    workspace_cwd: &str,
    workspace_name: &str,
    memory_root: &Path,
    plans: &[PlanMeta],
    warnings: &mut Vec<String>,
) -> Result<Vec<(String, bool)>, String> {
    let mut out = Vec::new();
    let mut links = Vec::new();
    for plan in plans.iter().filter(|plan| !plan.is_index) {
        let slug = slugify(if plan.slug.is_empty() {
            &plan.name
        } else {
            &plan.slug
        });
        let path = format!("plans/{slug}.md");
        let content = plan_read_inner(workspace_cwd, &plan.path)
            .map(|content| content.content)
            .unwrap_or_else(|e| {
                warnings.push(format!("read plan {}: {e}", plan.path));
                String::new()
            });
        let body = format!(
            "# {}\n\n- Workspace: `{}`\n- Source: `.agents/plans/{}`\n- Tasks total: `{}`\n- Pending: `{}`\n- In progress: `{}`\n- Blocked: `{}`\n- Completed: `{}`\n- Cancelled: `{}`\n\n## Excerpt\n\n{}\n",
            plan.title,
            workspace_name,
            plan.path,
            plan.task_summary.total,
            plan.task_summary.pending,
            plan.task_summary.in_progress,
            plan.task_summary.blocked,
            plan.task_summary.completed,
            plan.task_summary.cancelled,
            fenced_excerpt(&content),
        );
        let changed = write_managed_note(memory_root, &path, &plan.title, "plans", &body)?;
        out.push((path.clone(), changed));
        links.push(format!("- [[{path}|{}]]", plan.title));
    }
    let overview = category_overview("Plans", workspace_name, &links);
    let changed = write_managed_note(memory_root, "plans/README.md", "Plans", "plans", &overview)?;
    out.push(("plans/README.md".into(), changed));
    Ok(out)
}

fn render_global_aggregate(
    memory_root: &Path,
    category: &str,
    workspace_name: &str,
    count: &usize,
) -> Result<(String, bool), String> {
    let path = format!("{category}/{workspace_name}.md");
    let title = format!("{workspace_name} {category}");
    let body = format!(
        "# {title}\n\n- Workspace: `{workspace_name}`\n- Indexed {category}: `{count}`\n- Last indexed: `{}`\n\nThis global aggregate is maintained by BLXCode's Memory Indexer.\n",
        now_secs(),
    );
    let changed = write_managed_note(memory_root, &path, &title, category, &body)?;
    Ok((path, changed))
}

fn category_overview(title: &str, workspace_name: &str, links: &[String]) -> String {
    let list = if links.is_empty() {
        "No indexed entries.".to_string()
    } else {
        links.join("\n")
    };
    format!(
        "# {title}\n\nWorkspace: `{workspace_name}`\n\nThis category is maintained by BLXCode's Memory Indexer.\n\n## Indexed Entries\n\n{list}\n"
    )
}

fn write_managed_note(
    memory_root: &Path,
    rel: &str,
    title: &str,
    category: &str,
    body: &str,
) -> Result<bool, String> {
    let path = safe_memory_path(memory_root, rel)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let fm = MemoryFrontmatter {
        title: Some(title.to_string()),
        enabled: Some(true),
        tags: Some(vec![category.to_string(), MANAGED_BY.to_string()]),
        managed: Some(MANAGED_BY.to_string()),
        ..Default::default()
    };
    let content = serialize_frontmatter(&fm, body);
    write_if_changed(&path, &content)
}

fn safe_memory_path(memory_root: &Path, rel: &str) -> Result<PathBuf, String> {
    if rel.starts_with('/') || rel.starts_with('\\') || rel.contains("..") {
        return Err(format!("invalid generated memory path: {rel}"));
    }
    if !rel.ends_with(".md") {
        return Err(format!("generated memory path must be .md: {rel}"));
    }
    Ok(memory_root.join(rel))
}

fn write_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    match fs::read_to_string(path) {
        Ok(existing) if existing == content => return Ok(false),
        _ => {}
    }
    fs::write(path, content.as_bytes()).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(true)
}

fn record_paths(paths: Vec<(String, bool)>, generated: &mut Vec<String>, changed: &mut u32) {
    for (path, did_change) in paths {
        record_one((path, did_change), generated, changed);
    }
}

fn record_one(path: (String, bool), generated: &mut Vec<String>, changed: &mut u32) {
    generated.push(path.0);
    if path.1 {
        *changed += 1;
    }
}

fn fenced_excerpt(content: &str) -> String {
    let mut excerpt = content.chars().take(EXCERPT_MAX_CHARS).collect::<String>();
    if content.chars().count() > EXCERPT_MAX_CHARS {
        excerpt.push_str("\n...");
    }
    format!("```md\n{}\n```", excerpt.trim())
}

fn fallback_summary(summary: &str) -> String {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        "No summary available.".into()
    } else {
        trimmed.into()
    }
}

fn slugify(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_' || ch.is_whitespace() || ch == '.') && !out.ends_with('-')
        {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "item".into()
    } else {
        out
    }
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(crate::app_paths::app_data_dir()?.join(SETTINGS_FILE))
}

fn stats_path() -> Result<PathBuf, String> {
    Ok(crate::app_paths::app_data_dir()?.join("memory_index_stats.json"))
}

fn load_settings(app: &AppHandle) -> Result<MemoryIndexSettings, String> {
    let path = settings_path()?;
    match fs::read_to_string(&path) {
        Ok(raw) if raw.trim().is_empty() => Ok(MemoryIndexSettings::from_agent_settings(app)),
        Ok(raw) => serde_json::from_str(&raw)
            .map_err(|e| format!("parse memory index settings {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(MemoryIndexSettings::from_agent_settings(app))
        }
        Err(e) => Err(format!(
            "read memory index settings {}: {e}",
            path.display()
        )),
    }
}

fn save_settings(settings: &MemoryIndexSettings) -> Result<(), String> {
    let path = settings_path()?;
    atomic_write_json(&path, settings)
}

fn load_stats() -> Result<MemoryIndexStats, String> {
    let path = stats_path()?;
    match fs::read_to_string(&path) {
        Ok(raw) if raw.trim().is_empty() => Ok(MemoryIndexStats::default()),
        Ok(raw) => serde_json::from_str(&raw)
            .map_err(|e| format!("parse memory index stats {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(MemoryIndexStats::default()),
        Err(e) => Err(format!("read memory index stats {}: {e}", path.display())),
    }
}

fn save_stats(stats: &MemoryIndexStats) -> Result<(), String> {
    let path = stats_path()?;
    atomic_write_json(&path, stats)
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| format!("encode json: {e}"))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_keeps_generated_paths_safe() {
        assert_eq!(slugify("rule-Foo.md"), "rule-foo-md");
        assert_eq!(slugify("../bad"), "bad");
    }

    #[test]
    fn generated_paths_are_category_first() {
        let root = PathBuf::from("/tmp/memory");
        let path = safe_memory_path(&root, "rules/example.md").unwrap();
        assert_eq!(path, root.join("rules/example.md"));
        assert!(safe_memory_path(&root, "index/rules/example.md").is_ok());
    }
}
