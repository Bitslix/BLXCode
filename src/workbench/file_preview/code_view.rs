//! Source-code view + lightweight editor.
//!
//! Read mode renders line numbers, highlight.js syntax highlighting, click/drag
//! row selection, a context menu, and (view-mode) code folding. Edit mode
//! reuses the *same* highlighted backdrop and overlays a transparent
//! `<textarea>` so highlighting, caret, IME and native undo all come for free —
//! the backdrop is re-highlighted on a debounce as the buffer changes.
//!
//! All editor state lives in the shared [`EditorSession`] (created by
//! `FilePreviewDock`), so the same handle drives the header controls.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{pty_write, FileKind, PolicyKind};
use crate::workbench::agent_context_handoff::{
    file_snippet_context_item, list_terminal_targets_all_workspaces, render_file_snippet_envelope,
};
use crate::workbench::file_preview::code_context_menu::{
    CodeContextMenu, CodeContextMenuState, CodeMenuAction,
};
use crate::workbench::file_preview::editor::folding::compute_folds;
use crate::workbench::file_preview::editor::policy::Editability;
use crate::workbench::file_preview::editor::{DocStatus, EditMode, EditorSession};
use crate::workbench::file_preview::hljs_glue::highlight;
use crate::workbench::file_preview::util::{
    build_file_snippet_block, hljs_lang_for_ext, html_escape, render_load_error,
    split_highlighted_into_lines, FilePreviewError,
};
use crate::workbench::toast::ToastService;
use crate::workbench::{HarnessUiService, WorkbenchService};
use base64::Engine;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;

/// Debounce before re-highlighting while typing (ms).
const HIGHLIGHT_DEBOUNCE_MS: u32 = 90;
/// Above this buffer size we skip syntax highlighting (plain text) to keep
/// keystrokes responsive.
const MAX_HIGHLIGHT_BYTES: usize = 256 * 1024;

/// Coarse render phase. Derived via a deduplicating `Memo` so the editing
/// scaffold (and the textarea inside it) is *not* rebuilt as the buffer is
/// highlighted or saved — only when we move between loading / error / content.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    Error,
    Content,
}

/// Pre-rendered code data: one HTML fragment per source line plus a raw
/// plaintext mirror used for snippet/clipboard handoff.
#[derive(Clone)]
struct PreparedCode {
    /// Already-escaped (and possibly hljs-highlighted) HTML fragments, one per
    /// line. Safe to embed via `inner_html`.
    lines: Vec<String>,
    /// Raw text per line (no HTML), mirrored 1:1 with `lines`.
    plain_lines: Arc<Vec<String>>,
    /// Language alias that was highlighted with, or `None` for plain text.
    language: Option<&'static str>,
}

/// Resolves the language hint for `rel_path`'s extension. Returns `None` for
/// extensions that have no reliable highlight.js mapping (plain text path).
pub fn lang_for_path(rel_path: &str) -> Option<&'static str> {
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

/// Returns `(html_lines, raw_lines, used_language)`.
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

