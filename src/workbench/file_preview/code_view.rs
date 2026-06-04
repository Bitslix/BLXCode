//! Source-code preview + editor, both backed by CodeMirror 6.
//!
//! Read (preview) and edit modes mount the *same* CodeMirror 6 editor
//! (`super::editor::code_mirror::CodeMirrorEditor`); the only difference is the
//! `read_only` flag. Syntax highlighting, gutter, folding, selection and the
//! right-click handoff menu therefore look and behave identically in both
//! modes — preview is just edit mode with writes disabled.
//!
//! All editor state lives in the shared [`EditorSession`] (created by
//! `FilePreviewDock`), so the same handle drives the header controls.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{pty_write, FileKind, PolicyKind};
use crate::workbench::agent_context_handoff::{
    file_snippet_context_item, render_file_snippet_envelope,
};
use crate::workbench::file_preview::code_context_menu::{
    CodeContextMenu, CodeContextMenuState, CodeMenuAction,
};
use crate::workbench::file_preview::editor::code_mirror::CodeMirrorEditor;
use crate::workbench::file_preview::editor::policy::Editability;
use crate::workbench::file_preview::editor::{DocStatus, EditMode, EditorSession};
use crate::workbench::file_preview::util::{
    build_file_snippet_block, hljs_lang_for_ext, render_load_error, FilePreviewError,
};
use crate::workbench::toast::ToastService;
use crate::workbench::WorkbenchService;
use base64::Engine;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;
use wasm_bindgen::JsCast;

/// Coarse render phase. Derived via a deduplicating `Memo` so the CodeMirror
/// editor is only (re)mounted when we move between loading / error / content,
/// not on every buffer change.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    Error,
    Content,
}

/// Resolves the highlight.js language alias for `rel_path`'s extension. Used to
/// tag fenced snippets in the handoff menu; returns `None` for extensions with
/// no reliable mapping (plain text).
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

/// Maps `rel_path`'s extension to a CodeMirror language name understood by the
/// vendored bundle's `create({ language })` (see `cm-entry.js`). Returns `None`
/// for the few extensions with no bundled grammar (rendered as plain text).
fn cm_lang_for_path(rel_path: &str) -> Option<&'static str> {
    let lower = rel_path.to_ascii_lowercase();
    if lower.ends_with("dockerfile") || lower.ends_with("containerfile") {
        return Some("dockerfile");
    }
    if lower.ends_with("makefile") {
        return None; // no bundled Makefile grammar
    }
    let ext = lower.rsplit('.').next()?;
    Some(match ext {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "js" | "mjs" | "cjs" => "javascript",
        "py" | "pyw" | "pyi" => "python",
        "json" | "json5" | "jsonc" | "edn" => "json",
        "css" | "scss" | "sass" | "less" | "styl" => "css",
        "html" | "htm" | "xhtml" | "vue" | "svelte" => "html",
        "md" | "markdown" => "markdown",
        "xml" | "svg" | "plist" => "xml",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "m" | "mm" => "objc",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" | "sc" => "scala",
        "groovy" | "gradle" => "groovy",
        "cs" => "csharp",
        "fs" | "fsx" => "fsharp",
        "vb" => "vb",
        "dart" => "dart",
        "php" | "phtml" => "php",
        "sql" => "sql",
        "yaml" | "yml" => "yaml",
        "go" => "go",
        "rb" | "erb" => "ruby",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "r" => "r",
        "jl" => "julia",
        "clj" | "cljs" | "cljc" => "clojure",
        "erl" | "hrl" => "erlang",
        "hs" | "lhs" => "haskell",
        "swift" => "swift",
        "ml" | "mli" | "ocaml" => "ocaml",
        "sh" | "bash" | "zsh" | "fish" => "shell",
        "ps1" => "powershell",
        "toml" => "toml",
        "ini" | "conf" | "cfg" | "env" | "properties" | "editorconfig" | "gitattributes" => {
            "properties"
        }
        "proto" => "protobuf",
        "cmake" | "mk" => "cmake",
        "diff" | "patch" => "diff",
        _ => return None,
    })
}

/// Code preview/editor bound to a shared [`EditorSession`]. `kind`/`policy_kind`
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

    let menu_state: RwSignal<Option<CodeContextMenuState>> = RwSignal::new(None);

    let rel_path = session.rel_path();
    let language_hint = lang_for_path(&rel_path);

    // (Re)load on reload tick.
    Effect::new(move |_| {
        let _ = reload_tick.get();
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

    // Close the handoff context menu on an outside click or Escape.
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
        // Snippet / clipboard text comes straight from the live buffer, so it is
        // correct in both preview and edit mode (CodeMirror owns highlighting;
        // there is no separate plain-text mirror to keep in sync).
        let plain: Vec<String> = session
            .buffer
            .with_untracked(|b| b.split('\n').map(str::to_owned).collect());
        handle_menu_action(
            action,
            wb,
            i18n,
            toast,
            session.workspace_id,
            &session.rel_path(),
            language_hint,
            menu.range,
            Arc::new(plain),
        );
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
                    // Both preview and edit mount the same CodeMirror editor;
                    // only `read_only` differs, so they share gutter, folding,
                    // selection and the right-click handoff menu. Reading `mode`
                    // here remounts the editor when toggling View/Edit, not on
                    // every keystroke.
                    Phase::Content => {
                        let read_only = !matches!(session.mode.get(), EditMode::Edit);
                        view! {
                            <CodeMirrorEditor
                                session=session
                                language=cm_lang_for_path(&session.rel_path())
                                read_only=read_only
                                menu_state=menu_state
                            />
                        }.into_any()
                    }
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
    use super::cm_lang_for_path;

    #[test]
    fn cm_lang_maps_common_extensions() {
        assert_eq!(cm_lang_for_path("src/main.rs"), Some("rust"));
        assert_eq!(cm_lang_for_path("a/b.tsx"), Some("tsx"));
        assert_eq!(cm_lang_for_path("x.py"), Some("python"));
        assert_eq!(cm_lang_for_path("s.sh"), Some("shell"));
        assert_eq!(cm_lang_for_path("Cargo.toml"), Some("toml"));
        assert_eq!(cm_lang_for_path("app.rb"), Some("ruby"));
        assert_eq!(cm_lang_for_path("m.kt"), Some("kotlin"));
        assert_eq!(cm_lang_for_path(".env"), Some("properties"));
        assert_eq!(cm_lang_for_path("Dockerfile"), Some("dockerfile"));
        assert_eq!(cm_lang_for_path("a.patch"), Some("diff"));
    }

    #[test]
    fn cm_lang_unknown_is_none() {
        assert_eq!(cm_lang_for_path("notes.unknownext"), None);
        assert_eq!(cm_lang_for_path("Makefile"), None);
        assert_eq!(cm_lang_for_path("noext"), None);
    }
}
