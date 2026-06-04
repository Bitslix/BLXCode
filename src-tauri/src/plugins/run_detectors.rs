use super::types::{RunCommand, RunCommandKind, RunCommandSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetectorFile {
    #[serde(default)]
    pub detectors: Vec<RunDetectorRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetectorRule {
    pub id: String,
    pub kind: DetectorKind,
    #[serde(default)]
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DetectorKind {
    PackageJsonScripts,
    Cargo,
    GoModule,
    CmakeMake,
    ShellScripts,
    DirectRuntime,
}

#[derive(Clone, Debug)]
pub struct WorkspaceScan {
    root: PathBuf,
    files: BTreeSet<String>,
}

impl WorkspaceScan {
    pub fn from_root(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        if !root.is_dir() {
            return Err("workspace root is not a directory".into());
        }
        let mut files = BTreeSet::new();
        collect_files(&root, &root, &mut files, 5)?;
        Ok(Self { root, files })
    }

    pub fn from_files(root: impl Into<PathBuf>, files: impl IntoIterator<Item = String>) -> Self {
        Self {
            root: root.into(),
            files: files.into_iter().collect(),
        }
    }

    fn read_text(&self, rel: &str) -> Option<String> {
        fs::read_to_string(self.root.join(rel)).ok()
    }

    fn contains(&self, rel: &str) -> bool {
        self.files.contains(rel)
    }
}

pub fn built_in_detector_rules(plugin_id: &str) -> Vec<RunDetectorRule> {
    match plugin_id {
        "runtime-node" => vec![rule("node-scripts", DetectorKind::PackageJsonScripts)],
        "runtime-rust" => vec![rule("cargo", DetectorKind::Cargo)],
        "runtime-go" => vec![rule("go-module", DetectorKind::GoModule)],
        "runtime-c-cpp" => vec![rule("c-cpp-build", DetectorKind::CmakeMake)],
        "runtime-shell" => vec![rule("shell-scripts", DetectorKind::ShellScripts)],
        "runtime-direct" => vec![rule("direct-runtime", DetectorKind::DirectRuntime)],
        _ => Vec::new(),
    }
}

pub fn parse_detector_file(raw: &str) -> Result<RunDetectorFile, String> {
    let file: RunDetectorFile =
        serde_json::from_str(raw).map_err(|e| format!("parse run detector: {e}"))?;
    for detector in &file.detectors {
        validate_detector_id(&detector.id)?;
    }
    Ok(file)
}

pub fn discover_for_plugin(
    scan: &WorkspaceScan,
    plugin_id: &str,
    rules: &[RunDetectorRule],
) -> Vec<RunCommand> {
    let mut commands = Vec::new();
    for rule in rules {
        match rule.kind {
            DetectorKind::PackageJsonScripts => {
                detect_package_json_scripts(scan, plugin_id, rule, &mut commands)
            }
            DetectorKind::Cargo => detect_cargo(scan, plugin_id, rule, &mut commands),
            DetectorKind::GoModule => detect_go(scan, plugin_id, rule, &mut commands),
            DetectorKind::CmakeMake => detect_cmake_make(scan, plugin_id, rule, &mut commands),
            DetectorKind::ShellScripts => {
                detect_shell_scripts(scan, plugin_id, rule, &mut commands)
            }
            DetectorKind::DirectRuntime => {
                detect_direct_runtime(scan, plugin_id, rule, &mut commands)
            }
        }
    }
    stable_dedup(commands)
}

fn detect_package_json_scripts(
    scan: &WorkspaceScan,
    plugin_id: &str,
    rule: &RunDetectorRule,
    out: &mut Vec<RunCommand>,
) {
    for manifest in scan
        .files
        .iter()
        .filter(|rel| basename(rel) == "package.json")
    {
        let Some(raw) = scan.read_text(manifest) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(scripts) = value.get("scripts").and_then(|value| value.as_object()) else {
            continue;
        };
        let dir = directory_of(manifest);
        let pm = package_manager(
            scan,
            &dir,
            value.get("packageManager").and_then(|v| v.as_str()),
        );
        let mut names: Vec<String> = scripts.keys().cloned().collect();
        names.sort_by_key(|name| script_rank(name));
        for name in names {
            let label = format!("{} {}", pm, name);
            let command_text = package_script_command(&pm, &name);
            out.push(command(
                plugin_id,
                &rule.id,
                manifest,
                &dir,
                &label,
                &command_text,
                RunCommandKind::from_label(&name),
            ));
        }
    }
}

fn detect_cargo(
    scan: &WorkspaceScan,
    plugin_id: &str,
    rule: &RunDetectorRule,
    out: &mut Vec<RunCommand>,
) {
    for manifest in scan
        .files
        .iter()
        .filter(|rel| basename(rel) == "Cargo.toml")
    {
        let dir = directory_of(manifest);
        let Some(raw) = scan.read_text(manifest) else {
            continue;
        };
        if raw.contains("[package]") {
            out.push(command(
                plugin_id,
                &rule.id,
                manifest,
                &dir,
                "cargo run",
                "cargo run",
                RunCommandKind::Run,
            ));
        }
        out.push(command(
            plugin_id,
            &rule.id,
            manifest,
            &dir,
            "cargo test",
            "cargo test",
            RunCommandKind::Test,
        ));
        out.push(command(
            plugin_id,
            &rule.id,
            manifest,
            &dir,
            "cargo build",
            "cargo build",
            RunCommandKind::Build,
        ));
    }
}

fn detect_go(
    scan: &WorkspaceScan,
    plugin_id: &str,
    rule: &RunDetectorRule,
    out: &mut Vec<RunCommand>,
) {
    for manifest in scan.files.iter().filter(|rel| basename(rel) == "go.mod") {
        let dir = directory_of(manifest);
        out.push(command(
            plugin_id,
            &rule.id,
            manifest,
            &dir,
            "go run .",
            "go run .",
            RunCommandKind::Run,
        ));
        out.push(command(
            plugin_id,
            &rule.id,
            manifest,
            &dir,
            "go test ./...",
            "go test ./...",
            RunCommandKind::Test,
        ));
        out.push(command(
            plugin_id,
            &rule.id,
            manifest,
            &dir,
            "go build ./...",
            "go build ./...",
            RunCommandKind::Build,
        ));
    }
}

fn detect_cmake_make(
    scan: &WorkspaceScan,
    plugin_id: &str,
    rule: &RunDetectorRule,
    out: &mut Vec<RunCommand>,
) {
    for manifest in scan
        .files
        .iter()
        .filter(|rel| basename(rel) == "CMakeLists.txt")
    {
        let dir = directory_of(manifest);
        out.push(command(
            plugin_id,
            &rule.id,
            manifest,
            &dir,
            "cmake configure",
            "cmake -S . -B build",
            RunCommandKind::Build,
        ));
        out.push(command(
            plugin_id,
            &rule.id,
            manifest,
            &dir,
            "cmake build",
            "cmake --build build",
            RunCommandKind::Build,
        ));
    }
    for manifest in scan
        .files
        .iter()
        .filter(|rel| matches!(basename(rel), "Makefile" | "makefile" | "GNUmakefile"))
    {
        let dir = directory_of(manifest);
        out.push(command(
            plugin_id,
            &rule.id,
            manifest,
            &dir,
            "make",
            "make",
            RunCommandKind::Build,
        ));
        if make_has_target(scan, manifest, "test") {
            out.push(command(
                plugin_id,
                &rule.id,
                manifest,
                &dir,
                "make test",
                "make test",
                RunCommandKind::Test,
            ));
        }
        if make_has_target(scan, manifest, "run") {
            out.push(command(
                plugin_id,
                &rule.id,
                manifest,
                &dir,
                "make run",
                "make run",
                RunCommandKind::Run,
            ));
        }
    }
}

fn detect_shell_scripts(
    scan: &WorkspaceScan,
    plugin_id: &str,
    rule: &RunDetectorRule,
    out: &mut Vec<RunCommand>,
) {
    const EXTS: &[&str] = &["sh", "bash", "zsh", "fish", "ps1", "bat", "cmd"];
    const STEMS: &[&str] = &["run", "start", "dev", "debug", "test", "build"];
    for rel in &scan.files {
        let ext = extension_of(rel);
        if !EXTS.contains(&ext.as_str()) {
            continue;
        }
        let stem = file_stem(rel).to_ascii_lowercase();
        if !STEMS
            .iter()
            .any(|known| stem == *known || stem.starts_with(&format!("{known}-")))
        {
            continue;
        }
        let dir = directory_of(rel);
        let command_text = script_command(rel);
        out.push(command(
            plugin_id,
            &rule.id,
            rel,
            &dir,
            &stem,
            &command_text,
            RunCommandKind::from_label(&stem),
        ));
    }
}

fn detect_direct_runtime(
    scan: &WorkspaceScan,
    plugin_id: &str,
    rule: &RunDetectorRule,
    out: &mut Vec<RunCommand>,
) {
    for rel in &scan.files {
        let base = basename(rel);
        let dir = directory_of(rel);
        let Some((label, cmd)) = (match base {
            "main.js" | "index.js" => Some((format!("node {base}"), format!("node {base}"))),
            "main.mjs" | "index.mjs" => Some((format!("node {base}"), format!("node {base}"))),
            "main.ts" | "index.ts" => Some((format!("bun {base}"), format!("bun {base}"))),
            "app.js" => Some(("node app.js".into(), "node app.js".into())),
            _ => None,
        }) else {
            continue;
        };
        out.push(command(
            plugin_id,
            &rule.id,
            rel,
            &dir,
            &label,
            &cmd,
            RunCommandKind::Run,
        ));
    }
}

fn stable_dedup(mut commands: Vec<RunCommand>) -> Vec<RunCommand> {
    commands.sort_by(|a, b| {
        a.cwd_rel
            .cmp(&b.cwd_rel)
            .then(a.kind.cmp(&b.kind))
            .then(a.label.cmp(&b.label))
            .then(a.command.cmp(&b.command))
    });
    let mut seen = BTreeSet::new();
    commands
        .into_iter()
        .filter(|cmd| seen.insert((cmd.cwd_rel.clone(), cmd.command.clone())))
        .collect()
}

fn command(
    plugin_id: &str,
    detector_id: &str,
    manifest_path: &str,
    cwd_rel: &str,
    label: &str,
    command_text: &str,
    kind: RunCommandKind,
) -> RunCommand {
    let id = format!(
        "{}:{}:{}:{}",
        plugin_id,
        detector_id,
        if cwd_rel.is_empty() { "." } else { cwd_rel },
        label
    );
    RunCommand {
        id,
        label: label.into(),
        command: command_text.into(),
        cwd_rel: cwd_rel.into(),
        kind,
        source: RunCommandSource {
            plugin_id: plugin_id.into(),
            detector_id: detector_id.into(),
            manifest_path: Some(manifest_path.into()),
            package_path: cwd_rel.into(),
        },
    }
}

fn rule(id: &str, kind: DetectorKind) -> RunDetectorRule {
    RunDetectorRule {
        id: id.into(),
        kind,
        label: String::new(),
    }
}

fn collect_files(
    root: &Path,
    dir: &Path,
    out: &mut BTreeSet<String>,
    depth_left: u8,
) -> Result<(), String> {
    if depth_left == 0 {
        return Ok(());
    }
    let entries = fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if is_ignored_dir(&name) {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out, depth_left - 1)?;
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                out.insert(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".cache" | "vendor"
    )
}

fn validate_detector_id(id: &str) -> Result<(), String> {
    super::types::normalize_plugin_id(id).map(|_| ())
}

fn package_manager(scan: &WorkspaceScan, dir: &str, package_manager: Option<&str>) -> String {
    if let Some(raw) = package_manager {
        let first = raw.split('@').next().unwrap_or(raw).trim();
        if matches!(first, "pnpm" | "bun" | "yarn" | "npm") {
            return first.into();
        }
    }
    let lock = |name: &str| {
        let rel = if dir.is_empty() {
            name.to_string()
        } else {
            format!("{dir}/{name}")
        };
        scan.contains(&rel)
    };
    if lock("pnpm-lock.yaml") {
        "pnpm".into()
    } else if lock("bun.lockb") || lock("bun.lock") {
        "bun".into()
    } else if lock("yarn.lock") {
        "yarn".into()
    } else {
        "npm".into()
    }
}

fn package_script_command(pm: &str, script: &str) -> String {
    match pm {
        "npm" => format!("npm run {script}"),
        "yarn" => format!("yarn {script}"),
        "pnpm" => format!("pnpm {script}"),
        "bun" => format!("bun run {script}"),
        _ => format!("{pm} run {script}"),
    }
}

fn script_rank(name: &str) -> (u8, String) {
    let rank = match name {
        "dev" => 0,
        "start" => 1,
        "debug" => 2,
        "test" => 3,
        "build" => 4,
        "preview" => 5,
        _ => 9,
    };
    (rank, name.to_string())
}

fn make_has_target(scan: &WorkspaceScan, manifest: &str, target: &str) -> bool {
    scan.read_text(manifest)
        .map(|raw| {
            raw.lines()
                .any(|line| line.starts_with(&format!("{target}:")))
        })
        .unwrap_or(false)
}

fn script_command(rel: &str) -> String {
    match extension_of(rel).as_str() {
        "ps1" => format!("powershell -ExecutionPolicy Bypass -File {}", basename(rel)),
        "bat" | "cmd" => basename(rel).to_string(),
        _ => format!("./{}", basename(rel)),
    }
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn directory_of(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .unwrap_or_default()
}

fn extension_of(path: &str) -> String {
    basename(path)
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

fn file_stem(path: &str) -> String {
    basename(path)
        .rsplit_once('.')
        .map(|(stem, _)| stem.to_string())
        .unwrap_or_else(|| basename(path).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(files: &[(&str, &str)]) -> WorkspaceScan {
        let dir = std::env::temp_dir().join(format!(
            "blx_run_detectors_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (rel, body) in files {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        }
        WorkspaceScan::from_root(dir).unwrap()
    }

    #[test]
    fn parses_detector_file() {
        let raw = r#"{"detectors":[{"id":"node-scripts","kind":"packageJsonScripts"}]}"#;
        let parsed = parse_detector_file(raw).unwrap();
        assert_eq!(parsed.detectors[0].kind, DetectorKind::PackageJsonScripts);
    }

    #[test]
    fn node_scripts_use_package_manager_and_priority() {
        let scan = scan(&[
            (
                "package.json",
                r#"{"packageManager":"pnpm@9.0.0","scripts":{"build":"vite build","dev":"vite","test":"vitest"}}"#,
            ),
            ("pnpm-lock.yaml", ""),
        ]);
        let cmds = discover_for_plugin(
            &scan,
            "runtime-node",
            &built_in_detector_rules("runtime-node"),
        );
        assert_eq!(cmds[0].command, "pnpm dev");
        assert!(cmds.iter().any(|cmd| cmd.command == "pnpm build"));
    }

    #[test]
    fn cargo_go_and_make_commands_are_detected() {
        let scan = scan(&[
            ("crates/app/Cargo.toml", "[package]\nname = \"app\"\n"),
            ("go.mod", "module example.com/app\n"),
            ("Makefile", "run:\n\t./app\ntest:\n\ttrue\n"),
        ]);
        let mut cmds = Vec::new();
        cmds.extend(discover_for_plugin(
            &scan,
            "runtime-rust",
            &built_in_detector_rules("runtime-rust"),
        ));
        cmds.extend(discover_for_plugin(
            &scan,
            "runtime-go",
            &built_in_detector_rules("runtime-go"),
        ));
        cmds.extend(discover_for_plugin(
            &scan,
            "runtime-c-cpp",
            &built_in_detector_rules("runtime-c-cpp"),
        ));
        assert!(cmds
            .iter()
            .any(|cmd| cmd.command == "cargo run" && cmd.cwd_rel == "crates/app"));
        assert!(cmds.iter().any(|cmd| cmd.command == "go test ./..."));
        assert!(cmds.iter().any(|cmd| cmd.command == "make run"));
    }

    #[test]
    fn shell_and_direct_runtime_commands_are_detected() {
        let scan = scan(&[
            ("scripts/dev.sh", "#!/bin/sh\n"),
            ("tools/test.ps1", ""),
            ("apps/web/index.ts", ""),
        ]);
        let mut cmds = Vec::new();
        cmds.extend(discover_for_plugin(
            &scan,
            "runtime-shell",
            &built_in_detector_rules("runtime-shell"),
        ));
        cmds.extend(discover_for_plugin(
            &scan,
            "runtime-direct",
            &built_in_detector_rules("runtime-direct"),
        ));
        assert!(cmds
            .iter()
            .any(|cmd| cmd.command == "./dev.sh" && cmd.cwd_rel == "scripts"));
        assert!(cmds.iter().any(|cmd| cmd.command.contains("test.ps1")));
        assert!(cmds.iter().any(|cmd| cmd.command == "bun index.ts"));
    }
}
