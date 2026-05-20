//! Native Child-Webviews — eine pro Browser-Tab, alle am gleichen Rect angedockt.
//! Tab-Wechsel zeigt/versteckt nur die jeweilige Webview (kein Reload).
//! Depends on **`tauri`/unstable** (`Window::add_child`).

use crate::commands::BrowserBoundsPayload;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};
use url::Url;

pub const DEFAULT_HOME_URL: &str = "https://blxcode.com";

/// Child-WebViews mit SPA-gestützten Bounds funktionieren zuverlässig nur dort,
/// wo das Tauri-/wry-Backend eine echte Unter-WebView einpasst (Windows: HWND-Child,
/// macOS: NSView addSubview). Auf Linux/GTK fügt `add_child` die Webview als
/// neues Kind in den `GtkBox`-Container des Fensters ein — `set_position`/`set_size`
/// werden dort ignoriert. Deshalb fällt Linux auf `<iframe>` zurück.
#[must_use]
pub fn native_child_inset_supported() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

#[must_use]
pub fn browser_embedding_kind_str() -> &'static str {
    if native_child_inset_supported() {
        "native_child"
    } else {
        "iframe_embed"
    }
}

fn label_for(tab_id: u64) -> String {
    format!("embedded-browser-{tab_id}")
}

#[derive(Default)]
pub struct BrowserHost {
    state: Mutex<HostState>,
}

#[derive(Default)]
struct HostState {
    /// tab_id → last navigated URL (für no-op `navigate()`-Skip)
    tabs: HashMap<u64, Option<String>>,
}

impl BrowserHost {
    /// Synchronisiert den aktiven Tab mit dem SPA-Rect, blendet alle anderen aus.
    /// Erzeugt die Webview beim ersten Sichtbar-Werden eines Tabs.
    pub fn sync_bounds(
        &self,
        app: &AppHandle,
        active_tab_id: Option<u64>,
        rect: BrowserBoundsPayload,
        navigate_to: Option<&str>,
    ) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "browser host poisoned")?;

        // Linux/iframe-Modus: keine nativen Childs anlegen, alles aufräumen.
        if !native_child_inset_supported() {
            for tid in state.tabs.keys().copied().collect::<Vec<_>>() {
                if let Some(wv) = app.get_webview(&label_for(tid)) {
                    let _ = wv.close();
                }
            }
            state.tabs.clear();
            return Ok(());
        }

        // Alle bekannten, nicht-aktiven Tabs ausblenden (Hintergrund-Tabs leben weiter).
        for tid in state.tabs.keys().copied().collect::<Vec<_>>() {
            if Some(tid) != active_tab_id {
                if let Some(wv) = app.get_webview(&label_for(tid)) {
                    let _ = wv.hide();
                }
            }
        }

        let Some(tab_id) = active_tab_id else {
            return Ok(());
        };

        let label = label_for(tab_id);

        // Webview für aktiven Tab anlegen falls noch nicht vorhanden.
        if !state.tabs.contains_key(&tab_id) {
            let start = navigate_to
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_HOME_URL);
            let u = Url::parse(start).map_err(|e| format!("URL: {e}"))?;
            let builder = WebviewBuilder::new(&label, WebviewUrl::External(u));
            let window = app
                .get_window("main")
                .ok_or_else(|| "Fenster 'main' fehlt".to_string())?;
            window
                .add_child(
                    builder,
                    LogicalPosition::new(rect.x.max(0.), rect.y.max(0.)),
                    LogicalSize::new(rect.w.max(2.), rect.h.max(2.)),
                )
                .map_err(|e| format!("webview add_child: {e}"))?;
            state.tabs.insert(tab_id, Some(start.to_string()));
        }

        let wv = app
            .get_webview(&label)
            .ok_or_else(|| format!("webview {label}"))?;

        wv.set_position(LogicalPosition::new(rect.x, rect.y))
            .map_err(|e| e.to_string())?;
        wv.set_size(LogicalSize::new(rect.w.max(2.), rect.h.max(2.)))
            .map_err(|e| e.to_string())?;

        let last = state.tabs.get_mut(&tab_id).expect("tab inserted above");
        if let Some(nav) = navigate_to
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter(|s| last.as_deref() != Some(*s))
        {
            let pu = Url::parse(nav).map_err(|e| format!("{e}"))?;
            wv.navigate(pu).map_err(|e| e.to_string())?;
            *last = Some(nav.to_string());
        }

        if rect.visible && rect.w >= 8. && rect.h >= 8. {
            wv.show().map_err(|e| e.to_string())?;
        } else {
            wv.hide().map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub fn close_tab(&self, app: &AppHandle, tab_id: u64) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "browser host poisoned")?;
        if state.tabs.remove(&tab_id).is_some() {
            if let Some(wv) = app.get_webview(&label_for(tab_id)) {
                let _ = wv.close();
            }
        }
        Ok(())
    }

    pub fn navigate(&self, app: &AppHandle, tab_id: u64, url: &str) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "browser host poisoned")?;
        let label = label_for(tab_id);
        let Some(wv) = app.get_webview(&label) else {
            return Err("Browser-Webview noch nicht angelegt (Tab öffnen).".into());
        };
        let trimmed = url.trim();
        let u = Url::parse(trimmed).map_err(|e| format!("{e}"))?;
        wv.navigate(u).map_err(|e| e.to_string())?;
        state.tabs.insert(tab_id, Some(trimmed.to_string()));
        Ok(())
    }

    pub fn eval_embedded(
        &self,
        app: &AppHandle,
        tab_id: u64,
        js: impl Into<String>,
    ) -> Result<(), String> {
        let label = label_for(tab_id);
        let wv = app
            .get_webview(&label)
            .ok_or_else(|| "Browser-Webview noch nicht erstellt.".to_string())?;
        wv.eval(js.into()).map_err(|e| e.to_string())
    }
}
