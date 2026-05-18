//! JS bridge for xterm (`window.__blxcodeTerminal` from `public/terminal_bootstrap.mjs`).
use js_sys::{Function, Promise, Reflect};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::JsValue;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlElement;

#[derive(Clone, Copy, Debug)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

pub fn terminal_api_ready() -> bool {
    let Some(w) = web_sys::window() else {
        return false;
    };
    Reflect::has(&w, &wasm_bindgen::JsValue::from_str("__blxcodeTerminal")).unwrap_or(false)
}

pub async fn terminal_wait_api_ready() -> bool {
    if terminal_api_ready() {
        return true;
    }
    let promise = Promise::new(&mut |resolve: Function, _reject: Function| {
        if terminal_api_ready() {
            let _ = resolve.call0(&JsValue::UNDEFINED);
            return;
        }
        let Some(window) = web_sys::window() else {
            return;
        };
        let callback = Closure::once_into_js(move |_event: web_sys::Event| {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        let _ = window.add_event_listener_with_callback(
            "blxcode-terminal-api-ready",
            callback.unchecked_ref(),
        );
    });
    JsFuture::from(promise).await.is_ok() && terminal_api_ready()
}

pub fn terminal_create(container: &HtmlElement) -> Result<f64, String> {
    let w = web_sys::window().ok_or("no window")?;
    let root = Reflect::get(&w, &wasm_bindgen::JsValue::from_str("__blxcodeTerminal"))
        .map_err(|_| "no __blxcodeTerminal")?;
    let create =
        Reflect::get(&root, &wasm_bindgen::JsValue::from_str("create")).map_err(|_| "no create")?;
    let f: &Function = create.dyn_ref().ok_or("create not function")?;
    let res = f
        .call1(&root, container)
        .map_err(|e| format!("terminal_create: {e:?}"))?;
    res.as_f64().ok_or_else(|| "terminal id missing".into())
}

pub fn terminal_dispose(term_id: f64) {
    let Ok(w) = web_sys::window().ok_or(()) else {
        return;
    };
    let Ok(root) = Reflect::get(&w, &wasm_bindgen::JsValue::from_str("__blxcodeTerminal")) else {
        return;
    };
    let Ok(dispose) = Reflect::get(&root, &wasm_bindgen::JsValue::from_str("dispose")) else {
        return;
    };
    if let Some(f) = dispose.dyn_ref::<Function>() {
        let _ = f.call1(&root, &wasm_bindgen::JsValue::from_f64(term_id));
    }
}

pub fn terminal_fit(term_id: f64) -> Option<TerminalSize> {
    let Some(w) = web_sys::window() else {
        return None;
    };
    let Ok(root) = Reflect::get(&w, &wasm_bindgen::JsValue::from_str("__blxcodeTerminal")) else {
        return None;
    };
    let Ok(fit_fn) = Reflect::get(&root, &wasm_bindgen::JsValue::from_str("fit")) else {
        return None;
    };
    if let Some(f) = fit_fn.dyn_ref::<Function>() {
        let Ok(v) = f.call1(&root, &wasm_bindgen::JsValue::from_f64(term_id)) else {
            return None;
        };
        return terminal_size_from_js(&v);
    }
    None
}

pub fn terminal_request_fit(term_id: f64) -> Option<TerminalSize> {
    let Some(w) = web_sys::window() else {
        return None;
    };
    let Ok(root) = Reflect::get(&w, &wasm_bindgen::JsValue::from_str("__blxcodeTerminal")) else {
        return None;
    };
    let Ok(fit_fn) = Reflect::get(&root, &wasm_bindgen::JsValue::from_str("requestFit")) else {
        return None;
    };
    if let Some(f) = fit_fn.dyn_ref::<Function>() {
        let Ok(v) = f.call1(&root, &wasm_bindgen::JsValue::from_f64(term_id)) else {
            return None;
        };
        return terminal_size_from_js(&v);
    }
    None
}

pub fn terminal_size_from_js(value: &wasm_bindgen::JsValue) -> Option<TerminalSize> {
    let rows = Reflect::get(value, &wasm_bindgen::JsValue::from_str("rows"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or_default() as u16;
    let cols = Reflect::get(value, &wasm_bindgen::JsValue::from_str("cols"))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or_default() as u16;
    (rows > 0 && cols > 0).then_some(TerminalSize { rows, cols })
}

pub fn terminal_write_b64(term_id: f64, b64: &str) {
    let Some(w) = web_sys::window() else {
        return;
    };
    let Ok(root) = Reflect::get(&w, &wasm_bindgen::JsValue::from_str("__blxcodeTerminal")) else {
        return;
    };
    let Ok(wb) = Reflect::get(&root, &wasm_bindgen::JsValue::from_str("writeBytesB64")) else {
        return;
    };
    if let Some(f) = wb.dyn_ref::<Function>() {
        let _ = f.call2(
            &root,
            &wasm_bindgen::JsValue::from_f64(term_id),
            &wasm_bindgen::JsValue::from_str(b64),
        );
    }
}

pub fn terminal_show_fallback(term_id: f64, text: &str) {
    let Some(w) = web_sys::window() else {
        return;
    };
    let Ok(root) = Reflect::get(&w, &wasm_bindgen::JsValue::from_str("__blxcodeTerminal")) else {
        return;
    };
    let Ok(sf) = Reflect::get(&root, &wasm_bindgen::JsValue::from_str("showFallback")) else {
        return;
    };
    if let Some(f) = sf.dyn_ref::<Function>() {
        let _ = f.call2(
            &root,
            &wasm_bindgen::JsValue::from_f64(term_id),
            &wasm_bindgen::JsValue::from_str(text),
        );
    }
}

pub fn terminal_set_stdin_enabled(term_id: f64, enabled: bool) {
    let Some(w) = web_sys::window() else {
        return;
    };
    let Ok(root) = Reflect::get(&w, &wasm_bindgen::JsValue::from_str("__blxcodeTerminal")) else {
        return;
    };
    let Ok(se) = Reflect::get(&root, &wasm_bindgen::JsValue::from_str("setStdinEnabled")) else {
        return;
    };
    if let Some(f) = se.dyn_ref::<Function>() {
        let _ = f.call2(
            &root,
            &wasm_bindgen::JsValue::from_f64(term_id),
            &wasm_bindgen::JsValue::from_bool(enabled),
        );
    }
}
