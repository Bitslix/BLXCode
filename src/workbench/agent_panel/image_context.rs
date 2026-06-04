use crate::agent_wire::AgentImageContextItem;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_read_image_file, git_commit_details, git_file_diff, pty_peek_output,
    AgentImageFilePayload,
};
use crate::workbench::agent_context_handoff::{
    attach_plan_into_agent, dir_ref_context_item, file_ref_context_item, git_commit_context_item,
    git_diff_context_item, list_workspace_terminal_targets, plan_task_context_item,
    terminal_session_context_item_with_content,
};
use crate::workbench::context_drag::{
    read_drag_payload as read_context_payload, ContextDragKind, ContextDragPayload,
    ContextDragService,
};
use crate::workbench::kanban_dnd::{
    is_kanban_drag, read_drag_payload as read_kanban_payload, KanbanDragKind, KanbanDragPayload,
    KanbanDragService,
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
    AcceptFile,
    AcceptFolder,
    AcceptDiff,
    AcceptCommit,
    AcceptPlan,
    AcceptTask,
    Reject,
}

impl DropZoneState {
    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive)
    }

    #[must_use]
    pub fn message(&self, i18n: I18nService) -> String {
        match self {
            Self::Inactive => String::new(),
            Self::AcceptImage => i18n.tr(I18nKey::AgentImageDropImagesToAttach)().to_string(),
            Self::AcceptTerminal => {
                i18n.tr(I18nKey::AgentImageDropTerminalToAttachSessionContext)().to_string()
            }
            Self::AcceptFile => i18n.tr(I18nKey::AgentImageDropFileToAttachAsContext)().to_string(),
            Self::AcceptFolder => {
                i18n.tr(I18nKey::AgentImageDropFolderToAttachAsContext)().to_string()
            }
            Self::AcceptDiff => i18n.tr(I18nKey::AgentImageDropDiffToAttachAsContext)().to_string(),
            Self::AcceptCommit => {
                i18n.tr(I18nKey::AgentImageDropCommitToAttachAsContext)().to_string()
            }
            Self::AcceptPlan => {
                i18n.tr(I18nKey::AgentImageDropPlanToLoadIntoTheAgent)().to_string()
            }
            Self::AcceptTask => i18n.tr(I18nKey::AgentImageDropTaskToAttachAsContext)().to_string(),
            Self::Reject => i18n.tr(I18nKey::AgentImageUnsupportedDrop)().to_string(),
        }
    }
}

pub fn install_agent_image_intake(
    wb: WorkbenchService,
    i18n: I18nService,
    drop_state: RwSignal<DropZoneState>,
    status_line: RwSignal<Option<String>>,
) {
    install_paste_listener(wb, i18n, status_line);
    install_escape_listener(drop_state);
    install_tauri_drop_listener(wb, i18n, drop_state, status_line);
}

