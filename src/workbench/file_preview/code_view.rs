//! Source-code preview with line numbers, syntax highlighting via
//! highlight.js and click/drag row range selection.
//!
//! Pure text (e.g. `.txt`, `.log`, `.env`) is rendered through this same
//! component without syntax highlighting, but still receives line numbers
//! and row selection.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, pty_write, read_workspace_text_file};
use crate::workbench::agent_context_handoff::{
    file_snippet_context_item, list_terminal_targets_all_workspaces, render_file_snippet_envelope,
};
use crate::workbench::file_preview::code_context_menu::{
    CodeContextMenu, CodeContextMenuState, CodeMenuAction,
};
use crate::workbench::file_preview::hljs_glue::highlight;
use crate::workbench::file_preview::util::{
    build_file_snippet_block, hljs_lang_for_ext, html_escape, render_load_error,
    split_highlighted_into_lines, FilePreviewError,
};
use crate::workbench::toast::ToastService;
use crate::workbench::WorkbenchService;
use base64::Engine;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;

/// Pre-rendered code data: one HTML fragment per source line plus a raw
/// plaintext mirror used for snippet/clipboard handoff.
#[derive(Clone)]
struct PreparedCode {
    /// Already-escaped (and possibly hljs-highlighted) HTML fragments, one per
    /// line. Safe to embed via `inner_html`.
    lines: Vec<String>,
    /// Raw text per line (no HTML), mirrored 1:1 with `lines`. Used to build
    /// fenced-code snippets and clipboard payloads.
    plain_lines: Arc<Vec<String>>,
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

/// Returns `(html_lines, raw_lines, used_language)`. When `language` is
/// `Some`, highlight.js is invoked first and its output is split with span
/// balancing; otherwise the raw text is HTML-escaped and split on `\n`.
/// `raw_lines` is always the plain `\n`-split source.
async fn prepare_lines(
    content: String,
    language: Option<&'static str>,
) -> (Vec<String>, Vec<String>, Option<&'static str>) {
    let plain: Vec<String> = if content.is_empty() {
        vec![String::new()]
    } else {
        content.split('\n').map(str::to_owned).collect()
    };
    if let Some(lang) = language {
        match highlight(&content, lang).await {
            Ok(html) => (split_highlighted_into_lines(&html), plain, Some(lang)),
            Err(e) => {
                web_sys::console::warn_1(
                    &format!("hljs highlight {lang}: {e}; falling back to plain text").into(),
                );
                (escape_lines(&content), plain, None)
            }
        }
    } else {
        (escape_lines(&content), plain, None)
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
    let toast = expect_context::<ToastService>();
    let result: RwSignal<Option<Result<PreparedCode, FilePreviewError>>> = RwSignal::new(None);
    // Selection is always stored as an ordered (start, end) pair, 1-based,
    // inclusive. `None` means "no selection".
    let selected: RwSignal<Option<(usize, usize)>> = RwSignal::new(None);
    // Drag anchor: the row the user pressed mousedown on. `Some(line)` means
    // a drag is in progress. Cleared on global mouseup.
    let drag_anchor: RwSignal<Option<usize>> = RwSignal::new(None);
    // `true` once a drag has crossed at least one row boundary. Lets us
    // distinguish a pure click (toggle) from a drag (replace selection).
    let drag_moved: RwSignal<bool> = RwSignal::new(false);
    // Context menu state. `Some(_)` is open; `None` is closed.
    let menu_state: RwSignal<Option<CodeContextMenuState>> = RwSignal::new(None);

    let language_hint = lang_for_path(&rel_path);

    let rel_for_effect = rel_path.clone();
    Effect::new(move |_| {
        // Only react to `reload_tick`. Reading `wb.workspaces().get()`
        // reactively would re-fetch + remount on every tab switch (see
        // FilePreviewDock for the same fix).
        let _ = reload_tick.get();
        result.set(None);
        selected.set(None);
        drag_anchor.set(None);
        drag_moved.set(false);
        menu_state.set(None);
        if !is_tauri_shell() {
            result.set(Some(Err(FilePreviewError::NoTauri)));
            return;
        }
        let Some(root) = wb.workspaces().with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.cwd.clone())
        }) else {
            result.set(Some(Err(FilePreviewError::WorkspaceNotFound)));
            return;
        };
        let rel = rel_for_effect.clone();
        let lang = language_hint;
        spawn_local(async move {
            match read_workspace_text_file(root, rel).await {
                Ok(t) => {
                    let truncated = t.truncated;
                    let byte_len = t.byte_len;
                    let (lines, plain, used_lang) = prepare_lines(t.content, lang).await;
                    result.set(Some(Ok(PreparedCode {
                        lines,
                        plain_lines: Arc::new(plain),
                        truncated,
                        byte_len,
                        language: used_lang,
                    })));
                }
                Err(e) => result.set(Some(Err(FilePreviewError::Failed(e)))),
            }
        });
    });

    // Window-level mouseup ends any in-progress drag, even if the pointer
    // left the code area. We register once and clean up with on_cleanup.
    let mouseup_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("mouseup", move |_| {
            if drag_anchor.get_untracked().is_some() {
                drag_anchor.set(None);
                drag_moved.set(false);
            }
        });
    on_cleanup(move || drop(mouseup_handle));

    // Click anywhere closes the menu. We listen at window level instead of
    // installing per-element listeners so the menu closes consistently for
    // every dismissal path (clicking another row, the page background, etc.).
    let click_close_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("mousedown", move |_| {
            if menu_state.get_untracked().is_some() {
                menu_state.set(None);
            }
        });
    on_cleanup(move || drop(click_close_handle));

    // Escape key also closes the menu.
    let escape_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("keydown", move |ev| {
            let Some(kev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            if kev.key() == "Escape" && menu_state.get_untracked().is_some() {
                menu_state.set(None);
            }
        });
    on_cleanup(move || drop(escape_handle));

    let rel_path_for_actions = rel_path.clone();
    let on_action = Callback::new(move |action: CodeMenuAction| {
        let Some(menu) = menu_state.get_untracked() else {
            return;
        };
        menu_state.set(None);
        let Some(Ok(prepared)) = result.get_untracked() else {
            return;
        };
        let plain = prepared.plain_lines.clone();
        let lang_tag = prepared.language;
        handle_menu_action(
            action,
            wb,
            i18n,
            toast,
            workspace_id,
            &rel_path_for_actions,
            lang_tag,
            menu.range,
            plain,
        );
    });

    view! {
        <div class="file-preview__stage file-preview__stage--code">
            {move || match result.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => render_load_error(i18n, I18nKey::FilePreviewLoadFailedText, err),
                Some(Ok(prepared)) => render_code(
                    prepared,
                    selected,
                    drag_anchor,
                    drag_moved,
                    menu_state,
                    workspace_id,
                    i18n,
                    wb,
                ).into_any(),
            }}
            <CodeContextMenu state=menu_state on_action=on_action />
        </div>
    }
}

