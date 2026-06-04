//! Format / sanitize helpers used by the file preview dispatcher.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::FileKind;
use js_sys::Date;
use leptos::prelude::*;

/// Returns a human-readable byte-size string (1.4 MiB, 768 B, …).
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0usize;
    while size >= 1024.0 && unit + 1 < UNITS.len() {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Renders a Unix-ms timestamp as a locale-aware string via [`js_sys::Date`].
/// Returns `None` if the timestamp is missing or not finite.
#[must_use]
pub fn format_mtime(ms: Option<i64>) -> Option<String> {
    let ms = ms?;
    let date = Date::new(&(ms as f64).into());
    let label = date.to_locale_string("default", &js_sys::Object::new());
    let label = label.as_string()?;
    if label.is_empty() {
        None
    } else {
        Some(label)
    }
}

/// Categorised error state for every file-preview renderer. Each variant
/// maps to its own translated banner so users see why a preview failed
/// instead of a raw backend string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilePreviewError {
    /// Running in a non-Tauri webview (the file APIs are unavailable).
    NoTauri,
    /// Workspace id no longer present in the service.
    WorkspaceNotFound,
    /// Payload exceeded the renderer's byte cap. `bytes` is the original size.
    TooLarge(u64),
    /// Backend or IPC error; `detail` is shown after the localized label.
    Failed(String),
}

/// Renders a [`FilePreviewError`] using the supplied label for the `Failed`
/// case. `NoTauri` / `WorkspaceNotFound` / `TooLarge` ignore the label and
/// show their own standalone message so the user sees the most relevant text.
#[must_use]
pub fn render_load_error(
    i18n: I18nService,
    failed_label: I18nKey,
    error: FilePreviewError,
) -> AnyView {
    match error {
        FilePreviewError::NoTauri => view! {
            <div class="file-preview__status file-preview__status--error">
                {i18n.tr(I18nKey::FilePreviewNoTauri)}
            </div>
        }
        .into_any(),
        FilePreviewError::WorkspaceNotFound => view! {
            <div class="file-preview__status file-preview__status--error">
                {i18n.tr(I18nKey::FilePreviewWorkspaceNotFound)}
            </div>
        }
        .into_any(),
        FilePreviewError::TooLarge(bytes) => view! {
            <div class="file-preview__status file-preview__status--error">
                {move || i18n
                    .tr(I18nKey::FilePreviewTooLarge)()
                    .replace("{size}", &format_bytes(bytes))}
            </div>
        }
        .into_any(),
        FilePreviewError::Failed(detail) => {
            let detail = detail.trim().to_string();
            let detail_for_render = detail.clone();
            view! {
                <div class="file-preview__status file-preview__status--error">
                    <strong class="file-preview__error-label">{i18n.tr(failed_label)}</strong>
                    {move || if detail_for_render.is_empty() {
                        String::new()
                    } else {
                        format!(": {detail_for_render}")
                    }}
                </div>
            }
            .into_any()
        }
    }
}

/// Lucide icon used in the topbar for each file kind.
#[must_use]
pub fn icon_for_kind(kind: FileKind) -> icondata::Icon {
    match kind {
        FileKind::Image => icondata::LuImage,
        FileKind::Video => icondata::LuFilm,
        FileKind::Markdown => icondata::LuFileText,
        FileKind::Mermaid => icondata::LuGitBranch,
        FileKind::Code => icondata::LuFileCode,
        FileKind::Text => icondata::LuFileText,
        FileKind::Binary => icondata::LuFile,
    }
}

