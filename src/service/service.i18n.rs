//! Reactive i18n facade for Leptos (locale signal, persistence, `lang` on `<html>`).
use crate::config::I18N_LOCALE_STORAGE_KEY;
use crate::i18n::{lookup, I18nKey, Locale};
use leptos::prelude::*;

/// Reactive translation service; provide with [`leptos::prelude::provide_context`].
#[derive(Clone, Copy)]
pub struct I18nService {
    locale: RwSignal<Locale>,
}

impl I18nService {
    /// Reads initial locale synchronously (storage → browser → default) and sets `<html lang>`.
    #[must_use]
    pub fn new() -> Self {
        let initial = crate::i18n::detect_initial_locale();
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Some(root) = doc.document_element() {
                let _ = root.set_attribute("lang", initial.as_str());
            }
        }
        Self {
            locale: RwSignal::new(initial),
        }
    }

    #[must_use]
    pub fn locale(&self) -> RwSignal<Locale> {
        self.locale
    }

    /// Persists BCP-47 tag to storage and updates `<html lang>` (e.g. language switcher in settings).
    #[allow(dead_code)]
    pub fn set_locale(&self, loc: Locale) {
        self.locale.set(loc);
        if let Some(w) = web_sys::window() {
            if let Ok(Some(s)) = w.local_storage() {
                let _ = s.set_item(I18N_LOCALE_STORAGE_KEY, loc.as_str());
            }
            if let Some(doc) = w.document() {
                if let Some(root) = doc.document_element() {
                    let _ = root.set_attribute("lang", loc.as_str());
                }
            }
        }
    }

    /// Closure suitable for Leptos `view!`; tracks locale so UI updates on language change.
    pub fn tr(&self, key: I18nKey) -> impl Fn() -> &'static str {
        let locale_sig = self.locale;
        move || lookup(locale_sig.get(), key)
    }
}

impl Default for I18nService {
    fn default() -> Self {
        Self::new()
    }
}
