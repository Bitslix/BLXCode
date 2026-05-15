//! Wikilink + workspace-memory path expansion for agent chat and memory preview.
//! Emits `blxmemory:<relative>` links handled by [`crate::open_http::dom_click_nav_href`].
//!
//! Fenced code blocks render as collapsible `<details>` (closed by default). If the agent
//! wants a block expanded initially, use `blx-open` as the first info word, e.g.
//! ` ```blx-open ` or ` ```blx-open rust `.

use crate::memory_paths::{sanitize_memory_relative_path, slug_to_filename};
use pulldown_cmark::{html, Options, Parser};
use std::borrow::Cow;

fn strip_known_prefixes(path: &str) -> Cow<'_, str> {
    let t = path.trim();
    if let Some(r) = t.strip_prefix(".blxcode/memory/") {
        return Cow::Borrowed(r);
    }
    if let Some(r) = t.strip_prefix("memory/") {
        return Cow::Borrowed(r);
    }
    Cow::Borrowed(t)
}

fn memory_rel_from_display_path(display_path: &str) -> Option<String> {
    let t = display_path.trim();
    let rel = strip_known_prefixes(t);
    sanitize_memory_relative_path(rel.as_ref())
}

fn escape_md_link_text_fragment(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            _ => out.push(ch),
        }
    }
}

fn try_emit_wikilink(out: &mut String, inner: &str) -> bool {
    let (target_raw, label) = if let Some(pipe) = inner.find('|') {
        (inner[..pipe].trim(), inner[pipe + 1..].trim())
    } else {
        let t = inner.trim();
        (t, t)
    };
    if target_raw.is_empty() {
        return false;
    }
    let target = strip_known_prefixes(target_raw);
    let href_rel = sanitize_memory_relative_path(target.as_ref())
        .unwrap_or_else(|| slug_to_filename(target.as_ref()));
    let display = if label.is_empty() {
        href_rel.as_str()
    } else {
        label
    };
    out.push('[');
    escape_md_link_text_fragment(display, out);
    out.push_str("](blxmemory:");
    out.push_str(&href_rel);
    out.push(')');
    true
}