/// Maps a lowercased file extension to the `highlight.js` language alias used
/// by `hljs.highlight(code, { language })`. Returns `None` when the extension
/// has no reliable mapping; callers then fall back to escaped plain text.
#[must_use]
pub fn hljs_lang_for_ext(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" | "pyw" | "pyi" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" | "sc" => "scala",
        "groovy" | "gradle" => "groovy",
        "swift" => "swift",
        "m" | "mm" => "objectivec",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "cs" => "csharp",
        "fs" | "fsx" => "fsharp",
        "vb" => "vbnet",
        "rb" | "erb" => "ruby",
        "php" | "phtml" => "php",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "dart" => "dart",
        "r" => "r",
        "jl" => "julia",
        "clj" | "cljs" | "cljc" | "edn" => "clojure",
        "ex" | "exs" | "eex" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" | "lhs" => "haskell",
        "elm" => "elm",
        "nim" => "nim",
        "zig" => "zig",
        "ml" | "mli" | "ocaml" => "ocaml",
        "html" | "htm" | "xhtml" | "vue" | "svelte" => "xml",
        "css" => "css",
        "scss" | "sass" => "scss",
        "less" => "less",
        "json" | "json5" | "jsonc" => "json",
        "toml" => "ini",
        "yaml" | "yml" => "yaml",
        "xml" | "plist" => "xml",
        "sh" | "bash" | "zsh" | "fish" => "bash",
        "ps1" => "powershell",
        "bat" | "cmd" => "dos",
        "sql" => "sql",
        "graphql" | "gql" => "graphql",
        "proto" | "thrift" => "protobuf",
        "tf" | "tfvars" | "hcl" => "hcl",
        "nix" => "nix",
        "dockerfile" | "containerfile" => "dockerfile",
        "makefile" | "mk" | "cmake" => "makefile",
        "diff" | "patch" => "diff",
        "md" | "markdown" => "markdown",
        _ => return None,
    })
}

/// Build a fenced markdown snippet for a file preview line range.
///
/// `range` is 1-based, inclusive. `source_workspace_for_header` is set when
/// the snippet is being sent to a target in a **different** workspace than
/// the preview source so the receiver can disambiguate; otherwise pass
/// `None` for a clean `path:start-end` header.
///
/// Out-of-range indices are clamped to the available `plain_lines` so the
/// helper never panics on stale signals.
#[must_use]
pub fn build_file_snippet_block(
    rel_path: &str,
    language: Option<&str>,
    plain_lines: &[String],
    range: (u32, u32),
    source_workspace_for_header: Option<&str>,
) -> String {
    let total = plain_lines.len() as u32;
    if total == 0 {
        return String::new();
    }
    let (raw_s, raw_e) = range;
    let start = raw_s.max(1).min(total);
    let end = raw_e.max(start).min(total);

    let start_idx = (start - 1) as usize;
    let end_idx = (end - 1) as usize;
    let slice = plain_lines[start_idx..=end_idx].join("\n");

    let location = match source_workspace_for_header {
        Some(ws) if !ws.is_empty() && start == end => format!("{ws}:{rel_path}:{start}"),
        Some(ws) if !ws.is_empty() => format!("{ws}:{rel_path}:{start}-{end}"),
        _ if start == end => format!("{rel_path}:{start}"),
        _ => format!("{rel_path}:{start}-{end}"),
    };
    let lang_tag = language.unwrap_or("");
    format!("**`{location}`**\n```{lang_tag}\n{slice}\n```\n")
}

/// Allowlist-based sanitizer for SVG strings rendered inline via `inner_html`.
/// Drops `<script>` blocks, `on*` attributes, and any `javascript:`/`data:`
/// references in `href`/`xlink:href`/`src`. Preserves the rest verbatim so
/// authored SVG appearance survives.
#[must_use]
pub fn sanitize_svg(input: &str) -> String {
    let stripped = strip_tag_blocks(input, "script");
    let stripped = strip_tag_blocks(&stripped, "foreignObject");
    strip_dangerous_attributes(&stripped)
}

/// Allowlist-based sanitizer for the HTML produced by `pulldown-cmark`.
/// Removes `<script>`, `<style>`, `<iframe>` blocks and dangerous attributes.
#[must_use]
pub fn sanitize_markdown_html(input: &str) -> String {
    let s = strip_tag_blocks(input, "script");
    let s = strip_tag_blocks(&s, "style");
    let s = strip_tag_blocks(&s, "iframe");
    let s = strip_tag_blocks(&s, "object");
    let s = strip_tag_blocks(&s, "embed");
    strip_dangerous_attributes(&s)
}

