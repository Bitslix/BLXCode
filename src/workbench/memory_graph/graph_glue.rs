use crate::tauri_bridge::GraphData;
use gloo_timers::future::TimeoutFuture;
use js_sys::{Function, Reflect};
use wasm_bindgen::prelude::JsValue;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

const GRAPH3D_SCRIPT_ID: &str = "blxcode-graph3d-script";

pub fn graph3d_api_ready() -> bool {
    let Some(w) = web_sys::window() else {
        return false;
    };
    Reflect::has(&w, &JsValue::from_str("__blxcodeGraph3d")).unwrap_or(false)
}

pub async fn ensure_graph3d_script() -> Result<(), String> {
    if graph3d_api_ready() {
        return Ok(());
    }

    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    if document.get_element_by_id(GRAPH3D_SCRIPT_ID).is_none() {
        let script = document
            .create_element("script")
            .map_err(|_| "failed to create graph script")?;
        script.set_id(GRAPH3D_SCRIPT_ID);
        script
            .set_attribute("type", "module")
            .map_err(|_| "failed to set graph script type")?;
        script
            .set_attribute("src", "/public/graph3d.bundle.mjs")
            .map_err(|_| "failed to set graph script src")?;
        let parent = document
            .head()
            .or_else(|| document.body().map(|body| body.unchecked_into()))
            .ok_or("no document head/body")?;
        parent
            .append_child(&script)
            .map_err(|_| "failed to append graph script")?;
    }

    for _ in 0..100 {
        if graph3d_api_ready() {
            return Ok(());
        }
        TimeoutFuture::new(50).await;
    }

    Err("Graph 3D bundle did not become ready".into())
}

pub fn graph3d_create(container: &HtmlElement) -> Result<f64, String> {
    let root = graph3d_root()?;
    let create = graph3d_fn(&root, "create")?;
    let res = create
        .call1(&root, container)
        .map_err(|e| format!("graph3d_create: {e:?}"))?;
    res.as_f64().ok_or_else(|| "graph id missing".into())
}

pub fn graph3d_dispose(graph_id: f64) {
    let Ok(root) = graph3d_root() else {
        return;
    };
    let Ok(dispose) = graph3d_fn(&root, "dispose") else {
        return;
    };
    let _ = dispose.call1(&root, &JsValue::from_f64(graph_id));
}

pub fn graph3d_set_data(graph_id: f64, graph: &GraphData) -> Result<(), String> {
    let root = graph3d_root()?;
    let set_data = graph3d_fn(&root, "setData")?;
    let value = serde_wasm_bindgen::to_value(graph).map_err(|e| e.to_string())?;
    set_data
        .call2(&root, &JsValue::from_f64(graph_id), &value)
        .map_err(|e| format!("graph3d_set_data: {e:?}"))?;
    Ok(())
}

pub fn graph3d_zoom(graph_id: f64, factor: f64) {
    call_graph3d_2("zoom", graph_id, JsValue::from_f64(factor));
}

pub fn graph3d_reset_view(graph_id: f64) {
    call_graph3d_1("resetView", graph_id);
}

pub fn graph3d_fly_to_node(graph_id: f64, node_id: &str, ms: f64) {
    let Ok(root) = graph3d_root() else {
        return;
    };
    let Ok(fly) = graph3d_fn(&root, "flyToNode") else {
        return;
    };
    let _ = fly.call3(
        &root,
        &JsValue::from_f64(graph_id),
        &JsValue::from_str(node_id),
        &JsValue::from_f64(ms),
    );
}

pub fn graph3d_resize(graph_id: f64) {
    call_graph3d_1("resize", graph_id);
}

fn graph3d_root() -> Result<JsValue, String> {
    let w = web_sys::window().ok_or("no window")?;
    Reflect::get(&w, &JsValue::from_str("__blxcodeGraph3d"))
        .map_err(|_| "no __blxcodeGraph3d".to_string())
}

fn graph3d_fn(root: &JsValue, name: &str) -> Result<Function, String> {
    let value = Reflect::get(root, &JsValue::from_str(name)).map_err(|_| format!("no {name}"))?;
    value
        .dyn_into::<Function>()
        .map_err(|_| format!("{name} not function"))
}

fn call_graph3d_1(name: &str, graph_id: f64) {
    let Ok(root) = graph3d_root() else {
        return;
    };
    let Ok(f) = graph3d_fn(&root, name) else {
        return;
    };
    let _ = f.call1(&root, &JsValue::from_f64(graph_id));
}

fn call_graph3d_2(name: &str, graph_id: f64, arg: JsValue) {
    let Ok(root) = graph3d_root() else {
        return;
    };
    let Ok(f) = graph3d_fn(&root, name) else {
        return;
    };
    let _ = f.call2(&root, &JsValue::from_f64(graph_id), &arg);
}
