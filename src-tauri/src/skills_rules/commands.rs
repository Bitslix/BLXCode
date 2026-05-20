//! Tauri command surface for Skills & Rules.
//!
//! Eleven commands mirror the agent toolcall catalogue planned in
//! `.agents/plans/skills-rules-tabs.md`:
//!
//! | Tauri command            | Purpose                              |
//! |--------------------------|--------------------------------------|
//! | `rules_list`             | enumerate `.agents/rules/*.md`       |
//! | `rules_read`             | read one rule's markdown body        |
//! | `rules_write`            | create or overwrite a rule           |
//! | `rules_set_enabled`      | toggle a rule's manifest enabled bit |
//! | `rules_remove`           | delete a rule + clean its index      |
//! | `skills_list`            | enumerate `.agents/skills/<name>/`   |
//! | `skills_read`            | read `SKILL.md`                      |
//! | `skills_write`           | create or overwrite `SKILL.md`       |
//! | `skills_set_enabled`     | toggle a skill's enabled bit         |
//! | `skills_remove`          | delete the skill folder + index      |
//! | `skills_install`         | install from git / npm / local       |
//!
//! The store and install modules already enforce path sandboxing and atomic
//! writes; these wrappers keep the Tauri surface thin.

use crate::skills_rules::install;
use crate::skills_rules::store;
use crate::skills_rules::types::{
    RuleEntry, SkillEntry, SkillSourceInput,
};

/// Idempotently create `.agents/{rules,skills}/` plus their `index.json`
/// manifests. Safe to call on every workspace open.
#[tauri::command]
pub fn skills_rules_bootstrap(ws: String) -> Result<(), String> {
    store::ensure_skills_rules_roots(&ws).map(|_| ())
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn rules_list(ws: String) -> Result<Vec<RuleEntry>, String> {
    store::list_rules(&ws)
}

#[tauri::command]
pub fn rules_read(ws: String, name: String) -> Result<String, String> {
    store::read_rule(&ws, &name)
}

#[tauri::command]
pub fn rules_write(ws: String, name: String, content: String) -> Result<RuleEntry, String> {
    store::write_rule(&ws, &name, &content)
}

#[tauri::command]
pub fn rules_set_enabled(ws: String, name: String, enabled: bool) -> Result<RuleEntry, String> {
    store::set_rule_enabled(&ws, &name, enabled)
}

#[tauri::command]
pub fn rules_remove(ws: String, name: String) -> Result<(), String> {
    store::remove_rule(&ws, &name)
}

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn skills_list(ws: String) -> Result<Vec<SkillEntry>, String> {
    store::list_skills(&ws)
}

#[tauri::command]
pub fn skills_read(ws: String, name: String) -> Result<String, String> {
    store::read_skill(&ws, &name)
}

#[tauri::command]
pub fn skills_write(ws: String, name: String, content: String) -> Result<SkillEntry, String> {
    store::write_skill(&ws, &name, &content)
}

#[tauri::command]
pub fn skills_set_enabled(ws: String, name: String, enabled: bool) -> Result<SkillEntry, String> {
    store::set_skill_enabled(&ws, &name, enabled)
}

#[tauri::command]
pub fn skills_remove(ws: String, name: String) -> Result<(), String> {
    store::remove_skill(&ws, &name)
}

#[tauri::command]
pub fn skills_install(
    ws: String,
    name: String,
    source: SkillSourceInput,
) -> Result<SkillEntry, String> {
    install::install_skill(&ws, &name, source)
}
