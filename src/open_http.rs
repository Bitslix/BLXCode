//! Helpers for resolving navigation targets from raw DOM events (e.g. Markdown `inner_html`).

use crate::memory_paths::{sanitize_memory_relative_path, slug_to_filename};
use wasm_bindgen::JsCast;

#[derive(Debug)]
pub enum DomNavHref {
    Http(String),
    Memory(String),
}

/// Left-click on `<a href>`: `http(s):`, `blxmemory:`, or `mailto`/`javascript`/`blob` ignored.
#[must_use]
pub fn dom_click_nav_href(ev: &web_sys::MouseEvent) -> Option<DomNavHref> {
    if ev.button() != 0 {
        return None;
    }
    let t = ev.target()?;
    let el = t.dyn_into::<web_sys::Element>().ok()?;
    let a = el.closest("a[href]").ok().flatten()?;
    let href = a.get_attribute("href")?;
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return None;
    }
    if href.starts_with("mailto:") || href.starts_with("javascript:") || href.starts_with("blob:") {
        return None;
    }
    if let Some(rest) = href.strip_prefix("blxmemory:") {
        let raw = rest.trim();
        let rel = sanitize_memory_relative_path(raw).or_else(|| {
            let slug = slug_to_filename(raw);
            sanitize_memory_relative_path(&slug)
        })?;
        return Some(DomNavHref::Memory(rel));
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(DomNavHref::Http(href.to_string()));
    }
    None
}

/// EULA / HTTP-only: same as [`dom_click_nav_href`] but returns only `http(s)`.
#[must_use]
pub fn dom_click_http_url_from_mouse_event(ev: &web_sys::MouseEvent) -> Option<String> {
    match dom_click_nav_href(ev)? {
        DomNavHref::Http(s) => Some(s),
        DomNavHref::Memory(_) => None,
    }
}
