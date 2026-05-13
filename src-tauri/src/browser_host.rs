//! Native child webview anchored to rects from the SPA (logical px).
//! Depends on **`tauri`/unstable** (`Window::add_child`).

use crate::commands::BrowserBoundsPayload;
use std::sync::Mutex;
use tauri::webview::WebviewBuilder;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};
use url::Url;

const LABEL: &str = "embedded-browser";
pub const DEFAULT_HOME_URL: &str = "https://bitslix.com";

#[must_use]
pub fn native_child_inset_supported() -> bool {
    true
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
    last_url: Mutex<Option<String>>,
}

impl BrowserHost {
    pub fn sync_bounds(
        &self,
        app: &AppHandle,
        rect: BrowserBoundsPayload,
        navigate_to: Option<&str>,
    ) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|_| "browser lock poisoned")?;
        let mut last_url = self
            .last_url
            .lock()
            .map_err(|_| "browser last_url lock poisoned")?;

        if !native_child_inset_supported() {
            if let Some(wv) = app.get_webview(LABEL) {
                let _ = wv.close();
            }
            *last_url = None;
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
            *last_url = Some(start.to_string());
        }

        let wv = app
            .get_webview(LABEL)
            .ok_or_else(|| format!("webview {LABEL}"))?;

        wv.set_position(LogicalPosition::new(rect.x, rect.y))
            .map_err(|e| e.to_string())?;
        wv.set_size(LogicalSize::new(rect.w.max(2.), rect.h.max(2.)))
            .map_err(|e| e.to_string())?;

        if let Some(nav) = navigate_to
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter(|s| last_url.as_deref() != Some(*s))
        {
            let pu = Url::parse(nav).map_err(|e| format!("{e}"))?;
            wv.navigate(pu).map_err(|e| e.to_string())?;
            *last_url = Some(nav.to_string());
        }

        if rect.visible && rect.w >= 8. && rect.h >= 8. {
            wv.show().map_err(|e| e.to_string())?;
        } else {
            wv.hide().map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub fn eval_embedded(&self, app: &AppHandle, js: impl Into<String>) -> Result<(), String> {
        let wv = app
            .get_webview(LABEL)
            .ok_or_else(|| "Browser-Webview noch nicht erstellt.".to_string())?;
        wv.eval(js.into()).map_err(|e| e.to_string())
    }

    pub fn navigate(&self, app: &AppHandle, url: &str) -> Result<(), String> {
        let Some(wv) = app.get_webview(LABEL) else {
            return Err("Browser-Webview noch nicht angelegt (Tab öffnen).".into());
        };
        let trimmed = url.trim();
        let u = Url::parse(trimmed).map_err(|e| format!("{e}"))?;
        wv.navigate(u).map_err(|e| e.to_string())
            .map(|_| {
                if let Ok(mut last_url) = self.last_url.lock() {
                    *last_url = Some(trimmed.to_string());
                }
            })
    }
}
