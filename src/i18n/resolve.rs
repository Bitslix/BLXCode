use crate::i18n::keys::I18nKey;
use crate::i18n::locale::Locale;
use crate::i18n::locales::{de_de, en_us};

/// Resolves a static string for the given locale and key.
#[must_use]
pub fn lookup(locale: Locale, key: I18nKey) -> &'static str {
    match locale {
        Locale::DeDe => de_de::msg(key),
        Locale::EnUs => en_us::msg(key),
    }
}
