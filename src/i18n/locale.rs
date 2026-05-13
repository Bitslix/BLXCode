/// Supported UI locales (BCP-47 subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    DeDe,
    EnUs,
}

impl Locale {
    /// BCP-47 tag persisted in storage and mirrored on `<html lang>`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeDe => "de-DE",
            Self::EnUs => "en-US",
        }
    }

    /// Parse exact `de-DE` / `en-US` tags (case-insensitive on prefix).
    #[must_use]
    pub fn parse_bcp47(raw: &str) -> Option<Self> {
        let s = raw.trim();
        let lower = s.to_ascii_lowercase();
        match lower.as_str() {
            "de-de" | "de" => Some(Self::DeDe),
            "en-us" | "en-gb" | "en" => Some(Self::EnUs),
            _ => None,
        }
    }

    #[must_use]
    pub fn infer_from_browser_lang(lang: &str) -> Self {
        if let Some(l) = Self::parse_bcp47(lang) {
            return l;
        }
        let lower = lang.to_ascii_lowercase();
        // Prefix match for e.g. "de-AT", "en-IN"
        if lower.starts_with("de") {
            return Self::DeDe;
        }
        if lower.starts_with("en") {
            return Self::EnUs;
        }
        Self::default()
    }
}
