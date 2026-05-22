//! Source-code preview with line numbers, syntax highlighting via
//! highlight.js and clickable row selection.
//!
//! Pure text (e.g. `.txt`, `.log`, `.env`) is rendered through this same
//! component without syntax highlighting, but still receives line numbers
//! and row selection.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, read_workspace_text_file};
use crate::workbench::file_preview::hljs_glue::highlight;
use crate::workbench::file_preview::util::{
    hljs_lang_for_ext, html_escape, render_load_error, split_highlighted_into_lines,
    FilePreviewError,
};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// Pre-rendered code data: one HTML fragment per source line plus optional
/// truncation indicator.
#[derive(Clone)]
struct PreparedCode {
    /// Already-escaped (and possibly hljs-highlighted) HTML fragments, one per
    /// line. Safe to embed via `inner_html`.
    lines: Vec<String>,
    /// `true` when the backend capped the file content; surfaces a notice
    /// banner above the gutter.
    truncated: bool,
    /// Raw byte length of the file as reported by the backend (used in the
    /// truncation notice).
    byte_len: u64,
    /// Language alias that was highlighted with, or `None` for plain text.
    language: Option<&'static str>,
}

/// Resolves the language hint for `rel_path`'s extension. Returns `None` for
/// extensions that have no reliable highlight.js mapping (plain text path).
fn lang_for_path(rel_path: &str) -> Option<&'static str> {
    let ext = rel_path.rsplit('.').next()?.to_ascii_lowercase();
    let lower = rel_path.to_ascii_lowercase();
    if lower.ends_with("dockerfile") || lower.ends_with("containerfile") {
        return Some("dockerfile");
    }
    if lower.ends_with("makefile") {
        return Some("makefile");
    }
    hljs_lang_for_ext(&ext)
}

/// Builds the per-line HTML fragments. When `language` is `Some`, highlight.js
/// is invoked first and its output is split with span balancing; otherwise the
/// raw text is HTML-escaped and split on `\n`.
async fn prepare_lines(content: String, language: Option<&'static str>) -> (Vec<String>, Option<&'static str>) {
    if let Some(lang) = language {
        match highlight(&content, lang).await {
            Ok(html) => (split_highlighted_into_lines(&html), Some(lang)),
            Err(e) => {
                web_sys::console::warn_1(
                    &format!("hljs highlight {lang}: {e}; falling back to plain text").into(),
                );
                (escape_lines(&content), None)
            }
        }
    } else {
        (escape_lines(&content), None)
    }
}

fn escape_lines(content: &str) -> Vec<String> {
    let mut out: Vec<String> = content.split('\n').map(html_escape).collect();
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

#[component]
pub fn CodeView(
    workspace_id: u64,
    rel_path: String,
    reload_tick: ReadSignal<u32>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let result: RwSignal<Option<Result<PreparedCode, FilePreviewError>>> = RwSignal::new(None);
    let selected: RwSignal<Option<usize>> = RwSignal::new(None);

    let language_hint = lang_for_path(&rel_path);

    let rel_for_effect = rel_path.clone();
    Effect::new(move |_| {
        let _ = reload_tick.get();
        result.set(None);
        selected.set(None);
        if !is_tauri_shell() {
            result.set(Some(Err(FilePreviewError::NoTauri)));
            return;
        }
        let Some(ws) = wb
            .workspaces()
            .get()
            .into_iter()
            .find(|w| w.id == workspace_id)
        else {
            result.set(Some(Err(FilePreviewError::WorkspaceNotFound)));
            return;
        };
        let root = ws.cwd;
        let rel = rel_for_effect.clone();
        let lang = language_hint;
        spawn_local(async move {
            match read_workspace_text_file(root, rel).await {
                Ok(t) => {
                    let truncated = t.truncated;
                    let byte_len = t.byte_len;
                    let (lines, used_lang) = prepare_lines(t.content, lang).await;
                    result.set(Some(Ok(PreparedCode {
                        lines,
                        truncated,
                        byte_len,
                        language: used_lang,
                    })));
                }
                Err(e) => result.set(Some(Err(FilePreviewError::Failed(e)))),
            }
        });
    });

    view! {
        <div class="file-preview__stage file-preview__stage--code">
            {move || match result.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => render_load_error(i18n, I18nKey::FilePreviewLoadFailedText, err),
                Some(Ok(prepared)) => render_code(prepared, selected, i18n).into_any(),
            }}
        </div>
    }
}

fn render_code(
    prepared: PreparedCode,
    selected: RwSignal<Option<usize>>,
    i18n: I18nService,
) -> impl IntoView {
    let truncated = prepared.truncated;
    let byte_len = prepared.byte_len;
    let language = prepared.language;
    let total_lines = prepared.lines.len();
    let gutter_width_ch = total_lines.to_string().len().max(2) + 1;

    let row_views: Vec<_> = prepared
        .lines
        .into_iter()
        .enumerate()
        .map(|(idx, html)| {
            let line_no = idx + 1;
            view! {
                <div
                    class="code-view__row"
                    class:code-view__row--selected=move || selected.get() == Some(line_no)
                    data-line=line_no.to_string()
                >
                    <span class="code-view__lineno" aria-hidden="true">{line_no}</span>
                    <span class="code-view__line" inner_html=html />
                </div>
            }
        })
        .collect();

    let container_class = {
        let mut c = String::from("code-view");
        if language.is_some() {
            c.push_str(" code-view--hljs hljs");
        } else {
            c.push_str(" code-view--plain");
        }
        c
    };

    view! {
        <Show when=move || truncated>
            <div class="file-preview__notice">
                {move || i18n
                    .tr(I18nKey::FilePreviewTextTruncated)()
                    .replace("{bytes}", &byte_len.to_string())}
            </div>
        </Show>
        <div
            class=container_class
            style=format!("--code-view-gutter-width: {gutter_width_ch}ch;")
            on:click=move |ev| {
                use wasm_bindgen::JsCast;
                let Some(target) = ev
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                else {
                    return;
                };
                let Ok(Some(row)) = target.closest("[data-line]") else {
                    return;
                };
                let Some(line_str) = row.get_attribute("data-line") else {
                    return;
                };
                let Ok(line_no) = line_str.parse::<usize>() else {
                    return;
                };
                selected.update(|s| {
                    *s = if *s == Some(line_no) { None } else { Some(line_no) };
                });
            }
        >
            {row_views}
        </div>
    }
}
