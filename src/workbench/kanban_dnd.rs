//! Drag-and-drop helpers for the interactive Workspace Kanban board.
//!
//! This mirrors the terminal/context DnD services: a typed JSON payload is
//! stamped onto `DataTransfer`, while a small in-memory service keeps the
//! active preview, ghost target, and cursor position reliable across WebKit and
//! Chromium drag-event quirks.

use crate::agent_wire::TaskStatus;
use crate::tauri_bridge::KanbanPlanState;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use web_sys::{DataTransfer, DragEvent};

pub const KANBAN_DRAG_MIME: &str = "application/x-blxcode-kanban";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KanbanDragKind {
    Plan,
    Task,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanDragPayload {
    pub workspace_id: u64,
    pub kind: KanbanDragKind,
    pub plan_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KanbanDragMeta {
    pub kind: KanbanDragKind,
    pub title: String,
    pub subtitle: String,
    pub badge: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KanbanDropTarget {
    Plan {
        state: KanbanPlanState,
        before_plan_path: Option<String>,
    },
    Task {
        plan_path: String,
        status: TaskStatus,
        before_task_id: Option<String>,
    },
}

#[derive(Clone, Copy)]
pub struct KanbanDragService {
    session: StoredValue<bool>,
    session_gen: StoredValue<u64>,
    pub active: RwSignal<Option<KanbanDragMeta>>,
    pub active_payload: RwSignal<Option<KanbanDragPayload>>,
    pub ghost: RwSignal<Option<KanbanDropTarget>>,
    pub overlay_pos: RwSignal<Option<(f64, f64)>>,
}

impl Default for KanbanDragService {
    fn default() -> Self {
        Self::new()
    }
}

impl KanbanDragService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session: StoredValue::new(false),
            session_gen: StoredValue::new(0),
            active: RwSignal::new(None),
            active_payload: RwSignal::new(None),
            ghost: RwSignal::new(None),
            overlay_pos: RwSignal::new(None),
        }
    }

    pub fn begin_session(&self) -> u64 {
        self.session.set_value(true);
        let gen = self.session_gen.get_value().wrapping_add(1);
        self.session_gen.set_value(gen);
        self.overlay_pos.set(None);
        gen
    }

    pub fn session_active(&self) -> bool {
        self.session.get_value()
    }

    pub fn try_set_active(&self, gen: u64, meta: KanbanDragMeta) {
        if self.session.get_value() && self.session_gen.get_value() == gen {
            self.active.set(Some(meta));
        }
    }

    pub fn set_overlay_pos_from_event(&self, ev: &DragEvent) {
        if !self.session.get_value() {
            return;
        }
        let x = ev.client_x() as f64;
        let y = ev.client_y() as f64;
        if x <= 0.0 && y <= 0.0 {
            return;
        }
        self.overlay_pos.set(Some((x, y)));
    }

    pub fn clear(&self) {
        self.session.set_value(false);
        self.session_gen
            .set_value(self.session_gen.get_value().wrapping_add(1));
        self.active.set(None);
        self.active_payload.set(None);
        self.ghost.set(None);
        self.overlay_pos.set(None);
    }
}

pub fn set_drag_payload(dt: &DataTransfer, payload: &KanbanDragPayload) {
    if let Ok(json) = serde_json::to_string(payload) {
        let _ = dt.set_data(KANBAN_DRAG_MIME, &json);
        let fallback = payload
            .task_id
            .as_deref()
            .unwrap_or(payload.plan_path.as_str());
        let _ = dt.set_data("text/plain", fallback);
        let _ = dt.set_effect_allowed("move");
    }
}

pub fn read_drag_payload(dt: &DataTransfer) -> Option<KanbanDragPayload> {
    dt.get_data(KANBAN_DRAG_MIME)
        .ok()
        .filter(|json| !json.is_empty())
        .and_then(|json| serde_json::from_str(&json).ok())
}

pub fn is_kanban_drag(dt: &DataTransfer) -> bool {
    let types = dt.types();
    for i in 0..types.length() {
        if types.get(i) == KANBAN_DRAG_MIME {
            return true;
        }
    }
    false
}

pub fn start_kanban_drag(
    ev: &DragEvent,
    svc: KanbanDragService,
    payload: KanbanDragPayload,
    meta: KanbanDragMeta,
) {
    let Some(dt) = ev.data_transfer() else {
        return;
    };
    set_drag_payload(&dt, &payload);
    let gen = svc.begin_session();
    svc.active_payload.set(Some(payload));
    svc.set_overlay_pos_from_event(ev);
    spawn_local(async move {
        TimeoutFuture::new(0).await;
        svc.try_set_active(gen, meta);
    });
}
