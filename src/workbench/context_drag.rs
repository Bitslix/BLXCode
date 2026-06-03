//! Drag-and-drop helpers for attaching workspace artifacts (files, diffs,
//! commits) to the Agent panel's context drop zone.
//!
//! Mirrors [`crate::workbench::terminal_slot_dnd`]: a single MIME type carries
//! a typed JSON payload, and a `Copy` service holds the in-flight drag meta +
//! cursor position so the floating preview overlay
//! ([`crate::workbench::context_drag_overlay`]) can follow the pointer on every
//! platform without relying on `DataTransfer::set_drag_image` (which WebKitGTK
//! aborts if the image isn't ready synchronously).

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use web_sys::{DataTransfer, DragEvent};

/// Custom MIME used for all context drags out of the sidebar into the Agent
/// drop zone. Distinct from the terminal-slot MIME so the two DnD flows never
/// collide inside the shared drop handler.
pub const CONTEXT_DRAG_MIME: &str = "application/x-blxcode-context";

/// What kind of workspace artifact is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextDragKind {
    /// A whole file from the project explorer (attached as a path reference).
    File,
    /// A single file's git diff from the diff sidebar.
    Diff,
    /// A git commit from the commit graph.
    Commit,
}

/// Serialized payload written to the drag `DataTransfer`. Optional fields are
/// populated per `kind`; the drop handler reads exactly what it needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDragPayload {
    pub workspace_id: u64,
    pub kind: ContextDragKind,
    /// Workspace-relative path (File / Diff).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel_path: Option<String>,
    /// Whether the diff should be read from the staged tree (Diff only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staged: Option<bool>,
    /// Full commit hash (Commit only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    /// Abbreviated commit hash (Commit only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_oid: Option<String>,
    /// Commit subject line (Commit only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// In-flight drag metadata used to render the floating preview card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextDragMeta {
    pub kind: ContextDragKind,
    /// Primary line of the preview (file name / short oid).
    pub title: String,
    /// Secondary line (relative path / commit subject).
    pub subtitle: String,
}

/// Thread-through DnD state for the context drag flow. `Copy` so it can be
/// captured by value into Leptos closures, exactly like
/// [`crate::workbench::terminal_slot_dnd::TerminalSlotDragService`].
#[derive(Clone, Copy)]
pub struct ContextDragService {
    session: StoredValue<bool>,
    /// Bumped on begin/clear so deferred UI updates cannot resurrect stale drags.
    session_gen: StoredValue<u64>,
    pub active: RwSignal<Option<ContextDragMeta>>,
    /// Cursor position (viewport-relative px) for the floating preview.
    pub overlay_pos: RwSignal<Option<(f64, f64)>>,
}

impl Default for ContextDragService {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextDragService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session: StoredValue::new(false),
            session_gen: StoredValue::new(0),
            active: RwSignal::new(None),
            overlay_pos: RwSignal::new(None),
        }
    }

    /// Begin a drag session and return its generation token. The caller seeds
    /// `try_set_active` with the same token so a late `setTimeout(0)` callback
    /// from a superseded drag can't overwrite a newer one.
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

    pub fn try_set_active(&self, gen: u64, meta: ContextDragMeta) {
        if self.session.get_value() && self.session_gen.get_value() == gen {
            self.active.set(Some(meta));
        }
    }

    /// Update the overlay's cursor coordinates from a native drag event.
    /// WebKit can emit `(0, 0)` at the end of a drag; we skip those so the
    /// preview doesn't snap to the viewport corner on release.
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
        self.overlay_pos.set(None);
    }
}

/// Write the payload onto the drag `DataTransfer` under the context MIME plus a
/// human-readable `text/plain` fallback.
pub fn set_drag_payload(dt: &DataTransfer, payload: &ContextDragPayload) {
    if let Ok(json) = serde_json::to_string(payload) {
        let _ = dt.set_data(CONTEXT_DRAG_MIME, &json);
        let fallback = payload
            .rel_path
            .clone()
            .or_else(|| payload.short_oid.clone())
            .unwrap_or_default();
        let _ = dt.set_data("text/plain", &fallback);
        let _ = dt.set_effect_allowed("copy");
    }
}

pub fn read_drag_payload(dt: &DataTransfer) -> Option<ContextDragPayload> {
    dt.get_data(CONTEXT_DRAG_MIME)
        .ok()
        .filter(|json| !json.is_empty())
        .and_then(|json| serde_json::from_str(&json).ok())
}

/// Wire a `dragstart` from a sidebar row: stamp the payload onto the
/// `DataTransfer`, open a drag session, and (deferred by a `setTimeout(0)`)
/// publish the preview meta. Deferral lets the native drag begin before our DOM
/// mutates, mirroring the terminal-slot drag start.
pub fn start_context_drag(
    ev: &DragEvent,
    svc: ContextDragService,
    payload: ContextDragPayload,
    meta: ContextDragMeta,
) {
    let Some(dt) = ev.data_transfer() else {
        return;
    };
    set_drag_payload(&dt, &payload);
    let gen = svc.begin_session();
    svc.set_overlay_pos_from_event(ev);
    spawn_local(async move {
        TimeoutFuture::new(0).await;
        svc.try_set_active(gen, meta);
    });
}

pub fn is_context_drag(dt: &DataTransfer) -> bool {
    let types = dt.types();
    for i in 0..types.length() {
        if types.get(i) == CONTEXT_DRAG_MIME {
            return true;
        }
    }
    false
}
