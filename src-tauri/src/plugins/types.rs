use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PLUGIN_MANIFEST_FILE: &str = "blx-plugin.json";
pub const CATEGORY_RUNTIME: &str = "runtime";
#[allow(dead_code)]
pub const CATEGORY_AGENT: &str = "agent";
#[allow(dead_code)]
pub const CATEGORY_MCP: &str = "mcp";
#[allow(dead_code)]
pub const CATEGORY_UI: &str = "ui";
#[allow(dead_code)]
pub const CATEGORY_WORKFLOW: &str = "workflow";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub commands: Vec<PluginCommandContribution>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl PluginManifest {
    pub fn normalized(mut self) -> Result<Self, String> {
        self.id = normalize_plugin_id(&self.id)?;
        self.name = normalize_required_text(&self.name, "plugin name", 120)?;
        self.version = normalize_required_text(&self.version, "plugin version", 80)?;
        self.description = trim_to(self.description, 600);
        self.author = self
            .author
            .map(|value| trim_to(value, 120))
            .filter(|value| !value.is_empty());
        self.category = normalize_category_id(&self.category)?;
        self.commands = self
            .commands
            .into_iter()
            .map(PluginCommandContribution::normalized)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    pub fn has_capability(&self, capability: PluginCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginCapability {
    RunCommands,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommandContribution {
    pub capability: PluginCapability,
    pub path: String,
}

impl PluginCommandContribution {
    fn normalized(mut self) -> Result<Self, String> {
        self.path = normalize_relative_package_path(&self.path)?;
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginInstallKind {
    BuiltIn,
    GitHub,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallSource {
    pub kind: PluginInstallKind,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub git_ref: Option<String>,
    #[serde(default)]
    pub package_dir: Option<String>,
}

impl PluginInstallSource {
    pub fn built_in() -> Self {
        Self {
            kind: PluginInstallKind::BuiltIn,
            url: None,
            git_ref: None,
            package_dir: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistryEntry {
    pub manifest: PluginManifest,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub source: PluginInstallSource,
    #[serde(default)]
    pub installed_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub path: Option<String>,
}

impl PluginRegistryEntry {
    pub fn normalized(mut self) -> Result<Self, String> {
        self.manifest = self.manifest.normalized()?;
        self.installed_at = trim_to(self.installed_at, 80);
        self.updated_at = trim_to(self.updated_at, 80);
        self.path = self
            .path
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Ok(self)
    }

    pub fn removable(&self) -> bool {
        self.source.kind != PluginInstallKind::BuiltIn
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistry {
    #[serde(default = "default_registry_version")]
    pub version: u32,
    #[serde(default)]
    pub plugins: Vec<PluginRegistryEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunCommandKind {
    Dev,
    Run,
    Debug,
    Test,
    Build,
    Other,
}

impl RunCommandKind {
    pub fn from_label(label: &str) -> Self {
        let lower = label.trim().to_ascii_lowercase();
        if matches!(lower.as_str(), "dev" | "serve" | "watch" | "preview") {
            Self::Dev
        } else if lower.contains("debug") || lower.starts_with("inspect") {
            Self::Debug
        } else if lower == "test" || lower.starts_with("test:") || lower.contains(" test") {
            Self::Test
        } else if lower == "build" || lower.starts_with("build:") || lower.contains(" build") {
            Self::Build
        } else if matches!(lower.as_str(), "start" | "run") || lower.starts_with("start:") {
            Self::Run
        } else {
            Self::Other
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCommandSource {
    pub plugin_id: String,
    pub detector_id: String,
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub package_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCommand {
    pub id: String,
    pub label: String,
    pub command: String,
    #[serde(default)]
    pub cwd_rel: String,
    pub kind: RunCommandKind,
    pub source: RunCommandSource,
}

impl PluginRegistry {
    pub fn normalized(mut self) -> Result<Self, String> {
        if self.version == 0 {
            self.version = default_registry_version();
        }
        let mut seen = std::collections::HashSet::<String>::new();
        let mut plugins = Vec::with_capacity(self.plugins.len());
        for entry in self.plugins {
            let entry = entry.normalized()?;
            if !seen.insert(entry.manifest.id.clone()) {
                return Err(format!("duplicate plugin id `{}`", entry.manifest.id));
            }
            plugins.push(entry);
        }
        self.plugins = plugins;
        Ok(self)
    }

    pub fn enabled_runtime_plugins(&self) -> impl Iterator<Item = &PluginRegistryEntry> {
        self.plugins.iter().filter(|entry| {
            entry.enabled && entry.manifest.has_capability(PluginCapability::RunCommands)
        })
    }
}

#[allow(dead_code)]
pub fn known_category_ids() -> &'static [&'static str] {
    &[
        CATEGORY_RUNTIME,
        CATEGORY_AGENT,
        CATEGORY_MCP,
        CATEGORY_UI,
        CATEGORY_WORKFLOW,
    ]
}

#[allow(dead_code)]
pub fn is_known_category_id(id: &str) -> bool {
    known_category_ids().contains(&id)
}

pub fn normalize_plugin_id(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("plugin id is empty".into());
    }
    if value.len() > 80 {
        return Err("plugin id is too long".into());
    }
    let mut chars = value.chars();
    let first = chars.next().ok_or("plugin id is empty")?;
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err("plugin id must start with a lowercase letter or digit".into());
    }
    if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_') {
        return Err("plugin id may only contain a-z, 0-9, `-`, `_`".into());
    }
    Ok(value.to_string())
}

pub fn normalize_category_id(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(default_category());
    }
    if value.len() > 48 {
        return Err("plugin category is too long".into());
    }
    let mut chars = value.chars();
    let first = chars.next().ok_or("plugin category is empty")?;
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err("plugin category must start with a lowercase letter or digit".into());
    }
    if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_') {
        return Err("plugin category may only contain a-z, 0-9, `-`, `_`".into());
    }
    Ok(value.to_string())
}

#[allow(dead_code)]
pub fn ui_category_group(category: &str) -> &str {
    if is_known_category_id(category) {
        category
    } else {
        "other"
    }
}

fn normalize_required_text(raw: &str, label: &str, max_chars: usize) -> Result<String, String> {
    let value = trim_to(raw.to_string(), max_chars);
    if value.is_empty() {
        Err(format!("{label} is empty"))
    } else {
        Ok(value)
    }
}

fn normalize_relative_package_path(raw: &str) -> Result<String, String> {
    let value = raw.trim().replace('\\', "/");
    if value.is_empty() {
        return Err("plugin command contribution path is empty".into());
    }
    if value.starts_with('/') || value.contains("..") || value.split('/').any(str::is_empty) {
        return Err("plugin command contribution path must stay inside the package".into());
    }
    Ok(value)
}

fn trim_to(raw: String, max_chars: usize) -> String {
    raw.trim().chars().take(max_chars).collect()
}

fn default_category() -> String {
    CATEGORY_RUNTIME.to_string()
}

fn default_registry_version() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(id: &str) -> PluginManifest {
        PluginManifest {
            id: id.into(),
            name: "Runtime Pack".into(),
            version: "1.0.0".into(),
            description: String::new(),
            author: None,
            category: CATEGORY_RUNTIME.into(),
            capabilities: vec![PluginCapability::RunCommands],
            commands: vec![PluginCommandContribution {
                capability: PluginCapability::RunCommands,
                path: "run-detectors/node.json".into(),
            }],
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn plugin_id_rejects_path_like_values() {
        assert!(normalize_plugin_id("../bad").is_err());
        assert!(normalize_plugin_id("Bad").is_err());
        assert!(normalize_plugin_id("bad/plugin").is_err());
        assert_eq!(normalize_plugin_id("runtime-node").unwrap(), "runtime-node");
    }

    #[test]
    fn unknown_category_is_valid_but_grouped_as_other() {
        let category = normalize_category_id("custom-tools").unwrap();
        assert_eq!(category, "custom-tools");
        assert_eq!(ui_category_group(&category), "other");
        assert_eq!(ui_category_group(CATEGORY_RUNTIME), CATEGORY_RUNTIME);
    }

    #[test]
    fn manifest_normalization_rejects_escaping_command_paths() {
        let mut good = manifest("runtime-node").normalized().unwrap();
        assert_eq!(good.category, CATEGORY_RUNTIME);
        good.commands[0].path = "../bad.json".into();
        let bad = PluginManifest {
            commands: good.commands,
            ..manifest("runtime-node")
        };
        assert!(bad.normalized().is_err());
    }

    #[test]
    fn registry_rejects_duplicate_plugin_ids() {
        let entry = PluginRegistryEntry {
            manifest: manifest("runtime-node"),
            enabled: true,
            source: PluginInstallSource::built_in(),
            installed_at: String::new(),
            updated_at: String::new(),
            path: None,
        };
        let registry = PluginRegistry {
            version: 1,
            plugins: vec![entry.clone(), entry],
        };
        assert!(registry.normalized().is_err());
    }
}
