use crate::agent_wire::AgentImageContextItem;
use crate::tauri_bridge::{agent_read_image_file, AgentImageFilePayload};
use crate::workbench::WorkbenchService;
use js_sys::{Array, Date, Function, Reflect};
use leptos::prelude::*;
use uuid::Uuid;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ClipboardEvent, DragEvent, File, FileReader, KeyboardEvent};

const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PENDING_IMAGES: usize = 4;
const MAX_TURN_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropZoneState {
    Inactive,
    Accept,
    Reject,
}

impl DropZoneState {
    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive)
    }

    #[must_use]
    pub fn message(&self) -> &'static str {
        match self {
            Self::Inactive => "",
            Self::Accept => "Drop images to attach",
            Self::Reject => "Only image files can be attached",
        }
    }
}

pub fn install_agent_image_intake(
    wb: WorkbenchService,
    drop_state: RwSignal<DropZoneState>,
    status_line: RwSignal<Option<String>>,
) {
    install_paste_listener(wb, status_line);
    install_escape_listener(drop_state);
    install_tauri_drop_listener(wb, drop_state, status_line);
}

pub fn handle_dom_drag_event(ev: DragEvent, drop_state: RwSignal<DropZoneState>) {
    if has_image_drag(&ev) {
        ev.prevent_default();
        if let Some(dt) = ev.data_transfer() {
            let _ = dt.set_drop_effect("copy");
        }
        drop_state.set(DropZoneState::Accept);
    } else if has_file_drag(&ev) {
        ev.prevent_default();
        if let Some(dt) = ev.data_transfer() {
            let _ = dt.set_drop_effect("none");
        }
        drop_state.set(DropZoneState::Reject);
    }
}

pub fn handle_dom_drop(
    ev: DragEvent,
    wb: WorkbenchService,
    drop_state: RwSignal<DropZoneState>,
    status_line: RwSignal<Option<String>>,
) {
    ev.prevent_default();
    drop_state.set(DropZoneState::Inactive);
    let Some(dt) = ev.data_transfer() else {
        return;
    };
    let Some(files) = dt.files() else {
        return;
    };
    let len = files.length();
    if len == 0 {
        return;
    }
    for idx in 0..len {
        if let Some(file) = files.get(idx) {
            read_dom_file(file, wb, status_line);
        }
    }
}

pub fn clear_drop_state(drop_state: RwSignal<DropZoneState>) {
    drop_state.set(DropZoneState::Inactive);
}

fn install_paste_listener(wb: WorkbenchService, status_line: RwSignal<Option<String>>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let cb: Closure<dyn FnMut(ClipboardEvent)> = Closure::new(move |ev: ClipboardEvent| {
        let Some(data) = ev.clipboard_data() else {
            return;
        };
        let Some(files) = data.files() else {
            return;
        };
        let len = files.length();
        if len == 0 {
            return;
        }
        let mut handled = false;
        for idx in 0..len {
            if let Some(file) = files.get(idx) {
                if is_supported_image_mime(&file.type_()) {
                    handled = true;
                    read_dom_file(file, wb, status_line);
                }
            }
        }
        if handled {
            ev.prevent_default();
        }
    });
    let _ = window.add_event_listener_with_callback("paste", cb.as_ref().unchecked_ref());
    cb.forget();
}

fn install_escape_listener(drop_state: RwSignal<DropZoneState>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let cb: Closure<dyn FnMut(KeyboardEvent)> = Closure::new(move |ev: KeyboardEvent| {
        if ev.key() == "Escape" {
            drop_state.set(DropZoneState::Inactive);
        }
    });
    let _ = window.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
    cb.forget();
}

fn install_tauri_drop_listener(
    wb: WorkbenchService,
    drop_state: RwSignal<DropZoneState>,
    status_line: RwSignal<Option<String>>,
) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(tauri) = Reflect::get(&window, &JsValue::from_str("__TAURI__")) else {
        return;
    };
    if tauri.is_undefined() || tauri.is_null() {
        return;
    }
    let Ok(webview) = Reflect::get(&tauri, &JsValue::from_str("webview")) else {
        return;
    };
    let Ok(get_current) = Reflect::get(&webview, &JsValue::from_str("getCurrentWebview"))
        .and_then(|v| v.dyn_into::<Function>().map_err(|e| e))
    else {
        return;
    };
    let Ok(current) = get_current.call0(&webview) else {
        return;
    };
    let Ok(on_drop) = Reflect::get(&current, &JsValue::from_str("onDragDropEvent"))
        .and_then(|v| v.dyn_into::<Function>().map_err(|e| e))
    else {
        return;
    };

    let cb: Closure<dyn FnMut(JsValue)> = Closure::new(move |event: JsValue| {
        let payload = Reflect::get(&event, &JsValue::from_str("payload")).unwrap_or(JsValue::NULL);
        let kind = Reflect::get(&payload, &JsValue::from_str("type"))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        match kind.as_str() {
            "enter" | "over" => drop_state.set(DropZoneState::Accept),
            "leave" => drop_state.set(DropZoneState::Inactive),
            "drop" => {
                drop_state.set(DropZoneState::Inactive);
                for path in payload_paths(&payload) {
                    read_tauri_file(path, wb, status_line);
                }
            }
            _ => {}
        }
    });

    if on_drop
        .call1(&current, cb.as_ref().unchecked_ref())
        .is_err()
    {
        return;
    }
    cb.forget();
}

