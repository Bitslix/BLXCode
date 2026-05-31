//! Sandboxed directory listing for the sidebar project explorer.

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, State};

use crate::pty_host::{sh_quote, PtyManager};
use crate::ssh_exec::{RemoteExecManager, EXEC_TIMEOUT_MS};

const MAX_TEXT_PREVIEW_BYTES: u64 = 512 * 1024;
const MAX_IMAGE_PREVIEW_BYTES: u64 = 16 * 1024 * 1024;
const MAX_VIDEO_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;
/// Upper bound on entries returned by [`list_workspace_files`] so the fuzzy
/// file finder stays responsive on huge trees.
const MAX_FILE_INDEX: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntryBrief {
    pub name: String,
    pub is_dir: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFilePreview {
    pub content: String,
    pub truncated: bool,
    pub byte_len: u64,
    /// Modification timestamp (Unix ms) when available. Used by the editor as
    /// one input to the save-time conflict check. Remote reads currently omit
    /// this (`None`) because portable remote mtime is unavailable.
    pub modified_ms: Option<i64>,
    /// Fast non-cryptographic content hash (FNV-1a, hex) of the raw bytes.
    /// Primary signal for the save-time conflict guard.
    pub hash: String,
}

/// Result of a successful [`write_workspace_text_file`]. Lets the frontend
/// reset its conflict baseline without re-reading the file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult {
    pub modified_ms: Option<i64>,
    pub hash: String,
    pub byte_len: u64,
}

/// Marker prefix on the error string returned when a save is refused because
/// the on-disk content changed since it was read. The frontend matches this
/// prefix to surface the conflict dialog instead of a generic error toast.
pub const CONFLICT_PREFIX: &str = "conflict:";

/// Path components that are never writable in-app: VCS internals, the agent
/// memory/skills store, build outputs and dependency trees. Matched
/// case-sensitively against every component of the relative path.
const PROTECTED_COMPONENTS: &[&str] = &[
    ".git",
    ".agents",
    ".blxcode",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".cache",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
    "coverage",
];

/// FNV-1a 64-bit hash of `bytes`, rendered as lowercase hex. Cheap, dependency
/// free, and good enough to detect an out-of-band change for the conflict guard
/// (not used for security).
fn content_hash(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// `true` when any component of the relative path matches a protected folder.
/// Operates on the caller-supplied relative string so it works identically for
/// local and remote writes.
fn is_protected_rel(rel: &str) -> bool {
    rel.split(['/', '\\'])
        .any(|c| PROTECTED_COMPONENTS.contains(&c))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Image,
    Video,
    Markdown,
    Mermaid,
    Code,
    Text,
    Binary,
}

/// Repository "policy" documents — these typically ship without an extension
/// (e.g. `LICENSE`, `CONTRIBUTING`) but are conventionally rendered as
/// Markdown. The frontend renders a hero banner above the body so the
/// document's role is immediately obvious.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyKind {
    License,
    Contributing,
    Contributors,
    CodeOfConduct,
    Security,
    Authors,
    Changelog,
    Readme,
    Support,
    Agents,
    Claude,
    Codex,
    Gemini,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub name: String,
    pub rel_path: String,
    pub byte_len: u64,
    pub modified_ms: Option<i64>,
    pub kind: FileKind,
    pub mime: Option<String>,
    /// Set when the file's stem matches a well-known repository policy
    /// document (`LICENSE`, `CONTRIBUTING`, `CONTRIBUTORS`, …) — applies
    /// regardless of whether the file ships with `.md` / `.markdown` or
    /// without any extension at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_kind: Option<PolicyKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryFilePreview {
    pub base64: String,
    pub mime: String,
    pub byte_len: u64,
    pub truncated: bool,
}

