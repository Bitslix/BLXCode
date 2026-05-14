/// Supported UI locales (BCP-47 subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Locale {
    #[default]
    EnUs,
    DeDe,
    EsEs,
    FrFr,
    ItIt,
    JaJp,
    KoKr,
    PlPl,
    PtBr,
    RuRu,
    ZhCn,
    ZhTw,
}

impl Locale {
    /// BCP-47 tag persisted in storage and mirrored on `<html lang>`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeDe => "de-DE",
            Self::EnUs => "en-US",
            Self::EsEs => "es-ES",
            Self::FrFr => "fr-FR",
            Self::ItIt => "it-IT",
            Self::JaJp => "ja-JP",
            Self::KoKr => "ko-KR",
            Self::PlPl => "pl-PL",
            Self::PtBr => "pt-BR",
            Self::RuRu => "ru-RU",
            Self::ZhCn => "zh-CN",
            Self::ZhTw => "zh-TW",
        }
    }

    /// Parse persisted or `<option>` BCP-47 tags (ASCII case-insensitive).
    #[must_use]
    pub fn parse_bcp47(raw: &str) -> Option<Self> {
        let s = raw.trim();
        let lower = s.to_ascii_lowercase();
        match lower.as_str() {
            "de-de" | "de" => Some(Self::DeDe),
            "en-us" | "en-gb" | "en" => Some(Self::EnUs),
            "es-es" | "es-mx" | "es-419" | "es" => Some(Self::EsEs),
            "fr-fr" | "fr-ca" | "fr" => Some(Self::FrFr),
            "it-it" | "it" => Some(Self::ItIt),
            "ja-jp" | "ja" => Some(Self::JaJp),
            "ko-kr" | "ko" => Some(Self::KoKr),
            "pl-pl" | "pl" => Some(Self::PlPl),
            "pt-br" | "pt-pt" | "pt" => Some(Self::PtBr),
            "ru-ru" | "ru" => Some(Self::RuRu),
            "zh-cn" | "zh-hans" | "zh-sg" | "zh-hans-cn" => Some(Self::ZhCn),
            "zh-tw" | "zh-hk" | "zh-mo" | "zh-hant" => Some(Self::ZhTw),
            "zh" => Some(Self::ZhCn),
            _ => None,
        }
    }

    #[must_use]
    pub fn infer_from_browser_lang(lang: &str) -> Self {
        if let Some(l) = Self::parse_bcp47(lang) {
            return l;
        }
        let lower = lang.to_ascii_lowercase();
        if lower.contains("hant")
            || lower.starts_with("zh-tw")
            || lower.starts_with("zh-hk")
            || lower.starts_with("zh-mo")
        {
            return Self::ZhTw;
        }
        if lower.contains("hans")
            || lower.starts_with("zh-cn")
            || lower.starts_with("zh-sg")
            || lower == "zh"
        {
            return Self::ZhCn;
        }
        if lower.starts_with("zh-") {
            return Self::EnUs;
        }
        if lower.starts_with("es") {
            return Self::EsEs;
        }
        if lower.starts_with("fr") {
            return Self::FrFr;
        }
        if lower.starts_with("it") {
            return Self::ItIt;
        }
        if lower.starts_with("ja") {
            return Self::JaJp;
        }
        if lower.starts_with("ko") {
            return Self::KoKr;
        }
        if lower.starts_with("pl") {
            return Self::PlPl;
        }
        if lower.starts_with("pt") {
            return Self::PtBr;
        }
        if lower.starts_with("ru") {
            return Self::RuRu;
        }
        if lower.starts_with("de") {
            return Self::DeDe;
        }
        if lower.starts_with("en") {
            return Self::EnUs;
        }
        Self::default()
    }
}

/// All selectable UI locales: `(Locale, native label)`. HTML `value` = `locale.as_str()` only.
pub const APP_LOCALES: &[(Locale, &str)] = &[
    (Locale::DeDe, "Deutsch"),
    (Locale::EnUs, "English"),
    (Locale::EsEs, "Español"),
    (Locale::FrFr, "Français"),
    (Locale::ItIt, "Italiano"),
    (Locale::JaJp, "日本語"),
    (Locale::KoKr, "한국어"),
    (Locale::PlPl, "Polski"),
    (Locale::PtBr, "Português (Brasil)"),
    (Locale::RuRu, "Русский"),
    (Locale::ZhCn, "简体中文"),
    (Locale::ZhTw, "繁體中文"),
];

#[cfg(test)]
mod tests {
    use super::Locale;

    #[test]
    fn parse_bcp47_spanish_variants() {
        assert_eq!(Locale::parse_bcp47("es-MX"), Some(Locale::EsEs));
        assert_eq!(Locale::parse_bcp47("es-419"), Some(Locale::EsEs));
    }

    #[test]
    fn parse_bcp47_portuguese() {
        assert_eq!(Locale::parse_bcp47("pt-PT"), Some(Locale::PtBr));
    }

    #[test]
    fn parse_bcp47_chinese() {
        assert_eq!(Locale::parse_bcp47("zh-Hans-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::parse_bcp47("zh-TW"), Some(Locale::ZhTw));
        assert_eq!(Locale::parse_bcp47("zh-hant"), Some(Locale::ZhTw));
    }

    #[test]
    fn infer_ambiguous_zh() {
        assert_eq!(Locale::infer_from_browser_lang("zh-XX-unknown"), Locale::EnUs);
    }

    #[test]
    fn default_is_english() {
        assert_eq!(Locale::default(), Locale::EnUs);
    }
}
