//! Custom title-bar window controls.
//!
//! With `decorations: false` the app draws its own title bar, so the
//! minimize / maximize / close / fullscreen affordances are served by these
//! Tauri commands instead of the OS chrome. Keeping the privileged window
//! operations server-side means the frontend only needs the drag permission
//! (`core:window:allow-start-dragging`) — not the broader JS window API.
//!
//! Every command operates on the `main` `WebviewWindow`. They are no-ops
//! (returning `Ok`) when that window is unavailable, so a missing window can
//! never panic the IPC boundary.

use serde::Serialize;
use tauri::{Emitter, LogicalSize, Manager, Size};

/// Event emitted after a state change so the bar's maximize/restore icon can
/// react without polling. Payload is the current `is_maximized` flag.
const WINDOW_STATE_EVENT: &str = "blxcode://window-state";

fn main_window(app: &tauri::AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window("main")
}

/// Broadcasts the maximized flag so the title bar can refresh its restore icon.
fn emit_window_state(app: &tauri::AppHandle, maximized: bool) {
    let _ = app.emit(WINDOW_STATE_EVENT, maximized);
}

#[tauri::command]
pub fn window_minimize(app: tauri::AppHandle) -> Result<(), String> {
    let Some(win) = main_window(&app) else {
        return Ok(());
    };
    win.minimize().map_err(|e| e.to_string())
}

/// Maximize when restored, unmaximize when maximized. Mirrors the OS
/// maximize button and the double-click-on-titlebar behaviour.
#[tauri::command]
pub fn window_toggle_maximize(app: tauri::AppHandle) -> Result<bool, String> {
    let Some(win) = main_window(&app) else {
        return Ok(false);
    };
    let maximized = win.is_maximized().map_err(|e| e.to_string())?;
    if maximized {
        win.unmaximize().map_err(|e| e.to_string())?;
    } else {
        win.maximize().map_err(|e| e.to_string())?;
    }
    let now = !maximized;
    emit_window_state(&app, now);
    Ok(now)
}

#[tauri::command]
pub fn window_is_maximized(app: tauri::AppHandle) -> Result<bool, String> {
    let Some(win) = main_window(&app) else {
        return Ok(false);
    };
    win.is_maximized().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn window_close(app: tauri::AppHandle) -> Result<(), String> {
    let Some(win) = main_window(&app) else {
        return Ok(());
    };
    // `close()` runs the normal close path (fires the `beforeunload` flush
    // already wired in the shell), matching the OS close button.
    win.close().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn window_toggle_fullscreen(app: tauri::AppHandle) -> Result<bool, String> {
    let Some(win) = main_window(&app) else {
        return Ok(false);
    };
    let fullscreen = win.is_fullscreen().map_err(|e| e.to_string())?;
    let next = !fullscreen;
    win.set_fullscreen(next).map_err(|e| e.to_string())?;
    Ok(next)
}

#[tauri::command]
pub fn window_is_fullscreen(app: tauri::AppHandle) -> Result<bool, String> {
    let Some(win) = main_window(&app) else {
        return Ok(false);
    };
    win.is_fullscreen().map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    pub fullscreen: bool,
}

#[tauri::command]
pub fn window_state(app: tauri::AppHandle) -> Result<WindowState, String> {
    let Some(win) = main_window(&app) else {
        return Ok(WindowState {
            width: 0,
            height: 0,
            maximized: false,
            fullscreen: false,
        });
    };
    let size = win.inner_size().map_err(|e| e.to_string())?;
    Ok(WindowState {
        width: size.width,
        height: size.height,
        maximized: win.is_maximized().map_err(|e| e.to_string())?,
        fullscreen: win.is_fullscreen().map_err(|e| e.to_string())?,
    })
}

#[tauri::command]
pub fn window_set_size(app: tauri::AppHandle, width: u32, height: u32) -> Result<(), String> {
    let Some(win) = main_window(&app) else {
        return Ok(());
    };
    let width = width.clamp(480, 7680);
    let height = height.clamp(360, 4320);
    win.set_size(Size::Logical(LogicalSize::new(width as f64, height as f64)))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn window_set_fullscreen(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let Some(win) = main_window(&app) else {
        return Ok(());
    };
    win.set_fullscreen(enabled).map_err(|e| e.to_string())
}