/// Lowercased extension or empty string for files without a suffix.
fn ext_lower(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Lowercased file stem (filename without extension) — empty string if the
/// path has no file name component.
fn stem_lower(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Classify a repository policy document by its filename stem (case-insensitive).
/// Matches both the bare filename (`LICENSE`) and the `.md` / `.markdown`
/// variant (`LICENSE.md`) since the caller always passes the stem.
fn classify_policy(stem: &str) -> Option<PolicyKind> {
    match stem {
        "license" | "licence" | "copying" | "copyright" | "unlicense" => Some(PolicyKind::License),
        "contributing" | "contribution" | "contributions" => Some(PolicyKind::Contributing),
        "contributors" | "contributer" | "contributers" => Some(PolicyKind::Contributors),
        "code_of_conduct" | "code-of-conduct" | "codeofconduct" => Some(PolicyKind::CodeOfConduct),
        "security" | "security-policy" | "security_policy" => Some(PolicyKind::Security),
        "authors" | "maintainers" | "owners" | "codeowners" => Some(PolicyKind::Authors),
        "changelog" | "changes" | "history" | "release_notes" | "release-notes"
        | "releasenotes" => Some(PolicyKind::Changelog),
        "readme" => Some(PolicyKind::Readme),
        "support" => Some(PolicyKind::Support),
        "agents" | "agent" => Some(PolicyKind::Agents),
        "claude" => Some(PolicyKind::Claude),
        "codex" => Some(PolicyKind::Codex),
        "gemini" => Some(PolicyKind::Gemini),
        "copilot" | "windsurf" | "aider" | "cursor" | "cody" | "devin" | "junie" | "continue" => {
            Some(PolicyKind::Agents)
        }
        _ => None,
    }
}

/// Maps a lowercased extension to a [`FileKind`] used by the preview dispatcher.
fn classify_kind(ext: &str) -> FileKind {
    match ext {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "avif" | "bmp" | "ico" | "svg" => FileKind::Image,
        "mp4" | "webm" | "mov" | "m4v" | "mkv" => FileKind::Video,
        "md" | "markdown" => FileKind::Markdown,
        "mmd" | "mermaid" => FileKind::Mermaid,
        // Source code with syntax highlighting.
        "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "py" | "pyw" | "pyi" | "go"
        | "java" | "kt" | "kts" | "scala" | "groovy" | "gradle" | "swift" | "m" | "mm" | "c"
        | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "cs" | "fs" | "fsx" | "vb" | "rb"
        | "erb" | "php" | "phtml" | "lua" | "pl" | "pm" | "dart" | "r" | "jl" | "clj" | "cljs"
        | "cljc" | "edn" | "ex" | "exs" | "eex" | "erl" | "hrl" | "hs" | "lhs" | "purs" | "elm"
        | "nim" | "zig" | "ml" | "mli" | "ocaml" | "html" | "htm" | "xhtml" | "vue" | "svelte"
        | "css" | "scss" | "sass" | "less" | "styl" | "json" | "json5" | "jsonc" | "toml"
        | "yaml" | "yml" | "xml" | "plist" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat"
        | "cmd" | "sql" | "graphql" | "gql" | "proto" | "thrift" | "tf" | "tfvars" | "hcl"
        | "nix" | "dockerfile" | "containerfile" | "makefile" | "mk" | "cmake" | "diff"
        | "patch" => FileKind::Code,
        // Plain text without highlighting (still gets line numbers in the preview).
        "txt" | "log" | "ini" | "conf" | "cfg" | "env" | "properties" | "lock"
        | "gitattributes" | "editorconfig" | "csv" | "tsv" => FileKind::Text,
        _ => FileKind::Binary,
    }
}

/// Best-effort MIME guess from the lowercased extension.
fn mime_for_ext(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "md" | "markdown" => "text/markdown",
        "mmd" | "mermaid" => "text/vnd.mermaid",
        "json" | "json5" | "jsonc" => "application/json",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" | "cjs" => "text/javascript",
        "ts" | "tsx" => "application/typescript",
        "xml" => "application/xml",
        "toml" => "application/toml",
        "yaml" | "yml" => "application/yaml",
        "txt" | "log" | "ini" | "conf" | "env" => "text/plain",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        _ => return None,
    })
}

fn modified_ms(meta: &fs::Metadata) -> Option<i64> {
    let modified = meta.modified().ok()?;
    let dur = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(dur.as_millis()).ok()
}

fn canonical_root(workspace_root: &str) -> Result<PathBuf, String> {
    let trimmed = workspace_root.trim();
    if trimmed.is_empty() {
        return Err("workspace root is empty".into());
    }
    let p = PathBuf::from(trimmed);
    if !p.is_dir() {
        return Err("workspace root is not a directory".into());
    }
    fs::canonicalize(&p).map_err(|e| format!("canonicalize workspace: {e}"))
}

/// Resolve a target path that does **not** exist yet (used for create
/// operations). [`resolve_under_root`] cannot be reused because it
/// `canonicalize`s the full path, which fails when the target is missing.
///
/// Rejects absolute paths and any `..` component so the result cannot escape
/// the (already canonicalized) `root`. Symlinked parents are defended against
/// by the caller, which canonicalizes the deepest existing ancestor and
/// re-checks containment before writing.
fn resolve_new_under_root(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim();
    if rel.is_empty() {
        return Err("name is empty".into());
    }
    let p = PathBuf::from(rel);
    if p.is_absolute() {
        return Err("path must be relative".into());
    }
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("path escapes workspace".into());
    }
    Ok(root.join(&p))
}

/// Create `dir` (and missing parents) then verify the canonicalized result
/// stays under `root` — guards against a symlinked parent redirecting the
/// write outside the workspace.
fn ensure_under_root(root: &Path, path: &Path) -> Result<(), String> {
    let canon = fs::canonicalize(path).map_err(|e| format!("canonicalize: {e}"))?;
    if !canon.starts_with(root) {
        return Err("path outside workspace".into());
    }
    Ok(())
}

/// Creates an empty file under `workspace_root`. Fails if it already exists.
/// Missing parent directories are created. Sandboxed to the workspace root.
#[tauri::command]
pub fn create_workspace_file(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<(), String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_create_file(&app, &pty, &exec, cid, &workspace_root, &path);
    }
    local_create_file(&workspace_root, &path)
}

fn local_create_file(workspace_root: &str, path: &str) -> Result<(), String> {
    let root = canonical_root(workspace_root)?;
    let target = resolve_new_under_root(&root, path)?;
    if target.exists() {
        return Err("a file or folder with that name already exists".into());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create parent: {e}"))?;
        ensure_under_root(&root, parent)?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| format!("create file: {e}"))?;
    Ok(())
}

/// Creates an empty directory (and missing parents) under `workspace_root`.
/// Fails if it already exists. Sandboxed to the workspace root.
#[tauri::command]
pub fn create_workspace_dir(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<(), String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_create_dir(&app, &pty, &exec, cid, &workspace_root, &path);
    }
    local_create_dir(&workspace_root, &path)
}

