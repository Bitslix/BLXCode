//! Lazy-loading bridge for the vendored CodeMirror 6 bundle
//! (`public/vendor/codemirror/codemirror.min.js`, global `BlxCM`). Mirrors the
//! highlight.js loader: the bundle is injected on first use and reused via
//! `globalThis.BlxCM`. Only the small wrapper API defined in
//! `scripts/codemirror-bundle/cm-entry.js` is called from here.

use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const SCRIPT_ID: &str = "blxcode-codemirror-bundle";
const SRC: &str = "/public/vendor/codemirror/codemirror.min.js";

fn cm_global() -> Option<JsValue> {
    let w = web_sys::window()?;
    Reflect::get(&w, &JsValue::from_str("BlxCM"))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
}

/// Resolves once `globalThis.BlxCM` exists. Injects the `<script>` the first
/// time and then polls every 25ms for up to ~7.5s.
pub async fn ensure_cm_loaded() -> Result<(), String> {
    if cm_global().is_some() {
        return Ok(());
    }
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    if document.get_element_by_id(SCRIPT_ID).is_none() {
        let script = document
            .create_element("script")
            .map_err(|_| "failed to create codemirror script")?;
        script.set_id(SCRIPT_ID);
        script
            .set_attribute("src", SRC)
            .map_err(|_| "failed to set codemirror src")?;
        let parent = document
            .head()
            .or_else(|| document.body().map(|b| b.unchecked_into()))
            .ok_or("no document head/body")?;
        parent
            .append_child(&script)
            .map_err(|_| "failed to append codemirror script")?;
    }
    for _ in 0..300 {
        if cm_global().is_some() {
            return Ok(());
        }
        TimeoutFuture::new(25).await;
    }
    Err("codemirror bundle did not become ready".into())
}

fn call_method(obj: &JsValue, name: &str, args: &Array) -> Result<JsValue, String> {
    let f = Reflect::get(obj, &JsValue::from_str(name)).map_err(|_| format!("no BlxCM.{name}"))?;
    let f: Function = f
        .dyn_into()
        .map_err(|_| format!("BlxCM.{name} not callable"))?;
    f.apply(obj, args)
        .map_err(|e| format!("BlxCM.{name}: {e:?}"))
}

/// Mount an editor inside `parent`. `on_change(text)` fires on every user edit;
/// `on_save()` fires on `Mod-s`. When `read_only` is set the document is shown
/// for preview only (no edits) while keeping the same gutter, folding, syntax
/// highlighting and selection as edit mode. Returns the opaque `EditorView`
/// handle.
pub async fn create_editor(
    parent: &web_sys::Element,
    doc: &str,
    language: Option<&str>,
    read_only: bool,
    on_change: &Function,
    on_save: &Function,
    on_cursor: &Function,
) -> Result<JsValue, String> {
    ensure_cm_loaded().await?;
    let cm = cm_global().ok_or("BlxCM not available")?;
    let opts = Object::new();
    let _ = Reflect::set(&opts, &"doc".into(), &JsValue::from_str(doc));
    let lang = language.map(JsValue::from_str).unwrap_or(JsValue::NULL);
    let _ = Reflect::set(&opts, &"language".into(), &lang);
    let _ = Reflect::set(&opts, &"readOnly".into(), &JsValue::from_bool(read_only));
    let _ = Reflect::set(&opts, &"onChange".into(), on_change);
    let _ = Reflect::set(&opts, &"onSave".into(), on_save);
    let _ = Reflect::set(&opts, &"onCursor".into(), on_cursor);
    let parent_val: JsValue = parent.clone().into();
    let args = Array::of2(&parent_val, &opts);
    call_method(&cm, "create", &args)
}

/// Replace the editor document (revert / reload). No-op if the text is already
/// current, so it never disturbs the caret or re-fires `on_change`.
pub fn set_doc(view: &JsValue, text: &str) {
    if let Some(cm) = cm_global() {
        let args = Array::of2(view, &JsValue::from_str(text));
        let _ = call_method(&cm, "setDoc", &args);
    }
}

pub fn destroy(view: &JsValue) {
    if let Some(cm) = cm_global() {
        let _ = call_method(&cm, "destroy", &Array::of1(view));
    }
}

/// 1-based `(from, to)` line range of the primary selection (collapsed caret ⇒
/// single line). Drives the right-click handoff menu in edit mode.
pub fn selection_lines(view: &JsValue) -> Option<(u32, u32)> {
    let cm = cm_global()?;
    let res = call_method(&cm, "selectionLines", &Array::of1(view)).ok()?;
    let arr: Array = res.dyn_into().ok()?;
    let from = arr.get(0).as_f64()? as u32;
    let to = arr.get(1).as_f64()? as u32;
    Some((from, to))
}