/// Code view/editor bound to a shared [`EditorSession`]. `kind`/`policy_kind`
/// set the editability policy; `reload_tick` forces a re-read from disk.
#[component]
pub fn CodeView(
    session: EditorSession,
    kind: FileKind,
    #[prop(default = None)] policy_kind: Option<PolicyKind>,
    reload_tick: ReadSignal<u32>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();
    let ui = expect_context::<HarnessUiService>();

    let prepared: RwSignal<Option<PreparedCode>> = RwSignal::new(None);
    let selected: RwSignal<Option<(usize, usize)>> = RwSignal::new(None);
    let drag_anchor: RwSignal<Option<usize>> = RwSignal::new(None);
    let drag_moved: RwSignal<bool> = RwSignal::new(false);
    let menu_state: RwSignal<Option<CodeContextMenuState>> = RwSignal::new(None);

    let rel_path = session.rel_path();
    let language_hint = lang_for_path(&rel_path);

    // (Re)load on reload tick. The editability policy is recomputed once the
    // backend reports truncation; do it here so a too-large file is read-only.
    Effect::new(move |_| {
        let _ = reload_tick.get();
        selected.set(None);
        drag_anchor.set(None);
        drag_moved.set(false);
        menu_state.set(None);
        session.reload(wb);
    });

    // The base editability is set centrally by `FilePreviewDock` from the file
    // metadata. Truncation is only known after the read returns, so downgrade
    // a too-large file to read-only here.
    let _ = (kind, policy_kind);
    Effect::new(move |_| {
        if matches!(session.status.get(), DocStatus::TooLarge) {
            session.editability.set(Editability::NeverEdit);
            if matches!(session.mode.get_untracked(), EditMode::Edit) {
                session.mode.set(EditMode::View);
            }
        }
    });

    // Re-highlight the backdrop from the live buffer. Debounced while editing
    // so a burst of keystrokes collapses into one highlight pass.
    let highlight_gen = RwSignal::new(0u32);
    Effect::new(move |_| {
        let text = session.buffer.get();
        let editing = matches!(session.mode.get(), EditMode::Edit);
        let lang = language_hint;
        let gen = highlight_gen.get_untracked().wrapping_add(1);
        highlight_gen.set(gen);
        spawn_local(async move {
            if editing {
                TimeoutFuture::new(HIGHLIGHT_DEBOUNCE_MS).await;
                if highlight_gen.get_untracked() != gen {
                    return; // superseded by a newer keystroke
                }
            }
            let lang_use = if text.len() > MAX_HIGHLIGHT_BYTES {
                None
            } else {
                lang
            };
            let folds = compute_folds(&text, lang_use);
            let (lines, plain, used) = prepare_lines(text, lang_use).await;
            if highlight_gen.get_untracked() != gen {
                return;
            }
            session.folds.update(|f| f.ranges = folds);
            prepared.set(Some(PreparedCode {
                lines,
                plain_lines: Arc::new(plain),
                language: used,
            }));
        });
    });

    // Window-level mouseup ends any in-progress drag (view mode only).
    let mouseup_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("mouseup", move |_| {
            if matches!(drag_anchor.try_get_untracked(), Some(Some(_))) {
                drag_anchor.set(None);
                drag_moved.set(false);
            }
        });
    on_cleanup(move || drop(mouseup_handle));

    let click_close_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("mousedown", move |_| {
            if matches!(menu_state.try_get_untracked(), Some(Some(_))) {
                menu_state.set(None);
            }
        });
    on_cleanup(move || drop(click_close_handle));

    let escape_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("keydown", move |ev| {
            let Some(kev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            if kev.key() == "Escape" && matches!(menu_state.try_get_untracked(), Some(Some(_))) {
                menu_state.set(None);
            }
        });
    on_cleanup(move || drop(escape_handle));

    let on_action = Callback::new(move |action: CodeMenuAction| {
        let Some(menu) = menu_state.get_untracked() else {
            return;
        };
        menu_state.set(None);
        let Some(prepared) = prepared.get_untracked() else {
            return;
        };
        let plain = prepared.plain_lines.clone();
        let lang_tag = prepared.language;
        handle_menu_action(
            action,
            wb,
            i18n,
            toast,
            session.workspace_id,
            &session.rel_path(),
            lang_tag,
            menu.range,
            plain,
        );
    });

    // Gutter width tracks the live line count so the textarea indent stays
    // aligned with the numbers even before a re-highlight lands.
    let gutter_ch = Memo::new(move |_| {
        let n = session.buffer.with(|b| b.split('\n').count()).max(1);
        n.to_string().len().max(2) + 1
    });
    let phase = Memo::new(move |_| match session.status.get() {
        DocStatus::Loading => Phase::Loading,
        DocStatus::Error(_) => Phase::Error,
        _ => Phase::Content,
    });

    view! {
        <div class="file-preview__stage file-preview__stage--code">
            {move || render_banner(session, i18n)}
            {move || {
                match phase.get() {
                    Phase::Loading => view! {
                        <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                    }.into_any(),
                    Phase::Error => {
                        let msg = session.status.with_untracked(|s| match s {
                            DocStatus::Error(e) => e.clone(),
                            _ => String::new(),
                        });
                        render_load_error(
                            i18n,
                            I18nKey::FilePreviewLoadFailedText,
                            FilePreviewError::Failed(msg),
                        )
                    }
                    // The editing scaffold + textarea are built once per
                    // enter-edit (this closure only depends on `phase` + `mode`),
                    // while the highlighted backdrop re-renders on `prepared`.
                    Phase::Content => if matches!(session.mode.get(), EditMode::Edit) {
                        view! {
                            <div class="code-view__edit-host">
                                <div
                                    class="code-view__edit-inner"
                                    style=move || format!("--code-view-gutter-width: {}ch;", gutter_ch.get())
                                >
                                    {move || prepared.get().map(|p| render_backdrop(
                                        p, session, selected, drag_anchor, drag_moved,
                                        menu_state, i18n, wb, true, gutter_ch.get_untracked(),
                                    ))}
                                    <CodeTextarea session=session toast=toast ui=ui wb=wb i18n=i18n menu_state=menu_state />
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            {move || prepared.get().map(|p| render_backdrop(
                                p, session, selected, drag_anchor, drag_moved,
                                menu_state, i18n, wb, false, gutter_ch.get_untracked(),
                            ))}
                        }.into_any()
                    },
                }
            }}
            <CodeContextMenu state=menu_state on_action=on_action />
        </div>
    }
}

/// A read-only / protected / too-large explanatory banner above the code area.
/// `None` when no banner applies.
fn render_banner(session: EditorSession, i18n: I18nService) -> Option<AnyView> {
    let editability = session.editability.get();
    let mode = session.mode.get();
    let status = session.status.get();
    let key = match (status, editability, mode) {
        (DocStatus::TooLarge, _, _) => I18nKey::FilePreviewEditorTooLargeBanner,
        (_, Editability::NeverEdit, _) => I18nKey::FilePreviewEditorProtectedBanner,
        (_, Editability::ReadOnlyByDefault, EditMode::View) => {
            I18nKey::FilePreviewEditorReadOnlyBanner
        }
        _ => return None,
    };
    Some(view! { <div class="file-preview__notice">{i18n.tr(key)}</div> }.into_any())
}

/// Renders the highlighted, line-numbered `.code-view` backdrop. In view mode
/// it carries selection + context-menu + fold interactions; in edit mode it is
/// purely visual (the overlay textarea, mounted separately, owns input).
#[allow(clippy::too_many_arguments)]
fn render_backdrop(
    prepared: PreparedCode,
    session: EditorSession,
    selected: RwSignal<Option<(usize, usize)>>,
    drag_anchor: RwSignal<Option<usize>>,
    drag_moved: RwSignal<bool>,
    menu_state: RwSignal<Option<CodeContextMenuState>>,
    i18n: I18nService,
    wb: WorkbenchService,
    editing: bool,
    gutter_width_ch: usize,
) -> impl IntoView {
    let workspace_id = session.workspace_id;
    let language = prepared.language;

    let row_views: Vec<_> = prepared
        .lines
        .into_iter()
        .enumerate()
        .map(|(idx, html)| {
            let line_no = idx + 1;
            let has_fold = move || {
                session
                    .folds
                    .with(|f| f.range_starting_at(line_no).is_some())
            };
            let collapsed = move || session.folds.with(|f| f.collapsed.contains(&line_no));
            // View-mode folding hides rows inside a collapsed range.
            let hidden = move || !editing && session.folds.with(|f| f.is_hidden(line_no));
            view! {
                <div
                    class="code-view__row"
                    class:code-view__row--selected=move || {
                        selected
                            .get()
                            .map(|(s, e)| s <= line_no && line_no <= e)
                            .unwrap_or(false)
                    }
                    class:code-view__row--hidden=hidden
                    data-line=line_no.to_string()
                >
                    <span class="code-view__gutter">
                        <Show when=move || !editing && has_fold()>
                            <button
                                type="button"
                                class="code-view__fold-toggle"
                                class:code-view__fold-toggle--collapsed=collapsed
                                title=move || if collapsed() {
                                    i18n.tr(I18nKey::FilePreviewEditorUnfold)().to_string()
                                } else {
                                    i18n.tr(I18nKey::FilePreviewEditorFold)().to_string()
                                }
                                aria-label=move || if collapsed() {
                                    i18n.tr(I18nKey::FilePreviewEditorUnfold)().to_string()
                                } else {
                                    i18n.tr(I18nKey::FilePreviewEditorFold)().to_string()
                                }
                                on:mousedown=move |ev: MouseEvent| {
                                    ev.stop_propagation();
                                    ev.prevent_default();
                                }
                                on:click=move |ev: MouseEvent| {
                                    ev.stop_propagation();
                                    session.folds.update(|f| f.toggle(line_no));
                                }
                            >
                                {move || if collapsed() { "▸" } else { "▾" }}
                            </button>
                        </Show>
                        <span class="code-view__lineno" aria-hidden="true">{line_no}</span>
                    </span>
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
        if editing {
            c.push_str(" code-view--editing");
        }
        c
    };

    // The backdrop carries selection/context-menu interactions only in view
    // mode; in edit mode the textarea handles selection and the backdrop is
    // purely visual (pointer-events disabled via CSS).
    let backdrop = view! {
        <div
            class=container_class
            style=format!("--code-view-gutter-width: {gutter_width_ch}ch;")
            on:mousedown=move |ev: MouseEvent| {
                if editing || ev.button() != 0 {
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
                if editing {
                    return;
                }
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
                if editing || drag_moved.get_untracked() {
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
                if editing {
                    return;
                }
                ev.prevent_default();
                ev.stop_propagation();
                let Some(line_no) = closest_data_line(&ev) else {
                    return;
                };
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
    };

    backdrop
}

/// 1-based line number containing the given selection offset. The offset is a
/// UTF-16 code-unit index (as returned by `selectionStart`/`selectionEnd`), so
/// we count `\n` among the first `offset` UTF-16 units — accurate regardless of
/// any multi-byte characters earlier in the buffer.
fn line_at_utf16_offset(value: &str, offset: usize) -> usize {
    value
        .encode_utf16()
        .take(offset)
        .filter(|u| *u == 0x000A)
        .count()
        + 1
}

/// The transparent editing surface layered over the highlighted backdrop.
#[component]
fn CodeTextarea(
    session: EditorSession,
    toast: ToastService,
    ui: HarnessUiService,
    wb: WorkbenchService,
    i18n: I18nService,
    menu_state: RwSignal<Option<CodeContextMenuState>>,
) -> impl IntoView {
    let ta_ref = NodeRef::<leptos::html::Textarea>::new();

    // Sync the DOM value from the buffer only when it actually differs, so a
    // self-originated keystroke never resets the caret. Depends on `mode` too
    // so entering edit mode populates the freshly-mounted textarea.
    Effect::new(move |_| {
        let v = session.buffer.get();
        let _ = session.mode.get();
        if let Some(el) = ta_ref.get() {
            if el.value() != v {
                el.set_value(&v);
            }
        }
    });

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        let key = ev.key();
        let mods = ev.ctrl_key() || ev.meta_key();
        if mods && key.eq_ignore_ascii_case("s") {
            ev.prevent_default();
            session.save(wb, toast, ui, i18n, false);
            return;
        }
        if key == "Escape" && !session.dirty.get_untracked() {
            ev.prevent_default();
            session.exit_edit();
            if let Some(el) = ta_ref.get() {
                let _ = el.blur();
            }
            return;
        }
        if key == "Tab" {
            // Insert two spaces at the caret rather than moving focus. Esc then
            // Tab still lets keyboard users leave the field. Offsets from the
            // DOM are UTF-16 units; only take the fast byte-slice path when they
            // line up with UTF-8 char boundaries (always true for ASCII), else
            // no-op rather than risk a panic on a non-boundary index.
            if let Some(el) = ta_ref.get() {
                ev.prevent_default();
                let start = el.selection_start().ok().flatten().unwrap_or(0);
                let end = el.selection_end().ok().flatten().unwrap_or(start);
                let (start, end) = (start as usize, end as usize);
                let mut value = el.value();
                if start <= end
                    && end <= value.len()
                    && value.is_char_boundary(start)
                    && value.is_char_boundary(end)
                {
                    value.replace_range(start..end, "  ");
                    el.set_value(&value);
                    let caret = (start + 2) as u32;
                    let _ = el.set_selection_range(caret, caret);
                    session.buffer.set(value);
                }
            }
        }
    };

    // Right-click opens the same handoff menu as view mode. The line range is
    // derived from the textarea selection: a collapsed caret yields its single
    // line; a non-empty selection yields the full span it covers.
    let on_contextmenu = move |ev: MouseEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        let Some(el) = ta_ref.get() else {
            return;
        };
        let value = el.value();
        let start = el.selection_start().ok().flatten().unwrap_or(0);
        let end = el.selection_end().ok().flatten().unwrap_or(start);
        let (start, end) = (start as usize, end as usize);
        let s_line = line_at_utf16_offset(&value, start);
        // For a real selection use the last *selected* unit so a selection that
        // stops at a line start doesn't pull in the following (empty) line.
        let e_off = if end > start { end - 1 } else { end };
        let e_line = line_at_utf16_offset(&value, e_off);
        let lo = s_line.min(e_line) as u32;
        let hi = s_line.max(e_line) as u32;
        let groups = list_terminal_targets_all_workspaces(&wb, Some(session.workspace_id));
        menu_state.set(Some(CodeContextMenuState {
            anchor_x: ev.client_x(),
            anchor_y: ev.client_y(),
            range: (lo, hi),
            groups,
            preview_workspace_id: session.workspace_id,
        }));
    };

    view! {
        <textarea
            class="code-view__textarea"
            spellcheck="false"
            node_ref=ta_ref
            on:input=move |ev| session.buffer.set(event_target_value(&ev))
            on:keydown=on_keydown
            on:contextmenu=on_contextmenu
        />
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

#[cfg(test)]
mod tests {
    use super::line_at_utf16_offset;

    #[test]
    fn line_at_offset_counts_newlines() {
        let s = "ab\ncd\nef";
        assert_eq!(line_at_utf16_offset(s, 0), 1); // start of line 1
        assert_eq!(line_at_utf16_offset(s, 2), 1); // before the first \n
        assert_eq!(line_at_utf16_offset(s, 3), 2); // just after the first \n
        assert_eq!(line_at_utf16_offset(s, 5), 2); // before the second \n
        assert_eq!(line_at_utf16_offset(s, 7), 3); // on line 3
    }

    #[test]
    fn line_at_offset_handles_multibyte_before_caret() {
        // "é" is 1 UTF-16 unit; "😀" is 2. Newline counting must stay correct.
        let s = "é😀\nx";
        // Offsets: é=1 unit, 😀=2 units, \n at unit index 3.
        assert_eq!(line_at_utf16_offset(s, 3), 1); // before the \n
        assert_eq!(line_at_utf16_offset(s, 4), 2); // after the \n
    }
}
