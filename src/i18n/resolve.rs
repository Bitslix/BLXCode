use crate::i18n::keys::I18nKey;
use crate::i18n::locale::Locale;
use crate::i18n::locales::{
    de_de, en_us, es_es, fr_fr, it_it, ja_jp, ko_kr, pl_pl, pt_br, ru_ru, zh_cn, zh_tw,
};

/// Resolves a static string for the given locale and key.
#[must_use]
pub fn lookup(locale: Locale, key: I18nKey) -> &'static str {
    match locale {
        Locale::DeDe => de_de::msg(key),
        Locale::EnUs => en_us::msg(key),
        Locale::EsEs => es_es::msg(key),
        Locale::FrFr => fr_fr::msg(key),
        Locale::ItIt => it_it::msg(key),
        Locale::JaJp => ja_jp::msg(key),
        Locale::KoKr => ko_kr::msg(key),
        Locale::PlPl => pl_pl::msg(key),
        Locale::PtBr => pt_br::msg(key),
        Locale::RuRu => ru_ru::msg(key),
        Locale::ZhCn => zh_cn::msg(key),
        Locale::ZhTw => zh_tw::msg(key),
    }
}
