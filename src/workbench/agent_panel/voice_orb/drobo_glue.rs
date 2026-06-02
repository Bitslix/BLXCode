use gloo_timers::future::TimeoutFuture;
use js_sys::{Function, Object, Reflect};
use wasm_bindgen::prelude::JsValue;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

const DROBO_ORB_SCRIPT_ID: &str = "blxcode-drobo-orb-script";

pub fn drobo_orb_api_ready() -> bool {
    let Some(w) = web_sys::window() else {
        return false;
    };
    Reflect::has(&w, &JsValue::from_str("__blxcodeDroboOrb")).unwrap_or(false)
}

pub async fn ensure_drobo_orb_script() -> Result<(), String> {
    if drobo_orb_api_ready() {
        return Ok(());
    }

    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    if document.get_element_by_id(DROBO_ORB_SCRIPT_ID).is_none() {
        let script = document
            .create_element("script")
            .map_err(|_| "failed to create Drobo orb script")?;
        script.set_id(DROBO_ORB_SCRIPT_ID);
        script
            .set_attribute("type", "module")
            .map_err(|_| "failed to set Drobo orb script type")?;
        script
            .set_attribute("src", "/public/drobo_orb.bundle.mjs")
            .map_err(|_| "failed to set Drobo orb script src")?;
        let parent = document
            .head()
            .or_else(|| document.body().map(|body| body.unchecked_into()))
            .ok_or("no document head/body")?;
        parent
            .append_child(&script)
            .map_err(|_| "failed to append Drobo orb script")?;
    }

    for _ in 0..100 {
        if drobo_orb_api_ready() {
            return Ok(());
        }
        TimeoutFuture::new(50).await;
    }

    Err("Drobo orb bundle did not become ready".into())
}

pub fn drobo_orb_create(container: &HtmlElement) -> Result<f64, String> {
    let root = drobo_orb_root()?;
    let create = drobo_orb_fn(&root, "create")?;
    let res = create
        .call1(&root, container)
        .map_err(|e| format!("drobo_orb_create: {e:?}"))?;
    res.as_f64().ok_or_else(|| "Drobo orb id missing".into())
}

pub fn drobo_orb_set_state(
    orb_id: f64,
    active: bool,
    transcribing: bool,
    compact: bool,
) -> Result<(), String> {
    let root = drobo_orb_root()?;
    let set_state = drobo_orb_fn(&root, "setState")?;
    let state = Object::new();
    Reflect::set(
        &state,
        &JsValue::from_str("active"),
        &JsValue::from_bool(active),
    )
    .map_err(|_| "failed to set active state")?;
    Reflect::set(
        &state,
        &JsValue::from_str("transcribing"),
        &JsValue::from_bool(transcribing),
    )
    .map_err(|_| "failed to set transcribing state")?;
    Reflect::set(
        &state,
        &JsValue::from_str("compact"),
        &JsValue::from_bool(compact),
    )
    .map_err(|_| "failed to set compact state")?;
    set_state
        .call2(&root, &JsValue::from_f64(orb_id), &state)
        .map_err(|e| format!("drobo_orb_set_state: {e:?}"))?;
    Ok(())
}

pub fn drobo_orb_resize(orb_id: f64) {
    call_drobo_orb_1("resize", orb_id);
}

pub fn drobo_orb_dispose(orb_id: f64) {
    call_drobo_orb_1("dispose", orb_id);
}

fn drobo_orb_root() -> Result<JsValue, String> {
    let w = web_sys::window().ok_or("no window")?;
    Reflect::get(&w, &JsValue::from_str("__blxcodeDroboOrb"))
        .map_err(|_| "no __blxcodeDroboOrb".to_string())
}

fn drobo_orb_fn(root: &JsValue, name: &str) -> Result<Function, String> {
    let value = Reflect::get(root, &JsValue::from_str(name)).map_err(|_| format!("no {name}"))?;
    value
        .dyn_into::<Function>()
        .map_err(|_| format!("{name} not function"))
}

fn call_drobo_orb_1(name: &str, orb_id: f64) {
    let Ok(root) = drobo_orb_root() else {
        return;
    };
    let Ok(f) = drobo_orb_fn(&root, name) else {
        return;
    };
    let _ = f.call1(&root, &JsValue::from_f64(orb_id));
}
