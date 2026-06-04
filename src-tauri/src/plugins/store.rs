use super::types::{PluginRegistry, PluginRegistryEntry};
use crate::app_paths;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const PLUGINS_DIR: &str = "plugins";
const REGISTRY_FILE: &str = "index.json";

pub fn plugins_dir() -> Result<PathBuf, String> {
    let dir = app_paths::app_data_dir()?.join(PLUGINS_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("create plugins dir {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn installed_plugin_dir(plugin_id: &str) -> Result<PathBuf, String> {
    let id = super::types::normalize_plugin_id(plugin_id)?;
    Ok(plugins_dir()?.join(id))
}

pub fn registry_path() -> Result<PathBuf, String> {
    Ok(plugins_dir()?.join(REGISTRY_FILE))
}

pub fn load_registry() -> Result<PluginRegistry, String> {
    let path = registry_path()?;
    match fs::read_to_string(&path) {
        Ok(raw) if raw.trim().is_empty() => Ok(PluginRegistry::default()),
        Ok(raw) => serde_json::from_str::<PluginRegistry>(&raw)
            .map_err(|e| format!("parse {}: {e}", path.display()))?
            .normalized(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(PluginRegistry::default()),
        Err(err) => Err(format!("read {}: {err}", path.display())),
    }
}

pub fn save_registry(registry: &PluginRegistry) -> Result<(), String> {
    let registry = registry.clone().normalized()?;
    let path = registry_path()?;
    let parent = path.parent().ok_or("invalid plugins registry path")?;
    fs::create_dir_all(parent).map_err(|e| format!("create plugins dir: {e}"))?;
    let body = serde_json::to_vec_pretty(&registry)
        .map_err(|e| format!("serialize plugins registry: {e}"))?;
    let tmp = parent.join(format!(".{REGISTRY_FILE}.{}.tmp", std::process::id()));
    {
        let mut file = fs::File::create(&tmp).map_err(|e| format!("create tmp: {e}"))?;
        file.write_all(&body)
            .map_err(|e| format!("write tmp: {e}"))?;
        file.sync_all().ok();
    }
    fs::rename(&tmp, &path).map_err(|e| format!("rename tmp -> final: {e}"))
}

pub fn upsert_entry(entry: PluginRegistryEntry) -> Result<PluginRegistry, String> {
    let entry = entry.normalized()?;
    let mut registry = load_registry()?;
    match registry
        .plugins
        .iter_mut()
        .find(|existing| existing.manifest.id == entry.manifest.id)
    {
        Some(existing) => *existing = entry,
        None => registry.plugins.push(entry),
    }
    registry
        .plugins
        .sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    save_registry(&registry)?;
    Ok(registry)
}

pub fn set_enabled(plugin_id: &str, enabled: bool) -> Result<PluginRegistry, String> {
    let id = super::types::normalize_plugin_id(plugin_id)?;
    let mut registry = load_registry()?;
    let Some(entry) = registry
        .plugins
        .iter_mut()
        .find(|entry| entry.manifest.id == id)
    else {
        return Err(format!("plugin `{id}` not found"));
    };
    entry.enabled = enabled;
    save_registry(&registry)?;
    Ok(registry)
}

pub fn remove_entry(plugin_id: &str) -> Result<PluginRegistry, String> {
    let id = super::types::normalize_plugin_id(plugin_id)?;
    let mut registry = load_registry()?;
    let Some(entry) = registry
        .plugins
        .iter()
        .find(|entry| entry.manifest.id == id)
        .cloned()
    else {
        return Err(format!("plugin `{id}` not found"));
    };
    if !entry.removable() {
        return Err(format!("built-in plugin `{id}` cannot be removed"));
    }
    registry.plugins.retain(|entry| entry.manifest.id != id);
    save_registry(&registry)?;
    if let Some(path) = entry.path {
        let path = PathBuf::from(path);
        if path.exists() {
            fs::remove_dir_all(&path)
                .map_err(|e| format!("remove plugin dir {}: {e}", path.display()))?;
        }
    }
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_paths::test_support::AppDataDirGuard;
    use crate::plugins::types::{
        PluginCapability, PluginCommandContribution, PluginInstallKind, PluginInstallSource,
        PluginManifest, CATEGORY_RUNTIME,
    };
    use std::collections::BTreeMap;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "blx_plugins_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(id: &str, kind: PluginInstallKind) -> PluginRegistryEntry {
        PluginRegistryEntry {
            manifest: PluginManifest {
                id: id.into(),
                name: "Runtime".into(),
                version: "1.0.0".into(),
                description: String::new(),
                author: None,
                category: CATEGORY_RUNTIME.into(),
                capabilities: vec![PluginCapability::RunCommands],
                commands: vec![PluginCommandContribution {
                    capability: PluginCapability::RunCommands,
                    path: "run-detectors/runtime.json".into(),
                }],
                metadata: BTreeMap::new(),
            },
            enabled: true,
            source: PluginInstallSource {
                kind,
                url: None,
                git_ref: None,
                package_dir: None,
            },
            installed_at: "2026-06-04T00:00:00Z".into(),
            updated_at: "2026-06-04T00:00:00Z".into(),
            path: None,
        }
    }

    #[test]
    fn registry_round_trips_through_app_data_dir() {
        let dir = tmp_dir("roundtrip");
        let _guard = AppDataDirGuard::new(dir.clone());
        let saved = upsert_entry(entry("runtime-node", PluginInstallKind::GitHub)).unwrap();
        assert_eq!(saved.plugins.len(), 1);
        let loaded = load_registry().unwrap();
        assert_eq!(loaded.plugins[0].manifest.id, "runtime-node");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_enabled_updates_entry() {
        let dir = tmp_dir("enabled");
        let _guard = AppDataDirGuard::new(dir.clone());
        upsert_entry(entry("runtime-node", PluginInstallKind::GitHub)).unwrap();
        let updated = set_enabled("runtime-node", false).unwrap();
        assert!(!updated.plugins[0].enabled);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_entry_rejects_built_ins() {
        let dir = tmp_dir("remove_builtin");
        let _guard = AppDataDirGuard::new(dir.clone());
        upsert_entry(entry("runtime-rust", PluginInstallKind::BuiltIn)).unwrap();
        assert!(remove_entry("runtime-rust").is_err());
        assert_eq!(load_registry().unwrap().plugins.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
