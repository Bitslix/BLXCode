//! CodeMirror 6 editing surface (edit mode). View mode keeps the highlight.js
//! backdrop; edit mode mounts a real CodeMirror editor so folding, selection,
//! and editing all work natively. State is bridged to the shared
//! [`EditorSession`]: edits flow into `session.buffer`, external buffer changes
//! (revert/reload) are pushed back into the editor, and `Mod-s` triggers save.

use super::EditorSession;
use crate::service::I18nService;
use crate::workbench::agent_context_handoff::list_terminal_targets_all_workspaces;
use crate::workbench::file_preview::code_context_menu::CodeContextMenuState;
use crate::workbench::file_preview::codemirror_glue as cm;
use crate::workbench::toast::ToastService;
use crate::workbench::{HarnessUiService, WorkbenchService};
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Keeps the wasm-bindgen closures alive for the lifetime of the editor (CM
/// holds JS references to them). Dropped on cleanup, after the view is torn
/// down.
type EditorClosures = (Closure<dyn Fn(String)>, Closure<dyn Fn()>);

#[component]
pub fn CodeMirrorEditor(
    session: EditorSession,
    #[prop(default = None)] language: Option<&'static str>,
    /// Mount the editor read-only (preview mode). Same chrome as edit mode, but
    /// edits and `Mod-s` save are disabled.
    #[prop(default = false)] read_only: bool,
    menu_state: RwSignal<Option<CodeContextMenuState>>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let toast = expect_context::<ToastService>();
    let ui = expect_context::<HarnessUiService>();
    let i18n = expect_context::<I18nService>();

    let host_ref = NodeRef::<leptos::html::Div>::new();
    // `JsValue` and the wasm-bindgen closures are !Send, so they live in the
    // thread-local (LocalStorage) arena rather than the default shared one.
    let view_handle = StoredValue::new_local(None::<JsValue>);
    let closures = StoredValue::new_local(None::<EditorClosures>);

    // Create the editor once the host element is mounted. Reads the buffer
    // untracked so this effect does not re-run while typing.
    Effect::new(move |_| {
        let Some(host) = host_ref.get() else {
            return;
        };
        if view_handle.with_value(|v| v.is_some()) {
            return;
        }
        let on_change = Closure::<dyn Fn(String)>::new(move |s: String| {
            session.buffer.set(s);
        });
        let on_save = Closure::<dyn Fn()>::new(move || {
            if !read_only {
                session.save(wb, toast, ui, i18n, false);
            }
        });
        let on_change_fn: js_sys::Function = on_change
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone();
        let on_save_fn: js_sys::Function =
            on_save.as_ref().unchecked_ref::<js_sys::Function>().clone();
        closures.set_value(Some((on_change, on_save)));

        let host_el: web_sys::Element = host.unchecked_into();
        let doc = session.buffer.get_untracked();
        spawn_local(async move {
            match cm::create_editor(&host_el, &doc, language, read_only, &on_change_fn, &on_save_fn)
                .await
            {
                Ok(view) => view_handle.set_value(Some(view)),
                Err(e) => {
                    web_sys::console::error_1(&format!("codemirror init: {e}").into());
                }
            }
        });
    });

    // Push external buffer changes (revert / reload) into the editor. The JS
    // side ignores a no-op set, so edits originating from CodeMirror don't loop
    // back or disturb the caret.
    Effect::new(move |_| {
        let text = session.buffer.get();
        view_handle.with_value(|v| {
            if let Some(view) = v {
                cm::set_doc(view, &text);
            }
        });
    });

    on_cleanup(move || {
        view_handle.update_value(|v| {
            if let Some(view) = v.take() {
                cm::destroy(&view);
            }
        });
        closures.update_value(|c| *c = None);
    });

    let on_contextmenu = move |ev: web_sys::MouseEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        let range = view_handle.with_value(|v| v.as_ref().and_then(cm::selection_lines));
        let Some((lo, hi)) = range else {
            return;
        };
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
        <div class="code-view__cm" node_ref=host_ref on:contextmenu=on_contextmenu />
    }
}
