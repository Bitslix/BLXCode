//! Compile-time translation tables (no Leptos).
pub mod eula;
mod keys;
mod locale;
mod locales;
mod resolve;

pub use eula::localized_eula_html;
pub use keys::I18nKey;
pub use locale::{Locale, APP_LOCALES};
pub use resolve::lookup;

use crate::config::I18N_LOCALE_STORAGE_KEY;

/// Detects initial locale: storage → browser language → default.
#[must_use]
pub fn detect_initial_locale() -> Locale {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(raw)) = storage.get_item(I18N_LOCALE_STORAGE_KEY) {
                let t = raw.trim();
                if !t.is_empty() {
                    if let Some(l) = Locale::parse_bcp47(t) {
                        return l;
                    }
                }
            }
        }
        if let Some(lang) = window.navigator().language() {
            if !lang.is_empty() {
                if let Some(l) = Locale::parse_bcp47(lang.as_str()) {
                    return l;
                }
                return Locale::infer_from_browser_lang(lang.as_str());
            }
        }
    }
    Locale::default()
}