fn local_create_dir(workspace_root: &str, path: &str) -> Result<(), String> {
    let root = canonical_root(workspace_root)?;
    let target = resolve_new_under_root(&root, path)?;
    if target.exists() {
        return Err("a file or folder with that name already exists".into());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create parent: {e}"))?;
        ensure_under_root(&root, parent)?;
    }
    fs::create_dir(&target).map_err(|e| format!("create dir: {e}"))?;
    Ok(())
}

fn resolve_under_root(root: &Path, rel_or_abs: &str) -> Result<PathBuf, String> {
    let target = if rel_or_abs.trim().is_empty() {
        root.to_path_buf()
    } else {
        let p = PathBuf::from(rel_or_abs);
        if p.is_absolute() {
            p
        } else {
            root.join(p)
        }
    };
    let canon = fs::canonicalize(&target).map_err(|e| format!("path not found: {e}"))?;
    if !canon.starts_with(root) {
        return Err("path outside workspace".into());
    }
    Ok(canon)
}

// ---------------------------------------------------------------------------
// Remote (SSH) variants — run over the per-connection exec channel.
//
// The remote sandbox is weaker than the local one (no `canonicalize`): paths
// are kept relative under the workspace root via string rules + a shell prefix
// guard, but symlinks on the remote are not resolved. This is documented and
// accepted (see the plan).
// ---------------------------------------------------------------------------

/// Join a caller-supplied `path` under the remote workspace `root`, rejecting
/// absolute paths and `..` traversal. The frontend only ever passes the root
/// itself or a relative path, so absolute inputs (other than the root) are
/// refused rather than canonicalized.
fn remote_target(root: &str, path: &str) -> Result<String, String> {
    let root = root.trim().trim_end_matches('/');
    if root.is_empty() {
        return Err("workspace root is empty".into());
    }
    let p = path.trim();
    if p.is_empty() || p == root || p == format!("{root}/") {
        return Ok(root.to_string());
    }
    if p.starts_with('/') {
        return Err("path must be relative".into());
    }
    if p.split(['/', '\\']).any(|c| c == "..") {
        return Err("path escapes workspace".into());
    }
    Ok(format!("{root}/{}", p.trim_start_matches('/')))
}

fn remote_list_path_entries(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
    path: &str,
) -> Result<Vec<FsEntryBrief>, String> {
    let dir = remote_target(workspace_root, path)?;
    // `-p` suffixes directories with `/`; `-A` excludes `.`/`..`.
    let cmd = format!("LC_ALL=C ls -Ap1 -- {}", sh_quote(&dir));
    let stdout = exec.run_text(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
    let mut out: Vec<FsEntryBrief> = stdout
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.is_empty() && *l != "./" && *l != "../")
        .map(|line| {
            let is_dir = line.ends_with('/');
            let name = line.trim_end_matches('/').to_string();
            let hidden = name.starts_with('.');
            FsEntryBrief {
                name,
                is_dir,
                hidden,
            }
        })
        .filter(|e| e.name != "." && e.name != "..")
        .collect();
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase()),
    });
    Ok(out)
}

fn remote_byte_len(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    target: &str,
) -> Result<u64, String> {
    let cmd = format!("wc -c < {}", sh_quote(target));
    let stdout = exec.run_text(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
    stdout
        .trim()
        .parse::<u64>()
        .map_err(|_| "could not read remote file size".to_string())
}

fn remote_read_text_file(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
    path: &str,
) -> Result<TextFilePreview, String> {
    let target = remote_target(workspace_root, path)?;
    let byte_len = remote_byte_len(app, pty, exec, connection_id, &target)?;
    let cmd = format!("head -c {MAX_TEXT_PREVIEW_BYTES} -- {}", sh_quote(&target));
    let out = exec.run(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
    if !out.ok() {
        return Err(out.stderr_string());
    }
    let truncated = byte_len > MAX_TEXT_PREVIEW_BYTES;
    let hash = content_hash(&out.stdout);
    let content =
        String::from_utf8(out.stdout).map_err(|_| "file is not valid UTF-8 text".to_string())?;
    Ok(TextFilePreview {
        content,
        truncated,
        byte_len,
        modified_ms: None, // portable remote mtime omitted; best-effort only
        hash,
    })
}

fn remote_stat_file(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
    path: &str,
) -> Result<FileMeta, String> {
    let target = remote_target(workspace_root, path)?;
    let byte_len = remote_byte_len(app, pty, exec, connection_id, &target)?;
    let tp = Path::new(path);
    let ext = ext_lower(tp);
    let stem = stem_lower(tp);
    let policy_kind = classify_policy(&stem);
    let kind = if policy_kind.is_some() {
        FileKind::Markdown
    } else {
        classify_kind(&ext)
    };
    let mime = mime_for_ext(&ext)
        .map(str::to_string)
        .or_else(|| policy_kind.map(|_| "text/markdown".to_string()));
    let name = tp
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    Ok(FileMeta {
        name,
        rel_path: path.to_string(),
        byte_len,
        modified_ms: None, // portable remote mtime omitted; best-effort only
        kind,
        mime,
        policy_kind,
    })
}

fn remote_read_binary(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
    path: &str,
    cap: u64,
    require: FileKind,
    not_kind_err: &str,
) -> Result<BinaryFilePreview, String> {
    let target = remote_target(workspace_root, path)?;
    let ext = ext_lower(Path::new(path));
    if classify_kind(&ext) != require {
        return Err(not_kind_err.into());
    }
    let byte_len = remote_byte_len(app, pty, exec, connection_id, &target)?;
    let cmd = format!("head -c {cap} -- {}", sh_quote(&target));
    let out = exec.run(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
    if !out.ok() {
        return Err(out.stderr_string());
    }
    let truncated = byte_len > cap;
    let mime = mime_for_ext(&ext)
        .map(str::to_string)
        .unwrap_or_else(|| "application/octet-stream".to_string());
    Ok(BinaryFilePreview {
        base64: BASE64_STANDARD.encode(&out.stdout),
        mime,
        byte_len,
        truncated,
    })
}

fn remote_create_file(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
    path: &str,
) -> Result<(), String> {
    let target = remote_target(workspace_root, path)?;
    let q = sh_quote(&target);
    // Create parents, then noclobber-create in a subshell so `set -C` doesn't
    // leak into the persistent exec shell.
    let cmd = format!("mkdir -p -- \"$(dirname -- {q})\" && ( set -C; : > {q} )");
    exec.run_check(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)
        .map_err(|e| {
            if e.contains("cannot overwrite") || e.contains("exists") {
                "a file or folder with that name already exists".to_string()
            } else {
                e
            }
        })
}

fn remote_create_dir(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
    path: &str,
) -> Result<(), String> {
    let target = remote_target(workspace_root, path)?;
    let q = sh_quote(&target);
    // `mkdir` without `-p` fails if the leaf already exists.
    let cmd = format!("mkdir -p -- \"$(dirname -- {q})\" && mkdir -- {q}");
    exec.run_check(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)
        .map_err(|e| {
            if e.contains("File exists") || e.contains("exists") {
                "a file or folder with that name already exists".to_string()
            } else {
                e
            }
        })
}

/// Lists files and directories under `path`, constrained to `workspace_root`.
#[tauri::command]
pub fn list_path_entries(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<Vec<FsEntryBrief>, String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_list_path_entries(&app, &pty, &exec, cid, &workspace_root, &path);
    }
    local_list_path_entries(&workspace_root, &path)
}

fn local_list_path_entries(workspace_root: &str, path: &str) -> Result<Vec<FsEntryBrief>, String> {
    let root = canonical_root(workspace_root)?;
    let dir = resolve_under_root(&root, path)?;
    if !dir.is_dir() {
        return Err("not a directory".into());
    }
    let read = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    let mut out: Vec<FsEntryBrief> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let ft = e.file_type().ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "." || name == ".." {
                return None;
            }
            let hidden = name.starts_with('.');
            Some(FsEntryBrief {
                name,
                is_dir: ft.is_dir(),
                hidden,
            })
        })
        .collect();
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a
            .name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase()),
    });
    Ok(out)
}

