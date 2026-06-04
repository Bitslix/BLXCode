use super::store::{load_registry_with_builtins, plugins_dir, save_registry};
use super::types::{
    PluginInstallKind, PluginInstallSource, PluginManifest, PluginRegistryEntry,
    PLUGIN_MANIFEST_FILE,
};
use crate::proc::command;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const INSTALL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallRequest {
    pub url: String,
    #[serde(default)]
    pub git_ref: Option<String>,
    #[serde(default)]
    pub package_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallProgress {
    pub busy: bool,
    pub phase: String,
    pub message: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
    pub plugin_id: Option<String>,
    pub updated_at_ms: u128,
}

impl Default for PluginInstallProgress {
    fn default() -> Self {
        Self {
            busy: false,
            phase: "idle".into(),
            message: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
            plugin_id: None,
            updated_at_ms: now_ms(),
        }
    }
}

#[derive(Clone, Default)]
pub struct PluginInstallState {
    progress: Arc<Mutex<PluginInstallProgress>>,
}

impl PluginInstallState {
    pub fn snapshot(&self) -> Result<PluginInstallProgress, String> {
        self.progress
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| "plugin install progress lock".into())
    }

    fn set(&self, update: impl FnOnce(&mut PluginInstallProgress)) -> Result<(), String> {
        let mut progress = self
            .progress
            .lock()
            .map_err(|_| "plugin install progress lock".to_string())?;
        update(&mut progress);
        progress.updated_at_ms = now_ms();
        Ok(())
    }
}

pub fn install_from_github(
    state: &PluginInstallState,
    request: PluginInstallRequest,
) -> Result<PluginRegistryEntry, String> {
    state.set(|progress| {
        *progress = PluginInstallProgress {
            busy: true,
            phase: "validating".into(),
            message: "validating".into(),
            ..PluginInstallProgress::default()
        };
    })?;

    let result = install_from_github_inner(state, request);
    match &result {
        Ok(entry) => {
            let id = entry.manifest.id.clone();
            state.set(|progress| {
                progress.busy = false;
                progress.phase = "done".into();
                progress.message = "done".into();
                progress.error = None;
                progress.plugin_id = Some(id);
            })?;
        }
        Err(err) => {
            let error = err.clone();
            state.set(|progress| {
                progress.busy = false;
                progress.phase = "error".into();
                progress.message = "error".into();
                progress.error = Some(error);
            })?;
        }
    }
    result
}

fn install_from_github_inner(
    state: &PluginInstallState,
    request: PluginInstallRequest,
) -> Result<PluginRegistryEntry, String> {
    let parsed = parse_github_source(&request)?;
    let root = plugins_dir()?;
    let staging = root.join(format!(".install.{}.tmp", std::process::id()));
    let clone_dir = staging.join("_clone");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&clone_dir).map_err(|e| format!("create install staging: {e}"))?;
    let install_result = (|| {
        state.set(|progress| {
            progress.phase = "downloading".into();
            progress.message = parsed.clone_url.clone();
        })?;
        run_with_timeout(
            command("git")
                .arg("clone")
                .arg("--depth=1")
                .arg("--branch")
                .arg(&parsed.git_ref)
                .arg("--single-branch")
                .arg("--")
                .arg(&parsed.clone_url)
                .arg(&clone_dir),
            INSTALL_TIMEOUT,
            "git clone",
        )?;

        state.set(|progress| {
            progress.phase = "validatingPackage".into();
            progress.message = parsed.package_dir.clone().unwrap_or_default();
        })?;
        let package_root = parsed
            .package_dir
            .as_deref()
            .map(|rel| safe_join(&clone_dir, rel))
            .transpose()?
            .unwrap_or_else(|| clone_dir.clone());
        let manifest_path = package_root.join(PLUGIN_MANIFEST_FILE);
        let raw = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
        let manifest = serde_json::from_str::<PluginManifest>(&raw)
            .map_err(|e| format!("parse {PLUGIN_MANIFEST_FILE}: {e}"))?
            .normalized()?;
        if manifest.capabilities.is_empty() {
            return Err("plugin manifest declares no supported capabilities".into());
        }

        let final_dir = root.join(&manifest.id);
        if final_dir.exists() {
            return Err(format!(
                "plugin `{}` already exists; remove it before reinstalling",
                manifest.id
            ));
        }

        state.set(|progress| {
            progress.phase = "installing".into();
            progress.message = manifest.id.clone();
        })?;
        fs::create_dir_all(&final_dir).map_err(|e| format!("create plugin dir: {e}"))?;
        let copy_result = copy_dir_contents(&package_root, &final_dir);
        if let Err(err) = copy_result {
            let _ = fs::remove_dir_all(&final_dir);
            return Err(err);
        }
        let now = rfc3339_now();
        let entry = PluginRegistryEntry {
            manifest,
            enabled: true,
            source: PluginInstallSource {
                kind: PluginInstallKind::GitHub,
                url: Some(parsed.original_url),
                git_ref: Some(parsed.git_ref),
                package_dir: parsed.package_dir,
            },
            installed_at: now.clone(),
            updated_at: now,
            path: Some(final_dir.to_string_lossy().into_owned()),
        }
        .normalized()?;

        let mut registry = load_registry_with_builtins()?;
        registry.plugins.push(entry.clone());
        registry
            .plugins
            .sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        if let Err(err) = save_registry(&registry) {
            let _ = fs::remove_dir_all(&final_dir);
            return Err(err);
        }
        Ok(entry)
    })();

    let _ = fs::remove_dir_all(&staging);
    install_result
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedGithubSource {
    original_url: String,
    clone_url: String,
    git_ref: String,
    package_dir: Option<String>,
}

