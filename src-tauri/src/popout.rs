//! Tauri child-window popouts for focused workbench views.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const LABEL_PREFIX: &str = "popout";
pub const POPOUT_CLOSED_EVENT: &str = "popout_closed";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PopoutPayload {
    Terminal {
        workspace_id: u64,
        slot_id: u64,
        pane_id: u64,
        terminal_key: String,
    },
    Memory {
        workspace_id: u64,
        initial_view: Option<String>,
    },
    MemoryGraph {
        workspace_id: u64,
    },
    MermaidFile {
        workspace_id: u64,
        rel_path: String,
    },
    DiagramGallery {
        workspace_id: u64,
        scope: serde_json::Value,
    },
    FileDiff {
        workspace_id: u64,
        rel_path: String,
        staged: bool,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopoutClosedPayload {
    label: String,
}

impl PopoutPayload {
    fn kind_slug(&self) -> &'static str {
        match self {
            Self::Terminal { .. } => "terminal",
            Self::Memory { .. } => "memory",
            Self::MemoryGraph { .. } => "memory-graph",
            Self::MermaidFile { .. } => "mermaid-file",
            Self::DiagramGallery { .. } => "diagram-gallery",
            Self::FileDiff { .. } => "file-diff",
        }
    }

    fn title(&self) -> String {
        match self {
            Self::Terminal { slot_id, .. } => format!("Terminal #{slot_id}"),
            Self::Memory { .. } => "Memory".into(),
            Self::MemoryGraph { .. } => "Memory Graph".into(),
            Self::MermaidFile { rel_path, .. } => format!("Mermaid: {rel_path}"),
            Self::DiagramGallery { .. } => "Mermaid Diagrams".into(),
            Self::FileDiff {
                rel_path, staged, ..
            } => {
                let state = if *staged { "staged" } else { "unstaged" };
                format!("Diff: {rel_path} ({state})")
            }
        }
    }

    fn window_size(&self) -> (f64, f64) {
        match self {
            Self::Terminal { .. } => (1040.0, 680.0),
            Self::Memory { .. } | Self::MemoryGraph { .. } => (1180.0, 760.0),
            Self::MermaidFile { .. } | Self::DiagramGallery { .. } => (1160.0, 780.0),
            Self::FileDiff { .. } => (1120.0, 760.0),
        }
    }
}

fn payload_json(payload: &PopoutPayload) -> Result<String, String> {
    serde_json::to_string(payload).map_err(|e| format!("serialize popout payload: {e}"))
}

fn payload_hash(json: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    json.hash(&mut hasher);
    hasher.finish()
}

fn popout_label(payload: &PopoutPayload, json: &str) -> String {
    format!(
        "{LABEL_PREFIX}-{}-{:016x}",
        payload.kind_slug(),
        payload_hash(json)
    )
}

fn popout_url(json: &str) -> PathBuf {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
    PathBuf::from(format!("index.html?popout={encoded}"))
}

#[tauri::command]
pub async fn popout_open(app: tauri::AppHandle, payload: PopoutPayload) -> Result<String, String> {
    let json = payload_json(&payload)?;
    let label = popout_label(&payload, &json);
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(label);
    }

    let title = payload.title();
    let (width, height) = payload.window_size();
    let mut builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(popout_url(&json)))
        .title(title)
        .inner_size(width, height)
        .min_inner_size(480.0, 320.0)
        .decorations(false)
        .focused(true)
        .visible(true);

    if let Some(main) = app.get_webview_window("main") {
        builder = builder.parent(&main).map_err(|e| e.to_string())?;
    }

    builder.build().map_err(|e| e.to_string())?;
    Ok(label)
}

#[tauri::command]
pub fn popout_focus(app: tauri::AppHandle, label: String) -> Result<(), String> {
    let Some(window) = app.get_webview_window(&label) else {
        return Ok(());
    };
    let _ = window.unminimize();
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn popout_close_current(window: tauri::WebviewWindow) -> Result<(), String> {
    if !window.label().starts_with(LABEL_PREFIX) {
        return Err("not a popout window".into());
    }
    window.close().map_err(|e| e.to_string())
}

pub fn notify_popout_closed(window: &tauri::Window) {
    let label = window.label();
    if !label.starts_with(LABEL_PREFIX) {
        return;
    }
    let _ = window.emit_to(
        "main",
        POPOUT_CLOSED_EVENT,
        PopoutClosedPayload {
            label: label.to_string(),
        },
    );
}