/// Recursively lists files under `workspace_root` (relative paths, `/`-joined,
/// sorted) for the fuzzy file finder. Skips [`PROTECTED_COMPONENTS`] directories
/// (`.git`, `node_modules`, `target`, …) and caps the result at
/// [`MAX_FILE_INDEX`] entries.
#[tauri::command]
pub fn list_workspace_files(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    connection_id: Option<String>,
) -> Result<Vec<String>, String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_list_workspace_files(&app, &pty, &exec, cid, &workspace_root);
    }
    local_list_workspace_files(&workspace_root)
}

fn local_list_workspace_files(workspace_root: &str) -> Result<Vec<String>, String> {
    let root = canonical_root(workspace_root)?;
    let mut out: Vec<String> = Vec::new();
    // Iterative DFS. `file_type()` does not follow symlinks, so symlinked
    // directories report as symlinks (not dirs) and are skipped — no cycles.
    let mut stack: Vec<PathBuf> = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        if out.len() >= MAX_FILE_INDEX {
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if PROTECTED_COMPONENTS.contains(&name.as_ref()) {
                    continue;
                }
                stack.push(entry.path());
            } else if ft.is_file() {
                if out.len() >= MAX_FILE_INDEX {
                    break;
                }
                if let Ok(rel) = entry.path().strip_prefix(&root) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    out.sort();
    out.truncate(MAX_FILE_INDEX);
    Ok(out)
}

fn remote_list_workspace_files(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
) -> Result<Vec<String>, String> {
    let target = remote_target(workspace_root, "")?;
    let prunes = PROTECTED_COMPONENTS
        .iter()
        .map(|c| format!("-name {}", sh_quote(c)))
        .collect::<Vec<_>>()
        .join(" -o ");
    // Prune protected dirs, print files, cap with head.
    let cmd = format!(
        "find {} \\( {} \\) -prune -o -type f -print 2>/dev/null | head -n {}",
        sh_quote(&target),
        prunes,
        MAX_FILE_INDEX
    );
    let stdout = exec.run_text(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
    let prefix = format!("{}/", target.trim_end_matches('/'));
    let mut out: Vec<String> = stdout
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter_map(|l| l.strip_prefix(&prefix))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    out.sort();
    out.truncate(MAX_FILE_INDEX);
    Ok(out)
}

/// Reads a UTF-8 text file under `workspace_root` for the center preview tab.
#[tauri::command]
pub fn read_workspace_text_file(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<TextFilePreview, String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_read_text_file(&app, &pty, &exec, cid, &workspace_root, &path);
    }
    local_read_text_file(&workspace_root, &path)
}

fn local_read_text_file(workspace_root: &str, path: &str) -> Result<TextFilePreview, String> {
    let root = canonical_root(workspace_root)?;
    let file = resolve_under_root(&root, path)?;
    if !file.is_file() {
        return Err("not a file".into());
    }
    let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
    let byte_len = meta.len();
    let mut bytes = fs::read(&file).map_err(|e| e.to_string())?;
    let truncated = byte_len > MAX_TEXT_PREVIEW_BYTES;
    if truncated {
        bytes.truncate(MAX_TEXT_PREVIEW_BYTES as usize);
    }
    let modified = modified_ms(&meta);
    let hash = content_hash(&bytes);
    let content =
        String::from_utf8(bytes).map_err(|_| "file is not valid UTF-8 text".to_string())?;
    Ok(TextFilePreview {
        content,
        truncated,
        byte_len,
        modified_ms: modified,
        hash,
    })
}

/// Writes UTF-8 `content` over an existing file under `workspace_root`.
///
/// Refuses files in [`PROTECTED_COMPONENTS`]. When `expected_hash` is `Some`
/// and the current on-disk hash differs, the write is refused with a
/// [`CONFLICT_PREFIX`]-tagged error and the file is left untouched. The write
/// itself is atomic (temp sibling + rename). Returns a fresh baseline so the
/// frontend can clear its dirty/conflict state without reloading.
#[tauri::command]
pub fn write_workspace_text_file(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    content: String,
    expected_hash: Option<String>,
    connection_id: Option<String>,
) -> Result<WriteResult, String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_write_text_file(
            &app,
            &pty,
            &exec,
            cid,
            &workspace_root,
            &path,
            &content,
            expected_hash.as_deref(),
        );
    }
    local_write_text_file(&workspace_root, &path, &content, expected_hash.as_deref())
}

