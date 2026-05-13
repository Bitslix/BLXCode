//! Native child webview anchored to rects from the SPA (logical px).
//! Depends on **`tauri`/unstable** (`Window::add_child`).

use crate::commands::BrowserBoundsPayload;
use std::sync::Mutex;
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};
use url::Url;

const LABEL: &str = "embedded-browser";
pub const DEFAULT_HOME_URL: &str = "https://bitslix.com";

/// Child-WebViews mit SPA-gestützten Bounds funktionieren zuverlässig nur dort, wo das
/// Tauri-/Wry-Backend eine echte Unter-WebView einpasst (nicht unter Linux GTK ohne GtkFixed-X11-Inset).
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

#[derive(Default)]
pub struct BrowserHost {
    /// Whether `add_child` completed at least once.
    lock: Mutex<()>,
}

impl BrowserHost {
    pub fn sync_bounds(
        &self,
        app: &AppHandle,
        rect: BrowserBoundsPayload,
        navigate_to: Option<&str>,
    ) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|_| "browser lock poisoned")?;

        if !native_child_inset_supported() {
            if let Some(wv) = app.get_webview(LABEL) {
                let _ = wv.close();
            }
            return Ok(());
        }

        if app.get_webview(LABEL).is_none() {
            let window = app
                .get_window("main")
                .ok_or_else(|| "Fenster 'main' fehlt".to_string())?;

            let start = navigate_to
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_HOME_URL);

            let u = Url::parse(start).map_err(|e| format!("URL: {e}"))?;

            let builder = WebviewBuilder::new(LABEL, WebviewUrl::External(u));

            window
                .add_child(
                    builder,
                    LogicalPosition::new(rect.x.max(0.), rect.y.max(0.)),
                    LogicalSize::new(rect.w.max(2.), rect.h.max(2.)),
                )
                .map_err(|e| format!("webview add_child: {e}"))?;
        }

        let wv = app
            .get_webview(LABEL)
            .ok_or_else(|| format!("webview {LABEL}"))?;

        wv.set_position(LogicalPosition::new(rect.x, rect.y))
            .map_err(|e| e.to_string())?;
        wv.set_size(LogicalSize::new(rect.w.max(2.), rect.h.max(2.)))
            .map_err(|e| e.to_string())?;

        if let Some(nav) = navigate_to.filter(|s| !s.is_empty()) {
            let pu = Url::parse(nav).map_err(|e| format!("{e}"))?;
            wv.navigate(pu).map_err(|e| e.to_string())?;
        }

        if rect.visible && rect.w >= 8. && rect.h >= 8. {
            wv.show().map_err(|e| e.to_string())?;
        } else {
            wv.hide().map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub fn eval_embedded(&self, app: &AppHandle, js: impl Into<String>) -> Result<(), String> {
        if !native_child_inset_supported() {
            return Ok(());
        }
        let wv = app
            .get_webview(LABEL)
            .ok_or_else(|| "Browser-Webview noch nicht erstellt.".to_string())?;
        wv.eval(js.into()).map_err(|e| e.to_string())
    }

    pub fn navigate(&self, app: &AppHandle, url: &str) -> Result<(), String> {
        if !native_child_inset_supported() {
            return Ok(());
        }
        let Some(wv) = app.get_webview(LABEL) else {
            return Err("Browser-Webview noch nicht angelegt (Tab öffnen).".into());
        };
        let u = Url::parse(url.trim()).map_err(|e| format!("{e}"))?;
        wv.navigate(u).map_err(|e| e.to_string())
    }
}
