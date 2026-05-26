//! App theme selection persisted in `localStorage` and applied via `data-theme` on `<html>`.

use crate::config::THEME_STORAGE_KEY;
use crate::theme::{theme_by_id, DEFAULT_THEME_ID, THEMES};
use js_sys;
use leptos::prelude::*;
use wasm_bindgen::JsValue;

#[allow(dead_code)]
pub const THEME_CHANGED_EVENT: &str = "blxcode-theme-changed"; // terminal_bootstrap.mjs / graph3d

#[derive(Clone, Copy)]
pub struct ThemeService {
    active_theme_id: RwSignal<String>,
}

impl ThemeService {
    #[must_use]
    pub fn new() -> Self {
        let id = read_theme_storage();
        apply_theme_to_dom(&id);
        Self {
            active_theme_id: RwSignal::new(id),
        }
    }

    #[must_use]
    pub fn active_theme_id(&self) -> RwSignal<String> {
        self.active_theme_id
    }

    #[must_use]
    pub fn active_theme(&self) -> impl Fn() -> &'static crate::theme::AppTheme + Copy {
        let sig = self.active_theme_id;
        move || {
            theme_by_id(&sig.get())
                .unwrap_or_else(|| theme_by_id(DEFAULT_THEME_ID).expect("default theme exists"))
        }
    }

    pub fn set_theme(&self, theme_id: &str) {
        let id = if crate::theme::is_valid_theme_id(theme_id) {
            theme_id.to_string()
        } else {
            DEFAULT_THEME_ID.to_string()
        };
        self.active_theme_id.set(id.clone());
        apply_theme_to_dom(&id);
        write_theme_storage(&id);
        dispatch_theme_changed(&id);
    }
}

impl Default for ThemeService {
    fn default() -> Self {
        Self::new()
    }
}

fn read_theme_storage() -> String {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(THEME_STORAGE_KEY).ok().flatten())
        .filter(|id| crate::theme::is_valid_theme_id(id))
        .unwrap_or_else(|| DEFAULT_THEME_ID.to_string())
}

fn write_theme_storage(theme_id: &str) {
    if let Some(w) = web_sys::window() {
        if let Ok(Some(s)) = w.local_storage() {
            let _ = s.set_item(THEME_STORAGE_KEY, theme_id);
        }
    }
}

fn apply_theme_to_dom(theme_id: &str) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(root) = doc.document_element() {
            let _ = root.set_attribute("data-theme", theme_id);
        }
    }
}

fn dispatch_theme_changed(theme_id: &str) {
    if let Some(w) = web_sys::window() {
        let detail = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &detail,
            &JsValue::from_str("themeId"),
            &JsValue::from_str(theme_id),
        );
        if let Ok(ev) = web_sys::CustomEvent::new("blxcode-theme-changed") {
            let _ = js_sys::Reflect::set(&ev, &JsValue::from_str("detail"), &detail);
            let _ = w.dispatch_event(&ev);
        }
    }
}

#[must_use]
pub fn theme_count() -> usize {
    THEMES.len()
}
