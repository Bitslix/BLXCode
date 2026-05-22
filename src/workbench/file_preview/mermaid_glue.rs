//! Lazy-loading bridge for the vendored Mermaid bundle (`public/vendor/mermaid/`).
//! Mermaid is fetched on first preview mount and reused across renderers via
//! `globalThis.mermaid`.

use gloo_timers::future::TimeoutFuture;
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

const MERMAID_SCRIPT_ID: &str = "blxcode-mermaid-bundle";
const MERMAID_SRC: &str = "/public/vendor/mermaid/mermaid.min.js";

fn mermaid_global() -> Option<JsValue> {
    let w = web_sys::window()?;
    Reflect::get(&w, &JsValue::from_str("mermaid")).ok().and_then(|v| {
        if v.is_undefined() || v.is_null() {
            None
        } else {
            Some(v)
        }
    })
}

/// Returns `Ok(())` once `globalThis.mermaid` exists. Inserts a `<script>`
/// tag the first time and then polls every 50ms for up to 5 seconds.
pub async fn ensure_mermaid_loaded() -> Result<(), String> {
    if mermaid_global().is_some() {
        return Ok(());
    }
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    if document.get_element_by_id(MERMAID_SCRIPT_ID).is_none() {
        let script = document
            .create_element("script")
            .map_err(|_| "failed to create mermaid script")?;
        script.set_id(MERMAID_SCRIPT_ID);
        script
            .set_attribute("src", MERMAID_SRC)
            .map_err(|_| "failed to set mermaid src")?;
        let parent = document
            .head()
            .or_else(|| document.body().map(|b| b.unchecked_into()))
            .ok_or("no document head/body")?;
        parent
            .append_child(&script)
            .map_err(|_| "failed to append mermaid script")?;
    }
    for _ in 0..100 {
        if mermaid_global().is_some() {
            initialize_mermaid()?;
            return Ok(());
        }
        TimeoutFuture::new(50).await;
    }
    Err("Mermaid bundle did not become ready".into())
}

fn initialize_mermaid() -> Result<(), String> {
    let mermaid = mermaid_global().ok_or("mermaid not available")?;
    let init = Reflect::get(&mermaid, &JsValue::from_str("initialize"))
        .map_err(|_| "no initialize")?;
    let init: Function = init.dyn_into().map_err(|_| "initialize not callable")?;
    let opts = Object::new();
    Reflect::set(&opts, &JsValue::from_str("startOnLoad"), &JsValue::FALSE)
        .map_err(|_| "set startOnLoad")?;
    Reflect::set(&opts, &JsValue::from_str("securityLevel"), &JsValue::from_str("strict"))
        .map_err(|_| "set securityLevel")?;
    Reflect::set(&opts, &JsValue::from_str("theme"), &JsValue::from_str("dark"))
        .map_err(|_| "set theme")?;
    init.call1(&mermaid, &opts).map_err(|e| format!("mermaid.initialize: {e:?}"))?;
    Ok(())
}

/// Runs Mermaid on the supplied nodes. Nodes must contain raw graph text as
/// their `textContent` and have the `mermaid` class so the library can find
/// them.
pub async fn run_mermaid_on(nodes: &[HtmlElement]) -> Result<(), String> {
    ensure_mermaid_loaded().await?;
    let mermaid = mermaid_global().ok_or("mermaid not available")?;
    let run = Reflect::get(&mermaid, &JsValue::from_str("run")).map_err(|_| "no run")?;
    let run: Function = run.dyn_into().map_err(|_| "run not callable")?;
    let arr = Array::new();
    for n in nodes {
        arr.push(n);
    }
    let opts = Object::new();
    Reflect::set(&opts, &JsValue::from_str("nodes"), &arr).map_err(|_| "set nodes")?;
    let promise = run
        .call1(&mermaid, &opts)
        .map_err(|e| format!("mermaid.run: {e:?}"))?;
    let promise: js_sys::Promise = promise
        .dyn_into()
        .map_err(|_| "mermaid.run did not return a promise")?;
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("mermaid.run awaited: {e:?}"))?;
    Ok(())
}
