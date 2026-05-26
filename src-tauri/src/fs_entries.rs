//! Sandboxed directory listing for the sidebar project explorer.

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_TEXT_PREVIEW_BYTES: u64 = 512 * 1024;
const MAX_IMAGE_PREVIEW_BYTES: u64 = 16 * 1024 * 1024;
const MAX_VIDEO_PREVIEW_BYTES: u64 = 64 * 1024 * 1024;

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
        "txt" | "log" | "ini" | "conf" | "cfg" | "env" | "properties" | "lock" | "gitignore"
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

/// Lists files and directories under `path`, constrained to `workspace_root`.
#[tauri::command]
pub fn list_path_entries(
    workspace_root: String,
    path: String,
) -> Result<Vec<FsEntryBrief>, String> {
    let root = canonical_root(&workspace_root)?;
    let dir = resolve_under_root(&root, &path)?;
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

/// Reads a UTF-8 text file under `workspace_root` for the center preview tab.
#[tauri::command]
pub fn read_workspace_text_file(
    workspace_root: String,
    path: String,
) -> Result<TextFilePreview, String> {
    let root = canonical_root(&workspace_root)?;
    let file = resolve_under_root(&root, &path)?;
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
    let content =
        String::from_utf8(bytes).map_err(|_| "file is not valid UTF-8 text".to_string())?;
    Ok(TextFilePreview {
        content,
        truncated,
        byte_len,
    })
}

/// Lightweight metadata for the file preview topbar.
/// Returns name, relative path (as supplied by the caller), byte size,
/// modification timestamp (Unix ms, if available), classified [`FileKind`]
/// and a best-effort MIME guess. Errors mirror the existing sandbox path.
#[tauri::command]
pub fn stat_workspace_file(workspace_root: String, path: String) -> Result<FileMeta, String> {
    let root = canonical_root(&workspace_root)?;
    let file = resolve_under_root(&root, &path)?;
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
        .unwrap_or_else(|| path.clone());
    Ok(FileMeta {
        name,
        rel_path: path,
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
    workspace_root: String,
    path: String,
) -> Result<BinaryFilePreview, String> {
    let root = canonical_root(&workspace_root)?;
    let file = resolve_under_root(&root, &path)?;
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
    workspace_root: String,
    path: String,
) -> Result<BinaryFilePreview, String> {
    let root = canonical_root(&workspace_root)?;
    let file = resolve_under_root(&root, &path)?;
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
        let entries = list_path_entries(root.clone(), root.clone()).unwrap();
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
        let preview = read_workspace_text_file(root, "hello.txt".into()).unwrap();
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
        let err = read_workspace_text_file(root, outside.to_string_lossy().into_owned())
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
        let err = read_workspace_text_file(root, "dir".into()).expect_err("directory should fail");
        assert_eq!(err, "not a file");
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_text_file_handles_missing_files() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err = read_workspace_text_file(root, "missing.txt".into()).expect_err("missing");
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
        assert!(classify_policy("random").is_none());
    }

    #[test]
    fn stat_workspace_file_marks_extensionless_license_as_markdown() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("LICENSE"), b"MIT License\n\nCopyright").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let meta = stat_workspace_file(root, "LICENSE".into()).unwrap();
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
        let meta = stat_workspace_file(root, "CONTRIBUTING.md".into()).unwrap();
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
        assert!(matches!(classify_kind("gitignore"), FileKind::Text));
        assert!(matches!(classify_kind("unknown"), FileKind::Binary));
    }

    #[test]
    fn stat_workspace_file_returns_metadata() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("hello.md"), b"# hi").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let meta = stat_workspace_file(root, "hello.md".into()).unwrap();
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
        let preview = read_workspace_image_file(root, "pixel.png".into()).unwrap();
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
        let err =
            read_workspace_image_file(root, "a.txt".into()).expect_err("non-image should fail");
        assert!(err.contains("not an image"));
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn read_workspace_video_file_rejects_non_video() {
        let tmp = std::env::temp_dir().join(format!("blx_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("a.png"), b"x").unwrap();
        let root = tmp.to_string_lossy().into_owned();
        let err =
            read_workspace_video_file(root, "a.png".into()).expect_err("non-video should fail");
        assert!(err.contains("not a video"));
        let _ = fs::remove_dir_all(tmp);
    }
}
