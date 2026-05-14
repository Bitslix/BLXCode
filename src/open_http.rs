//! Helpers for resolving HTTP(S) targets from raw DOM events (e.g. Markdown rendered via `inner_html`).

use wasm_bindgen::JsCast;

/// Left-click on an `<a href="http(s):…">` only; ignores relative, `mailto:`, `javascript:`, `blob:`, and `#fragment` links.
#[must_use]
pub fn dom_click_http_url_from_mouse_event(ev: &web_sys::MouseEvent) -> Option<String> {
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
    if href.starts_with("mailto:")
        || href.starts_with("javascript:")
        || href.starts_with("blob:")
    {
        return None;
    }
    if href.starts_with("http://") || href.starts_with("https://") {
        return Some(href.to_string());
    }
    None
}
