//! Lazy-loading bridge for the vendored `highlight.js` UMD bundle
//! (`public/vendor/highlight/highlight.min.js`). The bundle is fetched on
//! first request and reused via `globalThis.hljs`.

use gloo_timers::future::TimeoutFuture;
use js_sys::{Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const HLJS_SCRIPT_ID: &str = "blxcode-hljs-bundle";
const HLJS_SRC: &str = "/public/vendor/highlight/highlight.min.js";

fn hljs_global() -> Option<JsValue> {
    let w = web_sys::window()?;
    Reflect::get(&w, &JsValue::from_str("hljs"))
        .ok()
        .and_then(|v| {
            if v.is_undefined() || v.is_null() {
                None
            } else {
                Some(v)
            }
        })
}

/// Returns `Ok(())` once `globalThis.hljs` exists. Inserts a `<script>` tag
/// the first time it's called and then polls every 25ms for up to 5 seconds.
pub async fn ensure_hljs_loaded() -> Result<(), String> {
    if hljs_global().is_some() {
        return Ok(());
    }
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    if document.get_element_by_id(HLJS_SCRIPT_ID).is_none() {
        let script = document
            .create_element("script")
            .map_err(|_| "failed to create hljs script")?;
        script.set_id(HLJS_SCRIPT_ID);
        script
            .set_attribute("src", HLJS_SRC)
            .map_err(|_| "failed to set hljs src")?;
        let parent = document
            .head()
            .or_else(|| document.body().map(|b| b.unchecked_into()))
            .ok_or("no document head/body")?;
        parent
            .append_child(&script)
            .map_err(|_| "failed to append hljs script")?;
    }
    for _ in 0..200 {
        if hljs_global().is_some() {
            return Ok(());
        }
        TimeoutFuture::new(25).await;
    }
    Err("highlight.js bundle did not become ready".into())
}

/// Highlights `code` as `language` and returns the resulting HTML, ready to be
/// embedded inside a `<code>` element. Falls back to `Err` when highlight.js
/// is not available or the language is not registered.
pub async fn highlight(code: &str, language: &str) -> Result<String, String> {
    ensure_hljs_loaded().await?;
    let hljs = hljs_global().ok_or("hljs not available")?;
    let highlight_fn =
        Reflect::get(&hljs, &JsValue::from_str("highlight")).map_err(|_| "no hljs.highlight")?;
    let highlight_fn: Function = highlight_fn
        .dyn_into()
        .map_err(|_| "hljs.highlight not callable")?;

    let opts = Object::new();
    Reflect::set(
        &opts,
        &JsValue::from_str("language"),
        &JsValue::from_str(language),
    )
    .map_err(|_| "set language")?;
    Reflect::set(&opts, &JsValue::from_str("ignoreIllegals"), &JsValue::TRUE)
        .map_err(|_| "set ignoreIllegals")?;

    let result = highlight_fn
        .call2(&hljs, &JsValue::from_str(code), &opts)
        .map_err(|e| format!("hljs.highlight: {e:?}"))?;
    let value =
        Reflect::get(&result, &JsValue::from_str("value")).map_err(|_| "no .value on result")?;
    value
        .as_string()
        .ok_or_else(|| "hljs.highlight returned non-string".into())
}