pub fn handle_dom_drag_event(
    ev: DragEvent,
    wb: WorkbenchService,
    _i18n: I18nService,
    drop_state: RwSignal<DropZoneState>,
    slot_dnd: TerminalSlotDragService,
    context_dnd: ContextDragService,
    kanban_dnd: KanbanDragService,
) {
    if has_kanban_drag(&ev, kanban_dnd) {
        ev.prevent_default();
        ev.stop_propagation();
        let payload = kanban_dnd.active_payload.get_untracked();
        let active_ws = wb.active_id().get_untracked();
        let accept = payload
            .as_ref()
            .is_some_and(|p| active_ws == Some(p.workspace_id));
        if accept {
            if let Some(dt) = ev.data_transfer() {
                let _ = dt.set_drop_effect("copy");
            }
            kanban_dnd.set_overlay_pos_from_event(&ev);
            let state = match payload.as_ref().map(|p| p.kind) {
                Some(KanbanDragKind::Task) => DropZoneState::AcceptTask,
                _ => DropZoneState::AcceptPlan,
            };
            drop_state.set(state);
        } else {
            if let Some(dt) = ev.data_transfer() {
                let _ = dt.set_drop_effect("none");
            }
            drop_state.set(DropZoneState::Reject);
        }
        return;
    }
    if has_terminal_drag(&ev, slot_dnd) {
        ev.prevent_default();
        ev.stop_propagation();
        if let Some(dt) = ev.data_transfer() {
            let _ = dt.set_drop_effect("copy");
        }
        slot_dnd.set_overlay_pos_from_event(&ev);
        drop_state.set(DropZoneState::AcceptTerminal);
    } else if has_context_drag(&ev, context_dnd) {
        ev.prevent_default();
        ev.stop_propagation();
        if let Some(dt) = ev.data_transfer() {
            let _ = dt.set_drop_effect("copy");
        }
        context_dnd.set_overlay_pos_from_event(&ev);
        let state = match context_dnd.active.get_untracked().map(|m| m.kind) {
            Some(ContextDragKind::Folder) => DropZoneState::AcceptFolder,
            Some(ContextDragKind::Diff) => DropZoneState::AcceptDiff,
            Some(ContextDragKind::Commit) => DropZoneState::AcceptCommit,
            _ => DropZoneState::AcceptFile,
        };
        drop_state.set(state);
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
    i18n: I18nService,
    drop_state: RwSignal<DropZoneState>,
    status_line: RwSignal<Option<String>>,
    slot_dnd: TerminalSlotDragService,
    context_dnd: ContextDragService,
    kanban_dnd: KanbanDragService,
) {
    ev.prevent_default();
    drop_state.set(DropZoneState::Inactive);
    let Some(dt) = ev.data_transfer() else {
        return;
    };

    // Kanban plan/task dragged onto the agent. Checked first: it uses a distinct
    // MIME, so there is no overlap with terminal/context drags.
    if let Some(payload) =
        read_kanban_payload(&dt).or_else(|| kanban_dnd.active_payload.get_untracked())
    {
        ev.stop_propagation();
        attach_kanban_drop(payload, wb, i18n, status_line, kanban_dnd);
        return;
    }

    if let Some(payload) = read_drag_payload(&dt).or_else(|| terminal_payload_from_active(slot_dnd))
    {
        ev.stop_propagation();
        attach_terminal_context(payload, wb, i18n, status_line, slot_dnd);
        return;
    }

    if let Some(payload) =
        read_context_payload(&dt).or_else(|| context_dnd.active_payload.get_untracked())
    {
        ev.stop_propagation();
        attach_context_drag(payload, wb, i18n, status_line, context_dnd);
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
            read_dom_file(file, wb, i18n, status_line);
        }
    }
}

pub fn clear_drop_state(drop_state: RwSignal<DropZoneState>) {
    drop_state.set(DropZoneState::Inactive);
}

fn install_paste_listener(
    wb: WorkbenchService,
    i18n: I18nService,
    status_line: RwSignal<Option<String>>,
) {
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
                    read_dom_file(file, wb, i18n, status_line);
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
    i18n: I18nService,
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
                    read_tauri_file(path, wb, i18n, status_line);
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

fn read_tauri_file(
    path: String,
    wb: WorkbenchService,
    i18n: I18nService,
    status_line: RwSignal<Option<String>>,
) {
    leptos::task::spawn_local(async move {
        match agent_read_image_file(path).await {
            Ok(payload) => add_image_payload(wb, i18n, status_line, payload),
            Err(e) => status_line.set(Some(e)),
        }
    });
}

fn read_dom_file(
    file: File,
    wb: WorkbenchService,
    i18n: I18nService,
    status_line: RwSignal<Option<String>>,
) {
    let label = if file.name().trim().is_empty() {
        i18n.tr(I18nKey::AgentImagePastedImage)().to_string()
    } else {
        file.name()
    };
    let mime = file.type_();
    let size_bytes = file.size() as u64;
    if !is_supported_image_mime(&mime) {
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageUnsupportedFormat)().to_string(),
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
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageCouldNotCreateImageReader)().to_string(),
        ));
        return;
    };
    let reader_for_cb = reader.clone();
    let cb: Closure<dyn FnMut(web_sys::ProgressEvent)> =
        Closure::once(move |_ev: web_sys::ProgressEvent| {
            let result = reader_for_cb.result().ok().and_then(|v| v.as_string());
            let Some(data_url) = result else {
                status_line.set(Some(
                    i18n.tr(I18nKey::AgentImageCouldNotReadImageData)().to_string(),
                ));
                return;
            };
            let Some((mime_from_url, bytes_b64)) = parse_data_url(&data_url) else {
                status_line.set(Some(
                    i18n.tr(I18nKey::AgentImageCouldNotParseImageData)().to_string(),
                ));
                return;
            };
            add_image_payload(
                wb,
                i18n,
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
    i18n: I18nService,
    status_line: RwSignal<Option<String>>,
    payload: AgentImageFilePayload,
) {
    if !is_supported_image_mime(&payload.mime) {
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageUnsupportedFormat)().to_string(),
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
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageSelectAWorkspaceTabFirst)().to_string(),
        ));
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
    i18n: I18nService,
    status_line: RwSignal<Option<String>>,
    slot_dnd: TerminalSlotDragService,
) {
    let Some(active_ws_id) = wb.active_id().get_untracked() else {
        slot_dnd.clear();
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageSelectAWorkspaceTabFirst)().to_string(),
        ));
        return;
    };
    if payload.workspace_id != active_ws_id {
        slot_dnd.clear();
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageTerminalWorkspaceMismatch)().to_string(),
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

