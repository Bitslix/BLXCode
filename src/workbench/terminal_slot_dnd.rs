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
}

impl TerminalSlotDragService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session: StoredValue::new(false),
            session_gen: StoredValue::new(0),
            active: RwSignal::new(None),
            ghost: RwSignal::new(None),
        }
    }

    pub fn begin_session(&self) -> u64 {
        self.session.set_value(true);
        let gen = self.session_gen.get_value().wrapping_add(1);
        self.session_gen.set_value(gen);
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

    pub fn clear(&self) {
        self.session.set_value(false);
        self.session_gen
            .set_value(self.session_gen.get_value().wrapping_add(1));
        self.active.set(None);
        self.ghost.set(None);
    }
}

pub fn set_drag_payload(dt: &DataTransfer, payload: &TerminalSlotDragPayload) {
    if let Ok(json) = serde_json::to_string(payload) {
        let _ = dt.set_data(TERMINAL_SLOT_MIME, &json);
        let _ = dt.set_data("text/plain", &payload.slot_id.to_string());
        let _ = dt.set_effect_allowed("move");
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

pub fn ghost_style(pos: &GhostPos, target_index: usize) -> String {
    let rows = pos.rows.max(1) as f64;
    let cols = pos.cols.max(1) as f64;
    let row = (target_index as f64 / cols).floor();
    let col = target_index as f64 % cols;
    format!(
        "left:{:.4}%;top:{:.4}%;width:{:.4}%;height:{:.4}%;",
        (col / cols) * 100.0,
        (row / rows) * 100.0,
        100.0 / cols,
        100.0 / rows,
    )
}

pub fn drag_event_data_transfer(ev: &web_sys::DragEvent) -> Option<web_sys::DataTransfer> {
    ev.data_transfer()
}
