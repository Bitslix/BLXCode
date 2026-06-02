//! App theme selection persisted in `localStorage` and applied via `data-theme` on `<html>`.
//!
//! Alongside the theme id this also owns two theme-independent appearance knobs
//! — the corner-roundings scale (`--radius-scale`) and the font family
//! (`--font-mono`) — both applied as inline custom properties on `<html>` and
//! persisted in `localStorage`. Any change dispatches `blxcode-theme-changed`
//! so JS bridges (xterm, graph) re-read the CSS variables.

use crate::config::{FONT_FAMILY_STORAGE_KEY, RADIUS_SCALE_STORAGE_KEY, THEME_STORAGE_KEY};
use crate::theme::{
    font_stack_for, is_valid_font_id, RadiusScale, DEFAULT_FONT_ID, DEFAULT_THEME_ID, THEMES,
};
use js_sys;
use leptos::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

#[allow(dead_code)]
pub const THEME_CHANGED_EVENT: &str = "blxcode-theme-changed"; // terminal_bootstrap.mjs / graph3d

#[derive(Clone, Copy)]
pub struct ThemeService {
    active_theme_id: RwSignal<String>,
    radius_scale: RwSignal<RadiusScale>,
    font_id: RwSignal<String>,
}

impl ThemeService {
    #[must_use]
    pub fn new() -> Self {
        let id = read_theme_storage();
        apply_theme_to_dom(&id);

        let radius = RadiusScale::from_storage(read_string(RADIUS_SCALE_STORAGE_KEY).as_deref());
        apply_radius_to_dom(radius);

        let font_id = read_font_storage();
        apply_font_to_dom(&font_id);

        Self {
            active_theme_id: RwSignal::new(id),
            radius_scale: RwSignal::new(radius),
            font_id: RwSignal::new(font_id),
        }
    }

    #[must_use]
    pub fn active_theme_id(&self) -> RwSignal<String> {
        self.active_theme_id
    }

    pub fn set_theme(&self, theme_id: &str) {
        let id = if crate::theme::is_valid_theme_id(theme_id) {
            theme_id.to_string()
        } else {
            DEFAULT_THEME_ID.to_string()
        };
        self.active_theme_id.set(id.clone());
        apply_theme_to_dom(&id);
        write_string(THEME_STORAGE_KEY, &id);
        dispatch_theme_changed(&id);
    }

    #[must_use]
    pub fn radius_scale(&self) -> RwSignal<RadiusScale> {
        self.radius_scale
    }

    pub fn set_radius_scale(&self, scale: RadiusScale) {
        self.radius_scale.set(scale);
        apply_radius_to_dom(scale);
        write_string(RADIUS_SCALE_STORAGE_KEY, scale.storage_value());
        dispatch_theme_changed(&self.active_theme_id.get_untracked());
    }

    #[must_use]
    pub fn font_id(&self) -> RwSignal<String> {
        self.font_id
    }

    pub fn set_font(&self, font_id: &str) {
        let id = if is_valid_font_id(font_id) {
            font_id.to_string()
        } else {
            DEFAULT_FONT_ID.to_string()
        };
        self.font_id.set(id.clone());
        apply_font_to_dom(&id);
        write_string(FONT_FAMILY_STORAGE_KEY, &id);
        dispatch_theme_changed(&self.active_theme_id.get_untracked());
    }
}

impl Default for ThemeService {
    fn default() -> Self {
        Self::new()
    }
}

fn read_theme_storage() -> String {
    read_string(THEME_STORAGE_KEY)
        .filter(|id| crate::theme::is_valid_theme_id(id))
        .unwrap_or_else(|| DEFAULT_THEME_ID.to_string())
}

fn read_font_storage() -> String {
    read_string(FONT_FAMILY_STORAGE_KEY)
        .filter(|id| is_valid_font_id(id))
        .unwrap_or_else(|| DEFAULT_FONT_ID.to_string())
}

fn read_string(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(key).ok().flatten())
}

fn write_string(key: &str, value: &str) {
    if let Some(w) = web_sys::window() {
        if let Ok(Some(s)) = w.local_storage() {
            let _ = s.set_item(key, value);
        }
    }
}

fn root_element() -> Option<web_sys::HtmlElement> {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
}

fn apply_theme_to_dom(theme_id: &str) {
    if let Some(root) = root_element() {
        let _ = root.set_attribute("data-theme", theme_id);
    }
}

fn apply_radius_to_dom(scale: RadiusScale) {
    if let Some(root) = root_element() {
        let _ = root
            .style()
            .set_property("--radius-scale", scale.multiplier());
    }
}

fn apply_font_to_dom(font_id: &str) {
    if let Some(root) = root_element() {
        let _ = root
            .style()
            .set_property("--font-mono", font_stack_for(font_id));
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
