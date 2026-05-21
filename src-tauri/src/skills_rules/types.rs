// Step 1 of `.agents/plans/skills-rules-tabs.md` only lands the type module;
// the store + commands consume these in Step 2. Suppress dead-code lints
// here so the in-progress branch stays warning-clean.
#![allow(dead_code)]

//! Wire and on-disk types for Skills & Rules.
//!
//! - `RuleEntry` / `SkillEntry` are mirrored on the frontend in
//!   `src/skills_rules_wire.rs`. Change both in lockstep.
//! - `RulesIndex` / `SkillsIndex` are the on-disk JSON manifests under
//!   `.agents/{rules,skills}/index.json`; they stay backend-only.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Schema version for both `index.json` files. Bump only on breaking changes
/// — additive changes get a serde `#[serde(default)]` instead.
pub const INDEX_VERSION: u32 = 1;

// =========================================================================
// Wire types (mirrored in src/skills_rules_wire.rs)
// =========================================================================

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
    /// `true` when the skill folder lacks a readable `SKILL.md` on disk —
    /// the UI uses this to surface a warn badge.
    #[serde(default)]
    pub missing_skill_md: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillSourceKind {
    /// Built-in harness skill embedded in the binary.
    Core,
    Git,
    Npm,
    Local,
    /// Created in-place by the BLXCode agent via `skills_write`.
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

/// Payload accepted by `skills_install` / the `skills_install` agent tool.
///
/// Shape mirrors `SkillSourceMeta` but is treated as **input**: the store
/// records the final, validated meta only after the install succeeded, so
/// inputs go through their own type to keep validation explicit.
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

// =========================================================================
// On-disk index files (backend-only)
// =========================================================================

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesIndex {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub rules: BTreeMap<String, RuleIndexEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleIndexEntry {
    pub enabled: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsIndex {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillIndexEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillIndexEntry {
    pub enabled: bool,
    pub source: SkillSourceMeta,
    pub installed_at: String,
    pub updated_at: String,
}

impl Default for RulesIndex {
    fn default() -> Self {
        Self { version: INDEX_VERSION, rules: BTreeMap::new() }
    }
}

impl Default for SkillsIndex {
    fn default() -> Self {
        Self { version: INDEX_VERSION, skills: BTreeMap::new() }
    }
}

fn default_version() -> u32 {
    INDEX_VERSION
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<T: serde::de::DeserializeOwned>(s: &str) -> T {
        serde_json::from_str(s).expect("valid json")
    }

    #[test]
    fn rule_entry_camel_case_roundtrip() {
        let entry = RuleEntry {
            name: "rule-foo.md".into(),
            title: "Foo".into(),
            summary: "bar".into(),
            enabled: true,
            size_bytes: 42,
            updated_at: "2026-05-20T11:00:00Z".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        // camelCase on the wire
        assert!(json.contains("\"sizeBytes\":42"));
        assert!(json.contains("\"updatedAt\":\"2026-05-20T11:00:00Z\""));
        let back: RuleEntry = parse(&json);
        assert_eq!(back, entry);
    }

    #[test]
    fn skill_entry_roundtrip_with_git_source() {
        let entry = SkillEntry {
            name: "leptos-guide".into(),
            title: "Leptos Guide".into(),
            summary: "hints".into(),
            enabled: true,
            source: SkillSourceMeta {
                kind: SkillSourceKind::Git,
                url: Some("https://example.com/x.git".into()),
                git_ref: Some("main".into()),
                package: None,
                version: None,
                path: None,
            },
            installed_at: "2026-05-20T11:00:00Z".into(),
            updated_at: "2026-05-20T11:00:00Z".into(),
            missing_skill_md: false,
        };
        let json = serde_json::to_string(&entry).unwrap();
        // kebab-case for the source kind
        assert!(json.contains("\"kind\":\"git\""));
        // `ref` keyword renamed
        assert!(json.contains("\"ref\":\"main\""));
        // omitted fields skipped
        assert!(!json.contains("\"package\""));
        let back: SkillEntry = parse(&json);
        assert_eq!(back, entry);
    }

    #[test]
    fn agent_created_source_kind() {
        let meta = SkillSourceMeta {
            kind: SkillSourceKind::AgentCreated,
            url: None,
            git_ref: None,
            package: None,
            version: None,
            path: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"kind\":\"agent-created\""));
    }

    #[test]
    fn rules_index_default_version_when_missing() {
        let json = r#"{"rules":{}}"#;
        let idx: RulesIndex = parse(json);
        assert_eq!(idx.version, INDEX_VERSION);
        assert!(idx.rules.is_empty());
    }

    #[test]
    fn skills_index_full_roundtrip() {
        let mut idx = SkillsIndex::default();
        idx.skills.insert(
            "leptos-guide".into(),
            SkillIndexEntry {
                enabled: true,
                source: SkillSourceMeta {
                    kind: SkillSourceKind::Local,
                    url: None,
                    git_ref: None,
                    package: None,
                    version: None,
                    path: None,
                },
                installed_at: "2026-05-20T11:00:00Z".into(),
                updated_at: "2026-05-20T11:00:00Z".into(),
            },
        );
        let json = serde_json::to_string(&idx).unwrap();
        let back: SkillsIndex = parse(&json);
        assert_eq!(back, idx);
    }

    #[test]
    fn skill_source_input_accepts_partial_payload() {
        // Frontend may send `{ "kind": "git", "url": "..." }` without a ref.
        let json = r#"{"kind":"git","url":"https://example.com/x.git"}"#;
        let input: SkillSourceInput = parse(json);
        assert!(matches!(input.kind, SkillSourceKind::Git));
        assert_eq!(input.url.as_deref(), Some("https://example.com/x.git"));
        assert!(input.git_ref.is_none());
        assert!(input.package.is_none());
    }
}
