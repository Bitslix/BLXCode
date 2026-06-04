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
    Reflect::get(&w, &JsValue::from_str("mermaid"))
        .ok()
        .and_then(|v| {
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

/// Read a CSS custom property (design token) resolved on `:root`, e.g.
/// `css_token("--bg-panel")`. Returns `None` when unset/empty so callers can
/// fall back to Mermaid's own default.
fn css_token(name: &str) -> Option<String> {
    let root = web_sys::window()?.document()?.document_element()?;
    let style = web_sys::window()?.get_computed_style(&root).ok()??;
    let v = style.get_property_value(name).ok()?;
    let v = v.trim().to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

/// Map an app design token onto a Mermaid `themeVariables` entry, skipping it
/// when the token is unset (Mermaid keeps its base default).
fn set_token(vars: &Object, key: &str, token: &str) {
    if let Some(val) = css_token(token) {
        let _ = Reflect::set(vars, &JsValue::from_str(key), &JsValue::from_str(&val));
    }
}

/// Build Mermaid `themeVariables` from the active theme's design tokens so
/// diagram colors stay consistent across all themes (multi-theme support).
/// Uses the `base` theme, which is fully overridable via these variables.
fn theme_variables() -> Object {
    let vars = Object::new();
    // Surfaces.
    set_token(&vars, "background", "--bg-raised");
    set_token(&vars, "primaryColor", "--bg-panel");
    set_token(&vars, "mainBkg", "--bg-panel");
    set_token(&vars, "secondaryColor", "--bg-raised");
    set_token(&vars, "tertiaryColor", "--bg-app");
    set_token(&vars, "clusterBkg", "--bg-app");
    set_token(&vars, "edgeLabelBackground", "--bg-panel");
    // Borders / lines.
    set_token(&vars, "primaryBorderColor", "--border");
    set_token(&vars, "secondaryBorderColor", "--border");
    set_token(&vars, "tertiaryBorderColor", "--border");
    set_token(&vars, "clusterBorder", "--border");
    set_token(&vars, "nodeBorder", "--border");
    set_token(&vars, "lineColor", "--text-muted");
    // Text.
    set_token(&vars, "primaryTextColor", "--text");
    set_token(&vars, "secondaryTextColor", "--text");
    set_token(&vars, "tertiaryTextColor", "--text");
    set_token(&vars, "textColor", "--text");
    set_token(&vars, "titleColor", "--text");
    set_token(&vars, "nodeTextColor", "--text");
    // Accent (active/selected states, notes).
    set_token(&vars, "noteBkgColor", "--accent-soft");
    set_token(&vars, "noteTextColor", "--text");
    set_token(&vars, "activationBkgColor", "--accent-soft");
    if let Some(font) = css_token("--font-sans").or_else(|| css_token("font-family")) {
        let _ = Reflect::set(&vars, &JsValue::from_str("fontFamily"), &JsValue::from_str(&font));
    }
    vars
}

/// (Re-)initialize Mermaid with the current theme's tokens. Called on every
/// render so a live theme switch is reflected in subsequently rendered nodes.
fn initialize_mermaid() -> Result<(), String> {
    let mermaid = mermaid_global().ok_or("mermaid not available")?;
    let init =
        Reflect::get(&mermaid, &JsValue::from_str("initialize")).map_err(|_| "no initialize")?;
    let init: Function = init.dyn_into().map_err(|_| "initialize not callable")?;
    let opts = Object::new();
    Reflect::set(&opts, &JsValue::from_str("startOnLoad"), &JsValue::FALSE)
        .map_err(|_| "set startOnLoad")?;
    Reflect::set(
        &opts,
        &JsValue::from_str("securityLevel"),
        &JsValue::from_str("strict"),
    )
    .map_err(|_| "set securityLevel")?;
    Reflect::set(
        &opts,
        &JsValue::from_str("theme"),
        &JsValue::from_str("base"),
    )
    .map_err(|_| "set theme")?;
    Reflect::set(
        &opts,
        &JsValue::from_str("themeVariables"),
        &theme_variables(),
    )
    .map_err(|_| "set themeVariables")?;
    init.call1(&mermaid, &opts)
        .map_err(|e| format!("mermaid.initialize: {e:?}"))?;
    Ok(())
}

/// Render Mermaid `source` to a standalone `<svg>` string via `mermaid.render`.
///
/// Unlike [`run_mermaid_on`], this does not depend on a caller-owned DOM node
/// staying mounted: Mermaid performs measurement in its own offscreen container
/// and returns the finished SVG markup. `id` must be a unique, valid element id.
pub async fn render_mermaid_to_svg(id: &str, source: &str) -> Result<String, String> {
    ensure_mermaid_loaded().await?;
    // Re-initialize with current theme tokens so the SVG matches the live theme.
    initialize_mermaid()?;
    let mermaid = mermaid_global().ok_or("mermaid not available")?;
    let render = Reflect::get(&mermaid, &JsValue::from_str("render")).map_err(|_| "no render")?;
    let render: Function = render.dyn_into().map_err(|_| "render not callable")?;
    let promise = render
        .call2(
            &mermaid,
            &JsValue::from_str(id),
            &JsValue::from_str(source),
        )
        .map_err(|e| format!("mermaid.render: {e:?}"))?;
    let promise: js_sys::Promise = promise
        .dyn_into()
        .map_err(|_| "mermaid.render did not return a promise")?;
    let result = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("mermaid.render awaited: {e:?}"))?;
    let svg = Reflect::get(&result, &JsValue::from_str("svg"))
        .map_err(|_| "render result has no svg")?;
    svg.as_string().ok_or_else(|| "render svg not a string".into())
}

/// Runs Mermaid on the supplied nodes. Nodes must contain raw graph text as
/// their `textContent` and have the `mermaid` class so the library can find
/// them.
pub async fn run_mermaid_on(nodes: &[HtmlElement]) -> Result<(), String> {
    ensure_mermaid_loaded().await?;
    // Re-initialize with the current theme tokens so a live theme switch is
    // reflected in this render (cheap; just resets config).
    initialize_mermaid()?;
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