fn local_write_text_file(
    workspace_root: &str,
    path: &str,
    content: &str,
    expected_hash: Option<&str>,
) -> Result<WriteResult, String> {
    if is_protected_rel(path) {
        return Err("this file is in a protected folder and can't be edited here".into());
    }
    let root = canonical_root(workspace_root)?;
    // The file must already exist — the editor only saves opened documents.
    let file = resolve_under_root(&root, path)?;
    if !file.is_file() {
        return Err("not a file".into());
    }
    // Conflict guard: compare the caller's expected hash against the current
    // bytes on disk. `None` means "force write" (after explicit user consent).
    if let Some(expected) = expected_hash {
        let current = fs::read(&file).map_err(|e| e.to_string())?;
        let current_hash = content_hash(&current);
        if current_hash != expected {
            return Err(format!("{CONFLICT_PREFIX}file changed on disk"));
        }
    }
    let parent = file
        .parent()
        .ok_or_else(|| "file has no parent directory".to_string())?;
    // Atomic write: write a sibling temp file, then rename over the target so a
    // partial write can never leave a corrupt file in place.
    let tmp = parent.join(format!(".blxcode-write-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, content.as_bytes()).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("write temp: {e}")
    })?;
    if let Err(e) = ensure_under_root(&root, &tmp) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, &file) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("rename: {e}"));
    }
    let byte_len = content.len() as u64;
    let hash = content_hash(content.as_bytes());
    let modified = fs::metadata(&file).ok().and_then(|m| modified_ms(&m));
    Ok(WriteResult {
        modified_ms: modified,
        hash,
        byte_len,
    })
}

#[allow(clippy::too_many_arguments)]
fn remote_write_text_file(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
    path: &str,
    content: &str,
    expected_hash: Option<&str>,
) -> Result<WriteResult, String> {
    if is_protected_rel(path) {
        return Err("this file is in a protected folder and can't be edited here".into());
    }
    let target = remote_target(workspace_root, path)?;
    let q = sh_quote(&target);
    // Conflict guard: re-read the remote bytes (capped, same as the read path)
    // and compare hashes before writing. Best-effort — if the read fails we
    // surface the error rather than blindly overwriting.
    if let Some(expected) = expected_hash {
        let read_cmd = format!("head -c {MAX_TEXT_PREVIEW_BYTES} -- {q}");
        let out = exec.run(app, pty, connection_id, &read_cmd, EXEC_TIMEOUT_MS)?;
        if !out.ok() {
            return Err(out.stderr_string());
        }
        if content_hash(&out.stdout) != expected {
            return Err(format!("{CONFLICT_PREFIX}file changed on disk"));
        }
    }
    // base64-encode the new content and decode it remotely into a temp sibling,
    // then move it over the target so the write is atomic on the remote too.
    let b64 = BASE64_STANDARD.encode(content.as_bytes());
    let tmp = format!("{target}.blxcode-write.tmp");
    let qtmp = sh_quote(&tmp);
    let cmd = format!(
        "printf %s {} | base64 -d > {qtmp} && mv -f -- {qtmp} {q}",
        sh_quote(&b64)
    );
    exec.run_check(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)
        .map_err(|e| format!("remote write failed: {e}"))?;
    let byte_len = content.len() as u64;
    let hash = content_hash(content.as_bytes());
    Ok(WriteResult {
        modified_ms: None,
        hash,
        byte_len,
    })
}

/// Lightweight metadata for the file preview topbar.
/// Returns name, relative path (as supplied by the caller), byte size,
/// modification timestamp (Unix ms, if available), classified [`FileKind`]
/// and a best-effort MIME guess. Errors mirror the existing sandbox path.
#[tauri::command]
pub fn stat_workspace_file(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<FileMeta, String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_stat_file(&app, &pty, &exec, cid, &workspace_root, &path);
    }
    local_stat_file(&workspace_root, &path)
}