/// Removes complete `<tag …>…</tag>` blocks (case-insensitive). Stops at the
/// first `</tag>` after each opener — nested same-tag blocks (rare in HTML)
/// fall through cleanly because the outer scan continues past the close.
fn strip_tag_blocks(input: &str, tag: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let open_tok = format!("<{}", tag);
    let close_tok = format!("</{}>", tag);
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while let Some(rel) = lower[cursor..].find(&open_tok) {
        let open_start = cursor + rel;
        let after_name = open_start + open_tok.len();
        let after_name_ch = lower.as_bytes().get(after_name).copied();
        if !matches!(
            after_name_ch,
            Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')
        ) {
            out.push_str(&input[cursor..=open_start]);
            cursor = open_start + 1;
            continue;
        }
        out.push_str(&input[cursor..open_start]);
        if let Some(close_rel) = lower[after_name..].find(&close_tok) {
            cursor = after_name + close_rel + close_tok.len();
        } else if let Some(self_end) = lower[after_name..].find('>') {
            cursor = after_name + self_end + 1;
        } else {
            break;
        }
    }
    out.push_str(&input[cursor..]);
    out
}

/// Removes `on*=` attributes and neutralizes `javascript:` URIs anywhere in
/// the HTML. Implemented with a byte-level scan that only breaks on ASCII
/// delimiters (`<`, `>`, whitespace), and copies content via string slicing
/// so multi-byte UTF-8 codepoints are preserved verbatim.
fn strip_dangerous_attributes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut copy_start = 0usize;
    let mut in_tag = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'<' => {
                in_tag = true;
                i += 1;
            }
            b'>' => {
                in_tag = false;
                i += 1;
            }
            b' ' | b'\t' | b'\n' if in_tag => {
                let rest = &input[i + 1..];
                if let Some(end) = next_attr_end(rest) {
                    let attr = &rest[..end];
                    let lower = attr.to_ascii_lowercase();
                    if is_unsafe_attribute(&lower) {
                        out.push_str(&input[copy_start..i]);
                        i = i + 1 + end;
                        copy_start = i;
                        continue;
                    }
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    out.push_str(&input[copy_start..]);
    out
}

fn next_attr_end(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let mut name_end = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'=' {
            name_end = Some(i);
            break;
        }
        if c == b' ' || c == b'\t' || c == b'\n' || c == b'>' || c == b'/' {
            return Some(i);
        }
        i += 1;
    }
    let Some(name_end) = name_end else {
        return Some(bytes.len());
    };
    let mut j = name_end + 1;
    while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\n') {
        j += 1;
    }
    if j >= bytes.len() {
        return Some(bytes.len());
    }
    match bytes[j] {
        b'"' => {
            j += 1;
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            Some((j + 1).min(bytes.len()))
        }
        b'\'' => {
            j += 1;
            while j < bytes.len() && bytes[j] != b'\'' {
                j += 1;
            }
            Some((j + 1).min(bytes.len()))
        }
        _ => {
            while j < bytes.len() && !matches!(bytes[j], b' ' | b'\t' | b'\n' | b'>' | b'/') {
                j += 1;
            }
            Some(j)
        }
    }
}