fn has_kanban_drag(ev: &DragEvent, kanban_dnd: KanbanDragService) -> bool {
    kanban_dnd.session_active() || ev.data_transfer().as_ref().is_some_and(is_kanban_drag)
}

/// Attach a Kanban plan or task dropped onto the agent. A plan is loaded into
/// the agent (tasks applied + `PlanFile` context, like the Plans panel button);
/// a task becomes a compact `PlanTaskGroup` context item. Cross-workspace drops
/// are rejected.
fn attach_kanban_drop(
    payload: KanbanDragPayload,
    wb: WorkbenchService,
    i18n: I18nService,
    status_line: RwSignal<Option<String>>,
    kanban_dnd: KanbanDragService,
) {
    let Some(active_ws_id) = wb.active_id().get_untracked() else {
        kanban_dnd.clear();
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageSelectAWorkspaceTabFirst)().to_string(),
        ));
        return;
    };
    if payload.workspace_id != active_ws_id {
        kanban_dnd.clear();
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageKanbanWorkspaceMismatch)().to_string(),
        ));
        return;
    }
    let ws_cwd = wb.workspaces().with_untracked(|list| {
        list.iter()
            .find(|w| w.id == active_ws_id)
            .map(|w| w.cwd.clone())
            .filter(|cwd| !cwd.trim().is_empty())
    });
    let Some(ws_cwd) = ws_cwd else {
        kanban_dnd.clear();
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageSelectAWorkspaceTabFirst)().to_string(),
        ));
        return;
    };

    match payload.kind {
        KanbanDragKind::Plan => {
            status_line.set(None);
            attach_plan_into_agent(
                wb,
                active_ws_id,
                ws_cwd,
                payload.plan_path,
                None,
                move |result| {
                    if let Err(err) = result {
                        status_line.set(Some(format!("Could not load plan: {err}")));
                    }
                },
            );
        }
        KanbanDragKind::Task => {
            if let Some(task_id) = payload.task_id {
                // The serialized payload carries only ids; the task title lives on
                // the in-memory drag meta. Fall back to the task id if absent.
                let title = kanban_dnd
                    .active
                    .get_untracked()
                    .map(|meta| meta.title)
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| task_id.clone());
                let item = plan_task_context_item(&payload.plan_path, &task_id, &title);
                wb.upsert_workspace_agent_context(active_ws_id, item);
                status_line.set(None);
            }
        }
    }
    kanban_dnd.clear();
}