fn local_stat_file(workspace_root: &str, path: &str) -> Result<FileMeta, String> {
    let root = canonical_root(workspace_root)?;
    let file = resolve_under_root(&root, path)?;
    if !file.is_file() {
        return Err("not a file".into());
    }
    let meta = fs::metadata(&file).map_err(|e| e.to_string())?;
    let ext = ext_lower(&file);
    let stem = stem_lower(&file);
    let policy_kind = classify_policy(&stem);
    // Policy docs are rendered as Markdown regardless of extension so a bare
    // `LICENSE` (no `.md`) still gets the rich preview.
    let base_kind = classify_kind(&ext);
    let kind = if policy_kind.is_some() {
        FileKind::Markdown
    } else {
        base_kind
    };
    let mime = mime_for_ext(&ext)
        .map(str::to_string)
        .or_else(|| policy_kind.map(|_| "text/markdown".to_string()));
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    Ok(FileMeta {
        name,
        rel_path: path.to_string(),
        byte_len: meta.len(),
        modified_ms: modified_ms(&meta),
        kind,
        mime,
        policy_kind,
    })
}

fn read_binary_with_cap(file: &Path, cap: u64) -> Result<BinaryFilePreview, String> {
    if !file.is_file() {
        return Err("not a file".into());
    }
    let meta = fs::metadata(file).map_err(|e| e.to_string())?;
    let byte_len = meta.len();
    let truncated = byte_len > cap;
    let mut bytes = fs::read(file).map_err(|e| e.to_string())?;
    if truncated {
        bytes.truncate(cap as usize);
    }
    let ext = ext_lower(file);
    let mime = mime_for_ext(&ext)
        .map(str::to_string)
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let base64 = BASE64_STANDARD.encode(&bytes);
    Ok(BinaryFilePreview {
        base64,
        mime,
        byte_len,
        truncated,
    })
}

/// Reads an image file under `workspace_root` and returns it as base64 plus
/// an extension-derived MIME. Capped at [`MAX_IMAGE_PREVIEW_BYTES`].
#[tauri::command]
pub fn read_workspace_image_file(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<BinaryFilePreview, String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_read_binary(
            &app,
            &pty,
            &exec,
            cid,
            &workspace_root,
            &path,
            MAX_IMAGE_PREVIEW_BYTES,
            FileKind::Image,
            "not an image file",
        );
    }
    local_read_image_file(&workspace_root, &path)
}

fn local_read_image_file(workspace_root: &str, path: &str) -> Result<BinaryFilePreview, String> {
    let root = canonical_root(workspace_root)?;
    let file = resolve_under_root(&root, path)?;
    let ext = ext_lower(&file);
    if !matches!(classify_kind(&ext), FileKind::Image) {
        return Err("not an image file".into());
    }
    read_binary_with_cap(&file, MAX_IMAGE_PREVIEW_BYTES)
}

