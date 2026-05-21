// Consumers (tauri_bridge, workbench state, panels) arrive in Steps 4–5.
// Keep this file warning-clean during the staged rollout.
#![allow(dead_code)]

//! Mirror of `src-tauri/src/skills_rules/types.rs` (wire portion).
//!
//! Only the types crossing the Tauri boundary live here. On-disk index
//! shapes (`RulesIndex`, `SkillsIndex`) stay backend-only. Keep this file in
//! lockstep with the backend.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleEntry {
    pub name: String,
    pub title: String,
    pub summary: String,
    pub enabled: bool,
    pub size_bytes: u64,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    pub name: String,
    pub title: String,
    pub summary: String,
    pub enabled: bool,
    pub source: SkillSourceMeta,
    pub installed_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub missing_skill_md: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillSourceKind {
    /// Built-in harness skill embedded in the binary.
    Core,
    Git,
    Npm,
    Local,
    AgentCreated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSourceMeta {
    pub kind: SkillSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub git_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSourceInput {
    pub kind: SkillSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub git_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}
