use super::types::{
    PluginCapability, PluginCommandContribution, PluginInstallSource, PluginManifest,
    PluginRegistryEntry, CATEGORY_RUNTIME,
};
use std::collections::BTreeMap;

const BUILTIN_INSTALLED_AT: &str = "2026-06-04T00:00:00Z";

pub fn built_in_plugins() -> Vec<PluginRegistryEntry> {
    [
        (
            "runtime-node",
            "Node package scripts",
            "run-detectors/node.json",
        ),
        ("runtime-rust", "Rust Cargo", "run-detectors/rust.json"),
        ("runtime-go", "Go modules", "run-detectors/go.json"),
        (
            "runtime-c-cpp",
            "C/C++ build tools",
            "run-detectors/c-cpp.json",
        ),
        (
            "runtime-shell",
            "Shell scripts",
            "run-detectors/shell-scripts.json",
        ),
        (
            "runtime-direct",
            "Direct runtime entrypoints",
            "run-detectors/direct-runtime.json",
        ),
    ]
    .into_iter()
    .map(|(id, name, path)| built_in_entry(id, name, path))
    .collect()
}

fn built_in_entry(id: &str, name: &str, detector_path: &str) -> PluginRegistryEntry {
    PluginRegistryEntry {
        manifest: PluginManifest {
            id: id.into(),
            name: name.into(),
            version: "1.0.0".into(),
            description: String::new(),
            author: Some("BLXCode".into()),
            category: CATEGORY_RUNTIME.into(),
            capabilities: vec![PluginCapability::RunCommands],
            commands: vec![PluginCommandContribution {
                capability: PluginCapability::RunCommands,
                path: detector_path.into(),
            }],
            metadata: BTreeMap::new(),
        },
        enabled: true,
        source: PluginInstallSource::built_in(),
        installed_at: BUILTIN_INSTALLED_AT.into(),
        updated_at: BUILTIN_INSTALLED_AT.into(),
        path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::types::{PluginCapability, CATEGORY_RUNTIME};

    #[test]
    fn all_builtins_are_runtime_run_command_plugins() {
        let plugins = built_in_plugins();
        assert_eq!(plugins.len(), 6);
        for plugin in plugins {
            assert_eq!(plugin.manifest.category, CATEGORY_RUNTIME);
            assert!(plugin
                .manifest
                .capabilities
                .contains(&PluginCapability::RunCommands));
            assert!(!plugin.removable());
        }
    }
}