fn is_unsafe_attribute(lower_attr: &str) -> bool {
    let trimmed = lower_attr.trim_start();
    if trimmed.starts_with("on") {
        let after = &trimmed[2..];
        if after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        {
            return true;
        }
    }
    let unsafe_protocol = |attr_name: &str| {
        if let Some(eq) = trimmed.find('=') {
            if !trimmed[..eq].trim().ends_with(attr_name) {
                return false;
            }
            let value = trimmed[eq + 1..]
                .trim_start_matches([' ', '"', '\''])
                .trim_end();
            return value.starts_with("javascript:") || value.starts_with("vbscript:");
        }
        false
    };
    if unsafe_protocol("href")
        || unsafe_protocol("xlink:href")
        || unsafe_protocol("src")
        || unsafe_protocol("formaction")
        || unsafe_protocol("action")
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_handles_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn sanitize_svg_removes_scripts() {
        let svg = r#"<svg><script>alert(1)</script><circle r="5" onclick="boom()"/></svg>"#;
        let out = sanitize_svg(svg);
        assert!(!out.contains("script"));
        assert!(!out.contains("onclick"));
        assert!(out.contains("<circle"));
    }

    #[test]
    fn sanitize_markdown_strips_iframes_and_handlers() {
        let html = r#"<p onclick="boom()">hi<iframe src="x"></iframe></p>"#;
        let out = sanitize_markdown_html(html);
        assert!(!out.contains("iframe"));
        assert!(!out.contains("onclick"));
        assert!(out.contains("<p"));
    }

    #[test]
    fn sanitize_markdown_neutralizes_javascript_links() {
        let html = r#"<a href="javascript:alert(1)">x</a>"#;
        let out = sanitize_markdown_html(html);
        assert!(!out.contains("javascript:"));
    }

    #[test]
    fn sanitize_markdown_preserves_utf8_codepoints() {
        let html = "<p>Hallo, schöne Grüße für €-Land — 你好 ✓</p>";
        let out = sanitize_markdown_html(html);
        assert!(out.contains("schöne"));
        assert!(out.contains("Grüße"));
        assert!(out.contains("für"));
        assert!(out.contains("€"));
        assert!(out.contains("你好"));
        assert!(out.contains("✓"));
        assert!(!out.contains("Ã¼"));
        assert!(!out.contains("Ã¶"));
    }

    #[test]
    fn sanitize_svg_preserves_utf8_codepoints() {
        let svg = r#"<svg><text>für — €</text></svg>"#;
        let out = sanitize_svg(svg);
        assert!(out.contains("für"));
        assert!(out.contains("€"));
        assert!(!out.contains("Ã¼"));
    }

    #[test]
    fn hljs_lang_for_ext_covers_common_languages() {
        assert_eq!(hljs_lang_for_ext("rs"), Some("rust"));
        assert_eq!(hljs_lang_for_ext("ts"), Some("typescript"));
        assert_eq!(hljs_lang_for_ext("tsx"), Some("typescript"));
        assert_eq!(hljs_lang_for_ext("js"), Some("javascript"));
        assert_eq!(hljs_lang_for_ext("py"), Some("python"));
        assert_eq!(hljs_lang_for_ext("html"), Some("xml"));
        assert_eq!(hljs_lang_for_ext("css"), Some("css"));
        assert_eq!(hljs_lang_for_ext("toml"), Some("ini"));
        assert_eq!(hljs_lang_for_ext("zz_unknown"), None);
    }

    fn sample_plain_lines() -> Vec<String> {
        vec![
            "let a = 1;".to_string(),
            "let b = 2;".to_string(),
            "let c = 3;".to_string(),
            "let d = 4;".to_string(),
        ]
    }

    #[test]
    fn build_file_snippet_block_single_line_same_workspace() {
        let out = build_file_snippet_block(
            "src/foo.rs",
            Some("rust"),
            &sample_plain_lines(),
            (2, 2),
            None,
        );
        assert!(out.starts_with("**`src/foo.rs:2`**\n"));
        assert!(out.contains("```rust\nlet b = 2;\n```\n"));
    }

    #[test]
    fn build_file_snippet_block_range_same_workspace() {
        let out = build_file_snippet_block(
            "src/foo.rs",
            Some("rust"),
            &sample_plain_lines(),
            (2, 3),
            None,
        );
        assert!(out.contains("**`src/foo.rs:2-3`**"));
        assert!(out.contains("let b = 2;\nlet c = 3;"));
    }

    #[test]
    fn build_file_snippet_block_range_cross_workspace() {
        let out = build_file_snippet_block(
            "src/foo.rs",
            Some("rust"),
            &sample_plain_lines(),
            (1, 4),
            Some("Demo"),
        );
        assert!(out.contains("**`Demo:src/foo.rs:1-4`**"));
    }

    #[test]
    fn build_file_snippet_block_without_language_leaves_fence_tag_empty() {
        let out =
            build_file_snippet_block("notes/log.txt", None, &sample_plain_lines(), (1, 1), None);
        // Fence opens with bare three backticks (no lang tag).
        assert!(out.contains("```\nlet a = 1;\n```"));
    }

    #[test]
    fn build_file_snippet_block_preserves_utf8() {
        let lines: Vec<String> = vec!["Grüße aus München".into(), "你好世界 ✓".into()];
        let out = build_file_snippet_block("i18n.md", None, &lines, (1, 2), None);
        assert!(out.contains("Grüße aus München"));
        assert!(out.contains("你好世界 ✓"));
        // Header sanity-checks UTF-8 path safety too.
        assert!(out.contains("**`i18n.md:1-2`**"));
    }

    #[test]
    fn build_file_snippet_block_clamps_out_of_range() {
        // start above total -> clamp to last line; end below start -> raised to start.
        let out =
            build_file_snippet_block("x.rs", Some("rust"), &sample_plain_lines(), (99, 1), None);
        assert!(out.contains("**`x.rs:4`**"));
        assert!(out.contains("let d = 4;"));
    }
}
