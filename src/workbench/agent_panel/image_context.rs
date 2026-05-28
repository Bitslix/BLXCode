use crate::agent_wire::AgentImageContextItem;
use crate::tauri_bridge::{agent_read_image_file, pty_peek_output, AgentImageFilePayload};
use crate::workbench::agent_context_handoff::{
    list_workspace_terminal_targets, terminal_session_context_item_with_content,
};
use crate::workbench::terminal_slot_dnd::{
    is_terminal_drag, read_drag_payload, TerminalSlotDragPayload, TerminalSlotDragService,
};
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
const TERMINAL_CONTEXT_TAIL_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropZoneState {
    Inactive,
    AcceptImage,
    AcceptTerminal,
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
            Self::AcceptImage => "Drop images to attach",
            Self::AcceptTerminal => "Drop terminal to attach session context",
            Self::Reject => "Only image files or terminal sessions can be attached",
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

pub fn handle_dom_drag_event(
    ev: DragEvent,
    drop_state: RwSignal<DropZoneState>,
    slot_dnd: TerminalSlotDragService,
) {
    if has_terminal_drag(&ev, slot_dnd) {
        ev.prevent_default();
        ev.stop_propagation();
        if let Some(dt) = ev.data_transfer() {
            let _ = dt.set_drop_effect("copy");
        }
        slot_dnd.set_overlay_pos_from_event(&ev);
        drop_state.set(DropZoneState::AcceptTerminal);
    } else if has_image_drag(&ev) {
        ev.prevent_default();
        if let Some(dt) = ev.data_transfer() {
            let _ = dt.set_drop_effect("copy");
        }
        drop_state.set(DropZoneState::AcceptImage);
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
    slot_dnd: TerminalSlotDragService,
) {
    ev.prevent_default();
    drop_state.set(DropZoneState::Inactive);
    let Some(dt) = ev.data_transfer() else {
        return;
    };

    if let Some(payload) = read_drag_payload(&dt).or_else(|| terminal_payload_from_active(slot_dnd))
    {
        ev.stop_propagation();
        attach_terminal_context(payload, wb, status_line, slot_dnd);
        return;
    }

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
            "enter" | "over" => drop_state.set(DropZoneState::AcceptImage),
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

fn attach_terminal_context(
    payload: TerminalSlotDragPayload,
    wb: WorkbenchService,
    status_line: RwSignal<Option<String>>,
    slot_dnd: TerminalSlotDragService,
) {
    let Some(active_ws_id) = wb.active_id().get_untracked() else {
        slot_dnd.clear();
        status_line.set(Some("Select a workspace tab first.".into()));
        return;
    };
    if payload.workspace_id != active_ws_id {
        slot_dnd.clear();
        status_line.set(Some(
            "Terminal context can only be attached to its own workspace.".into(),
        ));
        return;
    }

    let target = list_workspace_terminal_targets(&wb, active_ws_id)
        .into_iter()
        .find(|target| target.slot_id == payload.slot_id);
    let Some(target) = target else {
        slot_dnd.clear();
        status_line.set(Some(format!(
            "Terminal slot {} has no running session.",
            payload.slot_id
        )));
        return;
    };

    status_line.set(None);
    leptos::task::spawn_local(async move {
        let tail = match pty_peek_output(target.session_id, TERMINAL_CONTEXT_TAIL_BYTES).await {
            Ok(text) => terminal_tail_content(&text),
            Err(err) => Some(format!("Could not read terminal output tail: {err}")),
        };
        let agent = if target.agent_slug.trim().is_empty() {
            "shell".to_string()
        } else {
            target.agent_slug.clone()
        };
        let source = format!(
            "Live terminal session: slot {}, pane {}, session {}, agent={}. Use `harness.read_terminal_output` with slotId {} to inspect fresh output if needed.",
            target.slot_id,
            target.pane_id,
            target.session_id,
            agent,
            target.slot_id
        );
        let item = terminal_session_context_item_with_content(
            target.slot_id,
            &target.label,
            &source,
            tail,
        );
        wb.upsert_workspace_agent_context(active_ws_id, item);
        slot_dnd.clear();
    });
}

fn terminal_payload_from_active(
    slot_dnd: TerminalSlotDragService,
) -> Option<TerminalSlotDragPayload> {
    slot_dnd.active.get().map(|meta| TerminalSlotDragPayload {
        workspace_id: meta.workspace_id,
        slot_id: meta.slot_id,
    })
}

fn terminal_tail_content(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
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

fn has_terminal_drag(ev: &DragEvent, slot_dnd: TerminalSlotDragService) -> bool {
    ev.data_transfer()
        .as_ref()
        .map(is_terminal_drag)
        .unwrap_or(false)
        || slot_dnd.active.get_untracked().is_some()
        || slot_dnd.session_active()
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