fn payload_paths(payload: &JsValue) -> Vec<String> {
    let Ok(paths) = Reflect::get(payload, &JsValue::from_str("paths")) else {
        return Vec::new();
    };
    if !Array::is_array(&paths) {
        return Vec::new();
    }
    Array::from(&paths)
        .iter()
        .filter_map(|v| v.as_string())
        .collect()
}

fn read_tauri_file(path: String, wb: WorkbenchService, status_line: RwSignal<Option<String>>) {
    leptos::task::spawn_local(async move {
        match agent_read_image_file(path).await {
            Ok(payload) => add_image_payload(wb, status_line, payload),
            Err(e) => status_line.set(Some(e)),
        }
    });
}

fn read_dom_file(file: File, wb: WorkbenchService, status_line: RwSignal<Option<String>>) {
    let label = if file.name().trim().is_empty() {
        "Pasted image".to_string()
    } else {
        file.name()
    };
    let mime = file.type_();
    let size_bytes = file.size() as u64;
    if !is_supported_image_mime(&mime) {
        status_line.set(Some(
            "Only PNG, JPEG, WebP, and GIF images can be attached.".into(),
        ));
        return;
    }
    if size_bytes > MAX_IMAGE_BYTES {
        status_line.set(Some(format!(
            "Image exceeds {} MiB limit.",
            MAX_IMAGE_BYTES / 1024 / 1024
        )));
        return;
    }

    let Ok(reader) = FileReader::new() else {
        status_line.set(Some("Could not create image reader.".into()));
        return;
    };
    let reader_for_cb = reader.clone();
    let cb: Closure<dyn FnMut(web_sys::ProgressEvent)> =
        Closure::once(move |_ev: web_sys::ProgressEvent| {
            let result = reader_for_cb.result().ok().and_then(|v| v.as_string());
            let Some(data_url) = result else {
                status_line.set(Some("Could not read image data.".into()));
                return;
            };
            let Some((mime_from_url, bytes_b64)) = parse_data_url(&data_url) else {
                status_line.set(Some("Could not parse image data.".into()));
                return;
            };
            add_image_payload(
                wb,
                status_line,
                AgentImageFilePayload {
                    label,
                    mime: if mime.is_empty() { mime_from_url } else { mime },
                    bytes_b64,
                    size_bytes,
                },
            );
        });
    reader.set_onloadend(Some(cb.as_ref().unchecked_ref()));
    cb.forget();
    if let Err(err) = reader.read_as_data_url(&file) {
        status_line.set(Some(format!("Could not read image: {err:?}")));
    }
}

fn add_image_payload(
    wb: WorkbenchService,
    status_line: RwSignal<Option<String>>,
    payload: AgentImageFilePayload,
) {
    if !is_supported_image_mime(&payload.mime) {
        status_line.set(Some(
            "Only PNG, JPEG, WebP, and GIF images can be attached.".into(),
        ));
        return;
    }
    if payload.size_bytes > MAX_IMAGE_BYTES {
        status_line.set(Some(format!(
            "Image exceeds {} MiB limit.",
            MAX_IMAGE_BYTES / 1024 / 1024
        )));
        return;
    }
    let Some(ws_id) = wb.active_id().get_untracked() else {
        status_line.set(Some("Select a workspace tab first.".into()));
        return;
    };
    let pending = wb.pending_agent_images_for_workspace_untracked(ws_id);
    if pending.len() >= MAX_PENDING_IMAGES {
        status_line.set(Some(format!(
            "Attach at most {MAX_PENDING_IMAGES} pending images."
        )));
        return;
    }
    let pending_bytes = pending
        .iter()
        .map(|item| item.size_bytes)
        .sum::<u64>()
        .saturating_add(payload.size_bytes);
    if pending_bytes > MAX_TURN_IMAGE_BYTES {
        status_line.set(Some(format!(
            "Pending images exceed {} MiB turn limit.",
            MAX_TURN_IMAGE_BYTES / 1024 / 1024
        )));
        return;
    }
    let item = AgentImageContextItem {
        id: format!("image:{}", Uuid::new_v4().simple()),
        label: payload.label,
        mime: payload.mime,
        bytes_b64: payload.bytes_b64,
        size_bytes: payload.size_bytes,
        added_at: Date::now() as i64,
    };
    wb.upsert_workspace_agent_image(ws_id, item);
    status_line.set(None);
}

fn parse_data_url(data_url: &str) -> Option<(String, String)> {
    let rest = data_url.strip_prefix("data:")?;
    let (meta, body) = rest.split_once(',')?;
    if !meta.ends_with(";base64") {
        return None;
    }
    let mime = meta.trim_end_matches(";base64").to_string();
    Some((mime, body.to_string()))
}

fn has_image_drag(ev: &DragEvent) -> bool {
    let Some(dt) = ev.data_transfer() else {
        return false;
    };
    let items = dt.items();
    for idx in 0..items.length() {
        if let Some(item) = items.get(idx) {
            if item.kind() == "file" && is_supported_image_mime(&item.type_()) {
                return true;
            }
        }
    }
    false
}

fn has_file_drag(ev: &DragEvent) -> bool {
    ev.data_transfer()
        .map(|dt| {
            let types = dt.types();
            (0..types.length())
                .filter_map(|idx| types.get(idx).as_string())
                .any(|ty| ty == "Files")
        })
        .unwrap_or(false)
}

fn is_supported_image_mime(mime: &str) -> bool {
    matches!(
        mime,
        "image/png" | "image/jpeg" | "image/webp" | "image/gif"
    )
}