fn scan_line_for_wikilinks_and_paths(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len() + 16);
    let mut i = 0;
    while i < line.len() {
        if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(end) = line[i + 2..].find("]]") {
                let inner = &line[i + 2..i + 2 + end];
                if try_emit_wikilink(&mut out, inner) {
                    i += 2 + end + 2;
                    continue;
                }
            }
            out.push('[');
            i += 1;
            continue;
        }

        const PREFIX: &str = ".blxcode/memory/";
        if line[i..].starts_with(PREFIX) {
            let after = &line[i + PREFIX.len()..];
            let end = after
                .find(|c: char| {
                    c.is_whitespace()
                        || matches!(
                            c,
                            ')' | ']' | '`' | '"' | '\'' | '>' | '<' | ';' | ','
                        )
                })
                .unwrap_or(after.len());
            let raw = after[..end].trim();
            if !raw.is_empty()
                && !raw.contains("..")
                && raw.to_ascii_lowercase().ends_with(".md")
            {
                if let Some(rel) = sanitize_memory_relative_path(raw) {
                    let display_path = format!("{PREFIX}{raw}");
                    out.push('[');
                    escape_md_link_text_fragment(&display_path, &mut out);
                    out.push_str("](blxmemory:");
                    out.push_str(&rel);
                    out.push(')');
                    i += PREFIX.len() + end;
                    continue;
                }
            }
        }

        let ch = line[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Expands `[[wikilinks]]`, `.blxcode/memory/*.md` (outside fenced code), then runs pulldown + light HTML fixes.
#[must_use]
pub fn preprocess_agent_chat_markdown(src: &str) -> String {
    let mut out = String::with_capacity(src.len() + 32);
    let mut fenced = false;
    for line in src.split_inclusive('\n') {
        let (body, nl) = if let Some(b) = line.strip_suffix('\n') {
            (b, true)
        } else {
            (line, false)
        };
        let trim = body.trim_start();
        if fenced {
            out.push_str(body);
            if nl {
                out.push('\n');
            }
            if trim.starts_with("```") {
                fenced = false;
            }
            continue;
        }
        if trim.starts_with("```") {
            out.push_str(body);
            if nl {
                out.push('\n');
            }
            fenced = true;
            continue;
        }
        out.push_str(&scan_line_for_wikilinks_and_paths(body));
        if nl {
            out.push('\n');
        }
    }
    out
}

/// Strips `blx-open` from fenced-block info lines (first token only) and records whether each
/// fence should start expanded.
fn normalize_blx_open_fenced_markers(md: &str) -> (String, Vec<bool>) {
    let mut out = String::with_capacity(md.len());
    let mut expand_defaults = Vec::new();
    let mut fenced = false;

    for line in md.split_inclusive('\n') {
        let (body, nl) = if let Some(b) = line.strip_suffix('\n') {
            (b, true)
        } else {
            (line, false)
        };
        let trim = body.trim_start();

        if fenced {
            out.push_str(body);
            if nl {
                out.push('\n');
            }
            if trim.starts_with("```") {
                fenced = false;
            }
            continue;
        }

        if trim.starts_with("```") {
            let info_raw = trim[3..].trim();
            let (expand, rewritten) = strip_blx_open_fence_info(info_raw);
            expand_defaults.push(expand);
            if expand {
                let indent = body.len() - trim.len();
                out.extend(std::iter::repeat(' ').take(indent));
                out.push_str("```");
                if !rewritten.is_empty() {
                    out.push(' ');
                    out.push_str(&rewritten);
                }
            } else {
                out.push_str(body);
            }
            if nl {
                out.push('\n');
            }
            fenced = true;
            continue;
        }

        out.push_str(body);
        if nl {
            out.push('\n');
        }
    }

    (out, expand_defaults)
}

fn strip_blx_open_fence_info(info: &str) -> (bool, String) {
    let mut parts = info.split_whitespace();
    let Some(first) = parts.next() else {
        return (false, String::new());
    };
    if !first.eq_ignore_ascii_case("blx-open") {
        return (false, info.to_string());
    }
    let rest = parts.collect::<Vec<_>>().join(" ");
    (true, rest)
}

fn language_token_from_open_tags(pre_and_code_open: &str) -> Option<String> {
    const KEY: &str = "class=\"language-";
    let start = pre_and_code_open.find(KEY)? + KEY.len();
    let rel = &pre_and_code_open[start..];
    let end = rel.find('"')?;
    Some(rel[..end].to_string())
}

/// `pulldown-cmark` emits `<pre><code class="language-…">` with the first `>` closing `<pre>`,
/// so `pre_open` must include the `<code …>` prefix to read the language class.
fn fence_summary_html(pre_open: &str, inner: &str) -> String {
    let scan = match inner.find('>') {
        Some(gt) => {
            let mut s = String::with_capacity(pre_open.len() + gt + 1);
            s.push_str(pre_open);
            s.push_str(&inner[..=gt]);
            s
        }
        None => pre_open.to_string(),
    };
    let lang = language_token_from_open_tags(&scan).filter(|s| !s.is_empty());
    let lang_html = lang
        .as_ref()
        .map(|l| {
            format!(
                r#"<span class="workbench-md-fence__summary-lang">{}</span>"#,
                html_escape_summary_text(l)
            )
        })
        .unwrap_or_default();
    format!(
        r#"<span class="workbench-md-fence__summary-title">Code</span>{lang_html}"#
    )
}

fn html_escape_summary_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

fn looks_like_markdown_fence_open_line(line: &str) -> bool {
    let t = line.trim_start();
    if !t.starts_with("```") {
        return false;
    }
    !t[3..].contains('`')
}

fn looks_like_markdown_fence_close_line(line: &str) -> bool {
    line.trim() == "```"
}

/// Removes a leading / trailing Markdown fence line accidentally left inside `<code>` (e.g.
/// indented blocks or odd model formatting).
fn strip_echo_fence_lines_in_code_inner_html(inner: &str) -> String {
    let Some(gt) = inner.find('>') else {
        return inner.to_string();
    };
    let Some(close) = inner.rfind("</code>") else {
        return inner.to_string();
    };
    let prefix = &inner[..=gt];
    let body = &inner[gt + 1..close];
    let suffix = &inner[close..];
    let ends_nl = body.ends_with('\n');
    let lines: Vec<&str> = body.split('\n').collect();
    if lines.len() < 2 {
        return inner.to_string();
    }
    let mut start = 0usize;
    let mut end = lines.len();
    if looks_like_markdown_fence_open_line(lines[0]) {
        start = 1;
    }
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    if end > start && looks_like_markdown_fence_close_line(lines[end - 1]) {
        end -= 1;
    }
    if start == 0 && end == lines.len() {
        return inner.to_string();
    }
    let mut out = lines[start..end].join("\n");
    if ends_nl && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    format!("{prefix}{out}{suffix}")
}

/// Wraps each `<pre>…</pre>` emitted for fenced code in `<details>`; consumes `expand_defaults`
/// in document order (extra `<pre>` blocks default to collapsed).
fn wrap_collapsible_code_fences(html: String, expand_defaults: &[bool]) -> String {
    let mut flags = expand_defaults.iter().copied();
    let mut out = String::with_capacity(html.len() + expand_defaults.len() * 180);
    let mut rest = html.as_str();

    while let Some(pos) = rest.find("<pre") {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let Some(gt) = rest.find('>') else {
            out.push_str(rest);
            return out;
        };
        let pre_open = &rest[..=gt];
        rest = &rest[gt + 1..];
        let Some(close) = rest.find("</pre>") else {
            out.push_str(pre_open);
            out.push_str(rest);
            return out;
        };
        let inner = &rest[..close];
        rest = &rest[close + 6..];

        let expand = flags.next().unwrap_or(false);
        let open_attr = if expand { " open" } else { "" };
        let summary = fence_summary_html(pre_open, inner);
        let inner = strip_echo_fence_lines_in_code_inner_html(inner);

        out.push_str(r#"<details class="workbench-md-fence""#);
        out.push_str(open_attr);
        out.push_str(r#"><summary class="workbench-md-fence__summary" tabindex="0">"#);
        out.push_str(&summary);
        out.push_str(r#"</summary><div class="workbench-md-fence__body">"#);
        out.push_str(pre_open);
        out.push_str(&inner);
        out.push_str("</pre></div></details>");
    }
    out.push_str(rest);
    out
}

fn escape_href_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

fn postprocess_html_inline_code_memory_paths(html: &mut String) {
    let src = std::mem::take(html);
    let mut out = String::with_capacity(src.len() + 64);
    let mut rest = src.as_str();
    while let Some(pos) = rest.find("<code>") {
        out.push_str(&rest[..pos + 6]);
        rest = &rest[pos + 6..];
        let Some(en) = rest.find("</code>") else {
            out.push_str(rest);
            *html = out;
            return;
        };
        let inner = &rest[..en];
        rest = &rest[en + 7..];
        let trimmed = inner.trim();
        if !inner.contains('\n')
            && trimmed.starts_with(".blxcode/memory/")
            && trimmed.to_ascii_lowercase().ends_with(".md")
        {
            if let Some(rel) = memory_rel_from_display_path(trimmed) {
                out.push_str(r#"<a href="blxmemory:"#);
                out.push_str(&escape_href_attr(&rel));
                out.push_str(r#"" class="workbench-md-memlink"><code>"#);
                out.push_str(inner);
                out.push_str("</code></a>");
                continue;
            }
        }
        out.push_str(inner);
        out.push_str("</code>");
    }
    out.push_str(rest);
    *html = out;
}

#[must_use]
pub fn render_markdown_to_html(src: &str) -> String {
    let prepped = preprocess_agent_chat_markdown(src);
    let (md_for_cmark, fence_expand) = normalize_blx_open_fenced_markers(&prepped);
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(&md_for_cmark, opts);
    let mut html_out = String::with_capacity(md_for_cmark.len() * 2);
    html::push_html(&mut html_out, parser);
    postprocess_html_inline_code_memory_paths(&mut html_out);
    wrap_collapsible_code_fences(html_out, &fence_expand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blx_open_stripped_and_flag_set() {
        let md = "```blx-open rust\nlet x = 1;\n```\n";
        let (out, flags) = normalize_blx_open_fenced_markers(md);
        assert_eq!(flags, vec![true]);
        assert!(out.contains("``` rust"));
        assert!(!out.contains("blx-open"));
    }

    #[test]
    fn default_fence_collapsed_flag() {
        let md = "```text\nhi\n```\n";
        let (_out, flags) = normalize_blx_open_fenced_markers(md);
        assert_eq!(flags, vec![false]);
    }

    #[test]
    fn wrap_inserts_details() {
        let html = "<p>a</p><pre><code>x\n</code></pre><p>b</p>";
        let wrapped = wrap_collapsible_code_fences(html.to_string(), &[false]);
        assert!(wrapped.contains("<details"));
        assert!(!wrapped.contains("<details class=\"workbench-md-fence\" open"));
        assert!(wrapped.contains("</details>"));
    }

    #[test]
    fn wrap_summary_includes_language_from_code_tag() {
        let html = r#"<pre><code class="language-rust">fn main() {}
</code></pre>"#;
        let wrapped = wrap_collapsible_code_fences(html.to_string(), &[false]);
        assert!(wrapped.contains("workbench-md-fence__summary-lang"));
        assert!(wrapped.contains("rust"));
        assert!(wrapped.contains("workbench-md-fence__summary-title"));
    }

    #[test]
    fn strip_echo_fences_inside_code_block() {
        let inner = r#"<code class="language-rust">``` rust
fn main() {}
```
</code>"#;
        let out = strip_echo_fence_lines_in_code_inner_html(inner);
        assert!(!out.contains("```"));
        assert!(out.contains("fn main()"));
    }

    #[test]
    fn render_fenced_rust_has_lang_and_no_ticks_in_body() {
        let md = "```rust\nfn main() {}\n```\n";
        let html = render_markdown_to_html(md);
        assert!(html.contains("workbench-md-fence__summary-lang"));
        assert!(html.contains("fence__summary-lang\">rust</span>"));
        assert!(!html.contains("```"));
    }

    #[test]
    fn wrap_respects_open_default() {
        let html = "<pre><code>z</code></pre>";
        let wrapped = wrap_collapsible_code_fences(html.to_string(), &[true]);
        assert!(wrapped.contains("<details class=\"workbench-md-fence\" open>"));
    }
}
