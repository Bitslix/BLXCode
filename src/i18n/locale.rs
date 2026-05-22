/// Supported UI locales (BCP-47 subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Locale {
    #[default]
    EnUs,
    DeDe,
    EsEs,
    FrFr,
    HuHu,
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
            Self::HuHu => "hu-HU",
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

    /// ISO-639-1 primary language code, used as the STT `language` hint.
    #[must_use]
    pub fn iso639_1(self) -> &'static str {
        match self {
            Self::DeDe => "de",
            Self::EnUs => "en",
            Self::EsEs => "es",
            Self::FrFr => "fr",
            Self::HuHu => "hu",
            Self::ItIt => "it",
            Self::JaJp => "ja",
            Self::KoKr => "ko",
            Self::PlPl => "pl",
            Self::PtBr => "pt",
            Self::RuRu => "ru",
            Self::ZhCn => "zh",
            Self::ZhTw => "zh",
        }
    }

    /// SVG flag path (bundled under `public/flags`, copied by Trunk into `dist`) for `<img src>`.
    #[must_use]
    pub fn flag_icon_url(self) -> &'static str {
        macro_rules! fi {
            ($cc:literal) => {
                concat!("/public/flags/", $cc, ".svg")
            };
        }
        match self {
            Self::DeDe => fi!("de"),
            Self::EnUs => fi!("us"),
            Self::EsEs => fi!("es"),
            Self::FrFr => fi!("fr"),
            Self::HuHu => fi!("hu"),
            Self::ItIt => fi!("it"),
            Self::JaJp => fi!("jp"),
            Self::KoKr => fi!("kr"),
            Self::PlPl => fi!("pl"),
            Self::PtBr => fi!("br"),
            Self::RuRu => fi!("ru"),
            Self::ZhCn => fi!("cn"),
            Self::ZhTw => fi!("tw"),
        }
    }

    /// Map a stored STT manual code (ISO-639-1 or BCP-47) to a built-in UI locale.
    #[must_use]
    pub fn from_iso639_1(code: &str) -> Self {
        let trimmed = code.trim();
        if trimmed.is_empty() {
            return Self::default();
        }
        if let Some(loc) = Self::parse_bcp47(trimmed) {
            return loc;
        }
        let lower = trimmed.to_ascii_lowercase();
        for (loc, _) in APP_LOCALES {
            if loc.iso639_1() == lower {
                return *loc;
            }
        }
        Self::default()
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
            "hu-hu" | "hu" => Some(Self::HuHu),
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
        if lower.starts_with("hu") {
            return Self::HuHu;
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
    (Locale::HuHu, "Magyar"),
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
        assert_eq!(
            Locale::infer_from_browser_lang("zh-XX-unknown"),
            Locale::EnUs
        );
    }

    #[test]
    fn parse_bcp47_hungarian() {
        assert_eq!(Locale::parse_bcp47("hu-HU"), Some(Locale::HuHu));
        assert_eq!(Locale::parse_bcp47("hu"), Some(Locale::HuHu));
    }

    #[test]
    fn from_iso639_1_maps_builtin_codes() {
        assert_eq!(Locale::from_iso639_1("de"), Locale::DeDe);
        assert_eq!(Locale::from_iso639_1("ja"), Locale::JaJp);
        assert_eq!(Locale::from_iso639_1("zh-TW"), Locale::ZhTw);
    }

    #[test]
    fn default_is_english() {
        assert_eq!(Locale::default(), Locale::EnUs);
    }
}