/// Reads a video file under `workspace_root` and returns it as base64 plus
/// an extension-derived MIME. Capped at [`MAX_VIDEO_PREVIEW_BYTES`].
#[tauri::command]
pub fn read_workspace_video_file(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<BinaryFilePreview, String> {
    if let Some(cid) = connection_id.as_deref() {
        return remote_read_binary(
            &app,
            &pty,
            &exec,
            cid,
            &workspace_root,
            &path,
            MAX_VIDEO_PREVIEW_BYTES,
            FileKind::Video,
            "not a video file",
        );
    }
    local_read_video_file(&workspace_root, &path)
}

fn local_read_video_file(workspace_root: &str, path: &str) -> Result<BinaryFilePreview, String> {
    let root = canonical_root(workspace_root)?;
    let file = resolve_under_root(&root, path)?;
    let ext = ext_lower(&file);
    if !matches!(classify_kind(&ext), FileKind::Video) {
        return Err("not a video file".into());
    }
    read_binary_with_cap(&file, MAX_VIDEO_PREVIEW_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn list_path_entries_sorts_dirs_first() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("z.txt"), b"").unwrap();
        fs::create_dir_all(tmp.join("a_dir")).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let entries = local_list_path_entries(&root, &root).unwrap();
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "a_dir");
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_text_file_reads_under_root() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("hello.txt"), b"hello").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let preview = local_read_text_file(&root, "hello.txt").unwrap();
        assert_eq!(preview.content, "hello");
        assert!(!preview.truncated);
        assert_eq!(preview.byte_len, 5);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_text_file_rejects_outside_root() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let outside = std::env::temp_dir().join(format!("blx_fs_out_{}", uuid::Uuid::new_v4()));
        fs::write(&outside, b"outside").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_read_text_file(&root, &outside.to_string_lossy())
            .expect_err("outside path should fail");
        assert!(err.contains("outside workspace"));
        let _ = fs::remove_dir_all(tmp);
        let _ = fs::remove_file(outside);
    }

    #[test]
    fn read_workspace_text_file_rejects_directories() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(tmp.join("dir")).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_read_text_file(&root, "dir").expect_err("directory should fail");
        assert_eq!(err, "not a file");
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_text_file_handles_missing_files() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_read_text_file(&root, "missing.txt").expect_err("missing");
        assert!(err.contains("path not found"));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn classify_policy_matches_known_stems() {
        assert!(matches!(
            classify_policy("license"),
            Some(PolicyKind::License)
        ));
        assert!(matches!(
            classify_policy("licence"),
            Some(PolicyKind::License)
        ));
        assert!(matches!(
            classify_policy("copying"),
            Some(PolicyKind::License)
        ));
        assert!(matches!(
            classify_policy("contributing"),
            Some(PolicyKind::Contributing)
        ));
        assert!(matches!(
            classify_policy("contributions"),
            Some(PolicyKind::Contributing)
        ));
        assert!(matches!(
            classify_policy("contributors"),
            Some(PolicyKind::Contributors)
        ));
        assert!(matches!(
            classify_policy("code_of_conduct"),
            Some(PolicyKind::CodeOfConduct)
        ));
        assert!(matches!(
            classify_policy("security"),
            Some(PolicyKind::Security)
        ));
        assert!(matches!(
            classify_policy("changelog"),
            Some(PolicyKind::Changelog)
        ));
        assert!(matches!(
            classify_policy("readme"),
            Some(PolicyKind::Readme)
        ));
        assert_eq!(classify_policy("support"), Some(PolicyKind::Support));
        assert_eq!(classify_policy("agents"), Some(PolicyKind::Agents));
        assert_eq!(classify_policy("claude"), Some(PolicyKind::Claude));
        assert_eq!(classify_policy("codex"), Some(PolicyKind::Codex));
        assert_eq!(classify_policy("copilot"), Some(PolicyKind::Agents));
        assert!(classify_policy("random").is_none());
    }

    #[test]
    fn stat_workspace_file_marks_extensionless_license_as_markdown() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("LICENSE"), b"MIT License\n\nCopyright").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let meta = local_stat_file(&root, "LICENSE").unwrap();
        assert!(matches!(meta.kind, FileKind::Markdown));
        assert!(matches!(meta.policy_kind, Some(PolicyKind::License)));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn stat_workspace_file_marks_contributing_md_as_policy() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("CONTRIBUTING.md"), b"# Guide").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let meta = local_stat_file(&root, "CONTRIBUTING.md").unwrap();
        assert!(matches!(meta.kind, FileKind::Markdown));
        assert!(matches!(meta.policy_kind, Some(PolicyKind::Contributing)));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn classify_kind_covers_expected_extensions() {
        assert!(matches!(classify_kind("png"), FileKind::Image));
        assert!(matches!(classify_kind("svg"), FileKind::Image));
        assert!(matches!(classify_kind("mp4"), FileKind::Video));
        assert!(matches!(classify_kind("md"), FileKind::Markdown));
        assert!(matches!(classify_kind("markdown"), FileKind::Markdown));
        assert!(matches!(classify_kind("mmd"), FileKind::Mermaid));
        assert!(matches!(classify_kind("mermaid"), FileKind::Mermaid));
        // Programming languages go through the syntax-highlighted Code path.
        assert!(matches!(classify_kind("rs"), FileKind::Code));
        assert!(matches!(classify_kind("ts"), FileKind::Code));
        assert!(matches!(classify_kind("tsx"), FileKind::Code));
        assert!(matches!(classify_kind("js"), FileKind::Code));
        assert!(matches!(classify_kind("py"), FileKind::Code));
        assert!(matches!(classify_kind("go"), FileKind::Code));
        assert!(matches!(classify_kind("html"), FileKind::Code));
        assert!(matches!(classify_kind("json"), FileKind::Code));
        // Plain text and config-like files stay on the Text path.
        assert!(matches!(classify_kind("txt"), FileKind::Text));
        assert!(matches!(classify_kind("log"), FileKind::Text));
        assert!(matches!(classify_kind("env"), FileKind::Text));
        assert!(matches!(classify_kind("unknown"), FileKind::Binary));
    }

    #[test]
    fn stat_workspace_file_returns_metadata() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("hello.md"), b"# hi").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let meta = local_stat_file(&root, "hello.md").unwrap();
        assert_eq!(meta.name, "hello.md");
        assert_eq!(meta.byte_len, 4);
        assert!(matches!(meta.kind, FileKind::Markdown));
        assert_eq!(meta.mime.as_deref(), Some("text/markdown"));
        assert!(meta.modified_ms.is_some());
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_image_file_returns_base64() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        // Minimal valid 1x1 PNG signature + IHDR + IDAT + IEND not required for the test;
        // we only verify base64 round-trip and MIME classification.
        let bytes: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        fs::write(tmp.join("pixel.png"), bytes).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let preview = local_read_image_file(&root, "pixel.png").unwrap();
        assert_eq!(preview.mime, "image/png");
        assert_eq!(preview.byte_len, bytes.len() as u64);
        assert!(!preview.truncated);
        let decoded = BASE64_STANDARD.decode(&preview.base64).unwrap();
        assert_eq!(decoded, bytes);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_image_file_rejects_non_image() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("a.txt"), b"hi").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_read_image_file(&root, "a.txt").expect_err("non-image should fail");
        assert!(err.contains("not an image"));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_video_file_rejects_non_video() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("a.png"), b"x").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_read_video_file(&root, "a.png").expect_err("non-video should fail");
        assert!(err.contains("not a video"));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn create_workspace_file_creates_under_root() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        local_create_file(&root, "new.txt").unwrap();
        assert!(tmp.join("new.txt").is_file());
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn create_workspace_file_creates_nested_parents() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        local_create_file(&root, "a/b/c.txt").unwrap();
        assert!(tmp.join("a").join("b").join("c.txt").is_file());
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn create_workspace_file_rejects_duplicate() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("dup.txt"), b"x").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_create_file(&root, "dup.txt").expect_err("duplicate should fail");
        assert!(err.contains("already exists"));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn create_workspace_dir_creates_directory() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        local_create_dir(&root, "sub").unwrap();
        assert!(tmp.join("sub").is_dir());
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn create_rejects_parent_traversal() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_create_file(&root, "../escape.txt").expect_err("traversal should fail");
        assert!(err.contains("escapes workspace"));
        let err = local_create_dir(&root, "../escape").expect_err("traversal should fail");
        assert!(err.contains("escapes workspace"));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn remote_target_joins_and_sandboxes() {
        assert_eq!(remote_target("/home/u/proj", "").unwrap(), "/home/u/proj");
        assert_eq!(
            remote_target("/home/u/proj/", "/home/u/proj").unwrap(),
            "/home/u/proj"
        );
        assert_eq!(
            remote_target("/home/u/proj", "src/main.rs").unwrap(),
            "/home/u/proj/src/main.rs"
        );
        assert!(remote_target("/home/u/proj", "../escape").is_err());
        assert!(remote_target("/home/u/proj", "/etc/passwd").is_err());
        assert!(remote_target("", "x").is_err());
    }

    #[test]
    fn create_rejects_empty_name() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_create_file(&root, "   ").expect_err("empty name should fail");
        assert!(err.contains("name is empty"));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_text_file_returns_hash_and_mtime() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("a.txt"), b"hello").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let first = local_read_text_file(&root, "a.txt").unwrap();
        assert!(first.modified_ms.is_some());
        assert_eq!(first.hash, content_hash(b"hello"));
        // Hash changes when content changes.
        fs::write(tmp.join("a.txt"), b"world").unwrap();
        let second = local_read_text_file(&root, "a.txt").unwrap();
        assert_ne!(first.hash, second.hash);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn write_text_file_happy_path_is_atomic() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("note.txt"), b"old").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let res = local_write_text_file(&root, "note.txt", "new content", None).unwrap();
        assert_eq!(res.byte_len, "new content".len() as u64);
        assert_eq!(res.hash, content_hash(b"new content"));
        assert_eq!(
            fs::read_to_string(tmp.join("note.txt")).unwrap(),
            "new content"
        );
        // No temp files left behind.
        let leftovers: Vec<_> = fs::read_dir(&tmp)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("blxcode-write"))
            .collect();
        assert!(leftovers.is_empty(), "temp file leaked");
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn write_text_file_conflict_guard() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("c.txt"), b"disk").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        // Stale expected hash ⇒ conflict, file unchanged.
        let stale = content_hash(b"what we thought");
        let err = local_write_text_file(&root, "c.txt", "mine", Some(&stale))
            .expect_err("stale hash should conflict");
        assert!(err.starts_with(CONFLICT_PREFIX));
        assert_eq!(fs::read_to_string(tmp.join("c.txt")).unwrap(), "disk");
        // Matching hash ⇒ success.
        let current = content_hash(b"disk");
        local_write_text_file(&root, "c.txt", "mine", Some(&current)).unwrap();
        assert_eq!(fs::read_to_string(tmp.join("c.txt")).unwrap(), "mine");
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn write_text_file_rejects_protected_paths() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::write(tmp.join(".git").join("config"), b"x").unwrap();
        fs::create_dir_all(tmp.join("node_modules")).unwrap();
        fs::write(tmp.join("node_modules").join("p.js"), b"x").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        for rel in [
            ".git/config",
            "node_modules/p.js",
            "target/x",
            ".agents/m.md",
        ] {
            let err = local_write_text_file(&root, rel, "y", None)
                .expect_err("protected path should be rejected");
            assert!(err.contains("protected folder"), "rel={rel} err={err}");
        }
        // .git/config must be untouched.
        assert_eq!(
            fs::read_to_string(tmp.join(".git").join("config")).unwrap(),
            "x"
        );
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn write_text_file_rejects_escape_and_missing() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = local_write_text_file(&root, "../escape.txt", "y", None)
            .expect_err("traversal should fail");
        // resolve_under_root canonicalizes; the missing target fails to resolve.
        assert!(err.contains("path not found") || err.contains("outside workspace"));
        let err = local_write_text_file(&root, "missing.txt", "y", None)
            .expect_err("missing file should fail");
        assert!(err.contains("path not found") || err.contains("not a file"));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn is_protected_rel_matches_components() {
        assert!(is_protected_rel(".git/config"));
        assert!(is_protected_rel("a/b/target/x.rs"));
        assert!(is_protected_rel("node_modules/pkg/index.js"));
        assert!(!is_protected_rel("src/main.rs"));
        assert!(!is_protected_rel("targets.txt")); // not a full component
    }

    #[test]
    fn list_workspace_files_walks_and_skips_protected() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::create_dir_all(tmp.join("node_modules").join("pkg")).unwrap();
        fs::write(tmp.join("README.md"), b"x").unwrap();
        fs::write(tmp.join("src").join("main.rs"), b"x").unwrap();
        fs::write(tmp.join(".git").join("config"), b"x").unwrap();
        fs::write(tmp.join("node_modules").join("pkg").join("i.js"), b"x").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let files = local_list_workspace_files(&root).unwrap();
        assert!(files.contains(&"README.md".to_string()));
        assert!(files.contains(&"src/main.rs".to_string()));
        assert!(!files.iter().any(|f| f.starts_with(".git/")));
        assert!(!files.iter().any(|f| f.starts_with("node_modules/")));
        // Sorted.
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn content_hash_is_stable_and_distinct() {
        assert_eq!(content_hash(b"abc"), content_hash(b"abc"));
        assert_ne!(content_hash(b"abc"), content_hash(b"abd"));
        assert_eq!(content_hash(b"").len(), 16);
    }
}