fn parse_github_source(request: &PluginInstallRequest) -> Result<ParsedGithubSource, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("github url is empty".into());
    }
    if url.starts_with('-') || url.contains('\0') {
        return Err("github url contains unsupported characters".into());
    }
    let without_suffix = url.strip_suffix(".git").unwrap_or(url);
    let marker = "github.com/";
    let Some(rest) = without_suffix.split(marker).nth(1) else {
        return Err("plugin install requires a github.com url".into());
    };
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let owner = parts.next().ok_or("github url missing owner")?;
    let repo = parts.next().ok_or("github url missing repository")?;
    validate_github_segment(owner, "owner")?;
    validate_github_segment(repo, "repository")?;

    let mut git_ref = request
        .git_ref
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("main")
        .to_string();
    let mut package_dir = request
        .package_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let remaining: Vec<&str> = parts.collect();
    if remaining.first() == Some(&"tree") && remaining.len() >= 2 {
        git_ref = remaining[1].to_string();
        if remaining.len() > 2 {
            package_dir = Some(remaining[2..].join("/"));
        }
    }
    validate_git_ref(&git_ref)?;
    if let Some(dir) = package_dir.as_deref() {
        validate_package_dir(dir)?;
    }

    Ok(ParsedGithubSource {
        original_url: url.to_string(),
        clone_url: format!("https://github.com/{owner}/{repo}.git"),
        git_ref,
        package_dir,
    })
}

fn validate_github_segment(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.starts_with('-')
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        Err(format!("github {label} contains unsupported characters"))
    } else {
        Ok(())
    }
}

fn validate_git_ref(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.starts_with('-')
        || value.contains("..")
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '/' | '-'))
    {
        Err("git ref contains unsupported characters".into())
    } else {
        Ok(())
    }
}

fn validate_package_dir(value: &str) -> Result<(), String> {
    if value.starts_with('/')
        || value.contains("..")
        || value.split('/').any(str::is_empty)
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '/' | '-'))
    {
        Err("package dir must stay inside the github repository".into())
    } else {
        Ok(())
    }
}

fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    validate_package_dir(rel)?;
    Ok(root.join(rel))
}

fn copy_dir_contents(from: &Path, to: &Path) -> Result<(), String> {
    for entry in fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))? {
        let entry = entry.map_err(|e| format!("read dir entry: {e}"))?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            fs::create_dir_all(&dst).map_err(|e| format!("create {}: {e}", dst.display()))?;
            copy_dir_contents(&src, &dst)?;
        } else if src.is_file() {
            fs::copy(&src, &dst)
                .map_err(|e| format!("copy {} -> {}: {e}", src.display(), dst.display()))?;
        }
    }
    Ok(())
}

fn run_with_timeout(command: &mut Command, timeout: Duration, label: &str) -> Result<(), String> {
    let started = Instant::now();
    let mut child = command.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!("{label}: command not found"),
        _ => format!("{label}: {e}"),
    })?;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("{label}: {e}"))? {
            if status.success() {
                return Ok(());
            }
            return Err(format!("{label} failed with status {status}"));
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            return Err(format!("{label} timed out"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn rfc3339_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("{secs}")
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_tree_url() {
        let parsed = parse_github_source(&PluginInstallRequest {
            url: "https://github.com/acme/blx-plugins/tree/main/packages/node".into(),
            git_ref: None,
            package_dir: None,
        })
        .unwrap();
        assert_eq!(parsed.clone_url, "https://github.com/acme/blx-plugins.git");
        assert_eq!(parsed.git_ref, "main");
        assert_eq!(parsed.package_dir.as_deref(), Some("packages/node"));
    }

    #[test]
    fn rejects_escaping_package_dir() {
        let err = parse_github_source(&PluginInstallRequest {
            url: "https://github.com/acme/blx-plugins".into(),
            git_ref: None,
            package_dir: Some("../bad".into()),
        });
        assert!(err.is_err());
    }
}
