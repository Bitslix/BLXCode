// Step 2 of `.agents/plans/skills-rules-tabs.md` lands the disk-facing store +
// install code. Tauri-command wrappers and frontend consumers wire up in the
// remaining steps; keep the module warning-clean while it is in transit.
#![allow(dead_code)]

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

pub mod commands;
pub mod install;
pub mod pointers;
pub mod store;
pub mod types;
