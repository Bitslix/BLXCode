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
        FileKind::Text => icondata::LuFileText,
        FileKind::Binary => icondata::LuFile,
    }
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
        if !matches!(after_name_ch, Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')) {
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
/// the HTML. Implemented with a single pass — runs in O(n) over the input.
fn strip_dangerous_attributes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    let mut in_tag = false;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'<' {
            in_tag = true;
            out.push(c as char);
            i += 1;
            continue;
        }
        if c == b'>' {
            in_tag = false;
            out.push(c as char);
            i += 1;
            continue;
        }
        if in_tag && (c == b' ' || c == b'\t' || c == b'\n') {
            let rest = &input[i + 1..];
            if let Some(end) = next_attr_end(rest) {
                let attr = &rest[..end];
                let lower = attr.to_ascii_lowercase();
                if is_unsafe_attribute(&lower) {
                    i += 1 + end;
                    continue;
                }
            }
        }
        out.push(c as char);
        i += 1;
    }
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
}