#[allow(clippy::too_many_arguments)]
fn render_code(
    prepared: PreparedCode,
    selected: RwSignal<Option<(usize, usize)>>,
    drag_anchor: RwSignal<Option<usize>>,
    drag_moved: RwSignal<bool>,
    menu_state: RwSignal<Option<CodeContextMenuState>>,
    workspace_id: u64,
    i18n: I18nService,
    wb: WorkbenchService,
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
                    class:code-view__row--selected=move || {
                        selected
                            .get()
                            .map(|(s, e)| s <= line_no && line_no <= e)
                            .unwrap_or(false)
                    }
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
            on:mousedown=move |ev: MouseEvent| {
                // Only left-button drags select.
                if ev.button() != 0 {
                    return;
                }
                let Some(line_no) = closest_data_line(&ev) else {
                    return;
                };
                drag_anchor.set(Some(line_no));
                drag_moved.set(false);
                selected.set(Some((line_no, line_no)));
            }
            on:mousemove=move |ev: MouseEvent| {
                let Some(anchor) = drag_anchor.get_untracked() else {
                    return;
                };
                let Some(line_no) = closest_data_line(&ev) else {
                    return;
                };
                if line_no != anchor {
                    drag_moved.set(true);
                }
                let (s, e) = if anchor <= line_no {
                    (anchor, line_no)
                } else {
                    (line_no, anchor)
                };
                selected.update(|cur| {
                    if *cur != Some((s, e)) {
                        *cur = Some((s, e));
                    }
                });
            }
            on:click=move |ev: MouseEvent| {
                // Pure click (no drag): toggle a single-line selection. The
                // mousedown above already set `Some((n, n))`; if no drag
                // happened, we treat a second click on the same line as a
                // deselect to match the previous UX.
                if drag_moved.get_untracked() {
                    return;
                }
                let Some(line_no) = closest_data_line(&ev) else {
                    return;
                };
                selected.update(|cur| {
                    if *cur == Some((line_no, line_no)) {
                        *cur = None;
                    } else {
                        *cur = Some((line_no, line_no));
                    }
                });
            }
            on:contextmenu=move |ev: MouseEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                let Some(line_no) = closest_data_line(&ev) else {
                    return;
                };
                // If the click happened outside the current range, replace
                // the selection with that single line.
                selected.update(|cur| match *cur {
                    Some((s, e)) if s <= line_no && line_no <= e => {}
                    _ => *cur = Some((line_no, line_no)),
                });
                let range = selected.get_untracked().unwrap_or((line_no, line_no));
                let groups = list_terminal_targets_all_workspaces(&wb, Some(workspace_id));
                menu_state.set(Some(CodeContextMenuState {
                    anchor_x: ev.client_x(),
                    anchor_y: ev.client_y(),
                    range: (range.0 as u32, range.1 as u32),
                    groups,
                    preview_workspace_id: workspace_id,
                }));
            }
        >
            {row_views}
        </div>
    }
}

