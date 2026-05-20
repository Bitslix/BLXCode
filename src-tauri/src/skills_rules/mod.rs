//! Skills & Rules: workspace-local manifests under `.agents/{skills,rules}/`.
//!
//! - `types`   — wire types (`RuleEntry`, `SkillEntry`) shared with the
//!   frontend, plus on-disk index file shapes that stay backend-only.
//! - `store`   — Disk I/O, self-heal and toggling for the two index files
//!   (added in a follow-up step).
//! - `install` — Skill installation (`git` / `npm` / `local`) (follow-up step).
//!
//! Schritt 1 of `.agents/plans/skills-rules-tabs.md` only lands `types`; the
//! commands module and Tauri wrappers follow in Schritt 2.

pub mod install;
pub mod store;
pub mod types;

pub use install::install_skill;
pub use store::{
    ensure_skills_rules_roots, list_rules, list_skills, read_rule, read_skill, remove_rule,
    remove_skill, set_rule_enabled, set_skill_enabled, write_rule, write_skill,
};