fn attach_context_drag(
    payload: ContextDragPayload,
    wb: WorkbenchService,
    i18n: I18nService,
    status_line: RwSignal<Option<String>>,
    context_dnd: ContextDragService,
) {
    let Some(active_ws_id) = wb.active_id().get_untracked() else {
        context_dnd.clear();
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageSelectAWorkspaceTabFirst)().to_string(),
        ));
        return;
    };
    if payload.workspace_id != active_ws_id {
        context_dnd.clear();
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageContextWorkspaceMismatch)().to_string(),
        ));
        return;
    }

    match payload.kind {
        ContextDragKind::File => {
            let Some(rel) = payload.rel_path.filter(|p| !p.trim().is_empty()) else {
                context_dnd.clear();
                status_line.set(Some(
                    i18n.tr(I18nKey::AgentImageDraggedFileHasNoPath)().to_string(),
                ));
                return;
            };
            wb.upsert_workspace_agent_context(active_ws_id, file_ref_context_item(&rel));
            status_line.set(None);
            context_dnd.clear();
        }
        ContextDragKind::Folder => {
            let Some(rel) = payload.rel_path.filter(|p| !p.trim().is_empty()) else {
                context_dnd.clear();
                status_line.set(Some(
                    i18n.tr(I18nKey::AgentImageDraggedFolderHasNoPath)().to_string(),
                ));
                return;
            };
            wb.upsert_workspace_agent_context(active_ws_id, dir_ref_context_item(&rel));
            status_line.set(None);
            context_dnd.clear();
        }
        ContextDragKind::Diff => {
            let Some(rel) = payload.rel_path.filter(|p| !p.trim().is_empty()) else {
                context_dnd.clear();
                status_line.set(Some(
                    i18n.tr(I18nKey::AgentImageDraggedDiffHasNoPath)().to_string(),
                ));
                return;
            };
            let staged = payload.staged.unwrap_or(false);
            let Some(cwd) = wb.default_workspace_cwd() else {
                context_dnd.clear();
                status_line.set(Some(
                    i18n.tr(I18nKey::AgentImageWorkspaceHasNoPath)().to_string(),
                ));
                return;
            };
            let conn = wb.active_remote_connection_id();
            status_line.set(None);
            leptos::task::spawn_local(async move {
                match git_file_diff(cwd, rel.clone(), staged, conn).await {
                    Ok(diff) => {
                        let item = git_diff_context_item(&rel, staged, &diff);
                        wb.upsert_workspace_agent_context(active_ws_id, item);
                    }
                    Err(err) => status_line.set(Some(format!("Could not read diff: {err}"))),
                }
                context_dnd.clear();
            });
        }
        ContextDragKind::Commit => {
            let Some(oid) = payload.oid.filter(|o| !o.trim().is_empty()) else {
                context_dnd.clear();
                status_line.set(Some(
                    i18n.tr(I18nKey::AgentImageDraggedCommitHasNoId)().to_string(),
                ));
                return;
            };
            let short = payload
                .short_oid
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| oid.chars().take(7).collect());
            let subject = payload.subject.unwrap_or_default();
            let Some(cwd) = wb.default_workspace_cwd() else {
                context_dnd.clear();
                status_line.set(Some(
                    i18n.tr(I18nKey::AgentImageWorkspaceHasNoPath)().to_string(),
                ));
                return;
            };
            let conn = wb.active_remote_connection_id();
            status_line.set(None);
            leptos::task::spawn_local(async move {
                match git_commit_details(cwd, oid.clone(), conn).await {
                    Ok(details) => {
                        let paths: Vec<String> =
                            details.files.iter().map(|f| f.path.clone()).collect();
                        let item =
                            git_commit_context_item(&oid, &short, &subject, &details.body, &paths);
                        wb.upsert_workspace_agent_context(active_ws_id, item);
                    }
                    Err(err) => {
                        // Fall back to a metadata-only commit item so the drop
                        // still attaches something useful.
                        let item = git_commit_context_item(&oid, &short, &subject, "", &[]);
                        wb.upsert_workspace_agent_context(active_ws_id, item);
                        status_line.set(Some(format!("Commit details unavailable: {err}")));
                    }
                }
                context_dnd.clear();
            });
        }
    }
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

fn has_context_drag(ev: &DragEvent, context_dnd: ContextDragService) -> bool {
    ev.data_transfer()
        .as_ref()
        .map(crate::workbench::context_drag::is_context_drag)
        .unwrap_or(false)
        || context_dnd.session_active()
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