fn closest_data_line(ev: &MouseEvent) -> Option<usize> {
    let target = ev
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())?;
    let row = target.closest("[data-line]").ok().flatten()?;
    row.get_attribute("data-line")
        .and_then(|s| s.parse::<usize>().ok())
}

#[allow(clippy::too_many_arguments)]
fn handle_menu_action(
    action: CodeMenuAction,
    wb: WorkbenchService,
    i18n: I18nService,
    toast: ToastService,
    preview_workspace_id: u64,
    rel_path: &str,
    language: Option<&'static str>,
    range: (u32, u32),
    plain_lines: Arc<Vec<String>>,
) {
    let preview_workspace_label = wb.workspaces().with_untracked(|all| {
        all.iter()
            .find(|w| w.id == preview_workspace_id)
            .map(|w| {
                if w.title.trim().is_empty() {
                    w.cwd.clone()
                } else {
                    w.title.trim().to_owned()
                }
            })
            .unwrap_or_default()
    });

    match action {
        CodeMenuAction::InsertSnippetIntoTerminal {
            workspace_id,
            workspace_label,
            target,
        } => {
            let cross = workspace_id != preview_workspace_id;
            let snippet = build_file_snippet_block(
                rel_path,
                language,
                &plain_lines,
                range,
                if cross {
                    Some(&preview_workspace_label)
                } else {
                    None
                },
            );
            let payload = if snippet.ends_with('\n') {
                snippet
            } else {
                format!("{snippet}\n")
            };
            let toast = toast;
            let i18n_for_msg = i18n;
            let target_label = target.label.clone();
            let ws_label = workspace_label.clone();
            let session_id = target.session_id;
            spawn_local(async move {
                let b64 = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
                match pty_write(session_id, b64).await {
                    Ok(()) => {
                        let msg = i18n_for_msg.tr(I18nKey::CodeViewToastSnippetInsertedTerminal)()
                            .replace("{terminal}", &target_label)
                            .replace("{workspace}", &ws_label);
                        toast.success(msg);
                    }
                    Err(e) => {
                        let msg = i18n_for_msg.tr(I18nKey::CodeViewToastInsertFailed)()
                            .replace("{error}", &e);
                        toast.error(msg);
                    }
                }
            });
        }
        CodeMenuAction::InsertEnvelopeIntoTerminal {
            workspace_id,
            workspace_label,
            target,
        } => {
            let cross = workspace_id != preview_workspace_id;
            let snippet = build_file_snippet_block(
                rel_path,
                language,
                &plain_lines,
                range,
                if cross {
                    Some(&preview_workspace_label)
                } else {
                    None
                },
            );
            let target_workspace_root = wb.workspaces().with_untracked(|all| {
                all.iter()
                    .find(|w| w.id == workspace_id)
                    .map(|w| w.cwd.clone())
                    .unwrap_or_default()
            });
            let agent_slug = if target.agent_slug.is_empty() {
                None
            } else {
                Some(target.agent_slug.clone())
            };
            let envelope = render_file_snippet_envelope(
                Some(&target_workspace_root),
                Some(target.slot_id),
                agent_slug.as_deref(),
                rel_path,
                range,
                language,
                &snippet,
                if cross {
                    Some(&preview_workspace_label)
                } else {
                    None
                },
            );
            let toast = toast;
            let i18n_for_msg = i18n;
            let target_label = target.label.clone();
            let ws_label = workspace_label.clone();
            let session_id = target.session_id;
            spawn_local(async move {
                let b64 = base64::engine::general_purpose::STANDARD.encode(envelope.as_bytes());
                match pty_write(session_id, b64).await {
                    Ok(()) => {
                        let msg = i18n_for_msg.tr(I18nKey::CodeViewToastEnvelopeInsertedTerminal)()
                            .replace("{terminal}", &target_label)
                            .replace("{workspace}", &ws_label);
                        toast.success(msg);
                    }
                    Err(e) => {
                        let msg = i18n_for_msg.tr(I18nKey::CodeViewToastInsertFailed)()
                            .replace("{error}", &e);
                        toast.error(msg);
                    }
                }
            });
        }
        CodeMenuAction::AttachToAgent {
            workspace_id,
            workspace_label,
        } => {
            let cross = workspace_id != preview_workspace_id;
            let snippet = build_file_snippet_block(
                rel_path,
                language,
                &plain_lines,
                range,
                if cross {
                    Some(&preview_workspace_label)
                } else {
                    None
                },
            );
            let item_label = if range.0 == range.1 {
                format!("Snippet · {}:{}", rel_path, range.0)
            } else {
                format!("Snippet · {}:{}-{}", rel_path, range.0, range.1)
            };
            let item = file_snippet_context_item(
                rel_path,
                range.0,
                range.1,
                language,
                &item_label,
                &snippet,
                if cross {
                    Some(&preview_workspace_label)
                } else {
                    None
                },
            );
            wb.upsert_workspace_agent_context(workspace_id, item);
            let msg = i18n.tr(I18nKey::CodeViewToastAgentAttached)()
                .replace("{workspace}", &workspace_label);
            toast.success(msg);
        }
        CodeMenuAction::CopySnippet => {
            let cross_label_unused = (); // always preview workspace for clipboard
            let _ = cross_label_unused;
            let snippet = build_file_snippet_block(rel_path, language, &plain_lines, range, None);
            copy_to_clipboard(snippet, i18n, toast, I18nKey::CodeViewToastCopiedSnippet);
        }
        CodeMenuAction::CopyRange => {
            let total = plain_lines.len() as u32;
            if total == 0 {
                return;
            }
            let s = range.0.max(1).min(total);
            let e = range.1.max(s).min(total);
            let body = plain_lines[(s - 1) as usize..=(e - 1) as usize].join("\n");
            copy_to_clipboard(body, i18n, toast, I18nKey::CodeViewToastCopiedRange);
        }
        CodeMenuAction::CopyRaw => {
            let raw = plain_lines.join("\n");
            copy_to_clipboard(raw, i18n, toast, I18nKey::CodeViewToastCopiedRaw);
        }
    }
}

fn copy_to_clipboard(text: String, i18n: I18nService, toast: ToastService, success_key: I18nKey) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let clipboard = window.navigator().clipboard();
    let promise = clipboard.write_text(&text);
    let toast = toast;
    let i18n_for_msg = i18n;
    spawn_local(async move {
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(_) => toast.success(i18n_for_msg.tr(success_key)()),
            Err(e) => {
                let err = e
                    .as_string()
                    .or_else(|| {
                        js_sys::Reflect::get(&e, &"message".into())
                            .ok()
                            .and_then(|v| v.as_string())
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                let msg = i18n_for_msg.tr(I18nKey::CodeViewToastClipboardFailed)()
                    .replace("{error}", &err);
                toast.error(msg);
            }
        }
    });
}
