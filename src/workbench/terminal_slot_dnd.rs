//! Drag-and-drop helpers for reordering terminal slots within a workspace grid.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use web_sys::DataTransfer;

pub const TERMINAL_SLOT_MIME: &str = "application/x-blxcode-terminal-slot";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSlotDragPayload {
    pub workspace_id: u64,
    pub slot_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GhostPos {
    pub target_slot_id: u64,
    pub rows: u8,
    pub cols: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalDragMeta {
    pub workspace_id: u64,
    pub slot_id: u64,
    pub title: String,
    pub agent_label: String,
}

#[derive(Clone, Copy)]
pub struct TerminalSlotDragService {
    session: StoredValue<bool>,
    /// Bumped on begin/clear so deferred UI updates cannot resurrect stale drags.
    session_gen: StoredValue<u64>,
    pub active: RwSignal<Option<TerminalDragMeta>>,
    pub ghost: RwSignal<Option<GhostPos>>,
    /// Cursor position (viewport-relative px) for the floating drag
    /// preview. `None` until the first `drag`/`dragover` event with valid
    /// coordinates lands during this session.
    pub overlay_pos: RwSignal<Option<(f64, f64)>>,
}

impl TerminalSlotDragService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session: StoredValue::new(false),
            session_gen: StoredValue::new(0),
            active: RwSignal::new(None),
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

    pub fn try_set_active(&self, gen: u64, meta: TerminalDragMeta) {
        if self.session.get_value() && self.session_gen.get_value() == gen {
            self.active.set(Some(meta));
        }
    }

    /// Update the overlay's cursor coordinates from a native drag event.
    /// WebKit can emit `(0, 0)` at the end of a drag; we skip those so
    /// the preview doesn't snap to the viewport corner on release.
    pub fn set_overlay_pos_from_event(&self, ev: &web_sys::DragEvent) {
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
        self.ghost.set(None);
        self.overlay_pos.set(None);
    }
}

pub fn set_drag_payload(dt: &DataTransfer, payload: &TerminalSlotDragPayload) {
    if let Ok(json) = serde_json::to_string(payload) {
        let _ = dt.set_data(TERMINAL_SLOT_MIME, &json);
        let _ = dt.set_data("text/plain", &payload.slot_id.to_string());
        let _ = dt.set_effect_allowed("copyMove");
    }
}

pub fn read_drag_payload(dt: &DataTransfer) -> Option<TerminalSlotDragPayload> {
    dt.get_data(TERMINAL_SLOT_MIME)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
}

pub fn is_terminal_drag(dt: &DataTransfer) -> bool {
    let types = dt.types();
    for i in 0..types.length() {
        if types.get(i) == TERMINAL_SLOT_MIME {
            return true;
        }
    }
    false
}

pub fn drag_event_data_transfer(ev: &web_sys::DragEvent) -> Option<web_sys::DataTransfer> {
    ev.data_transfer()
}
