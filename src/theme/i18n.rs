use crate::i18n::I18nKey;

#[must_use]
pub fn theme_name_key(theme_id: &str) -> Option<I18nKey> {
    Some(match theme_id {
        "blxcode-dark" => I18nKey::ThemeNameBlxcodeDark,
        "blxcode-light" => I18nKey::ThemeNameBlxcodeLight,
        "dracula" => I18nKey::ThemeNameDracula,
        "gruvbox-dark" => I18nKey::ThemeNameGruvboxDark,
        "gruvbox-light" => I18nKey::ThemeNameGruvboxLight,
        "solarized-dark" => I18nKey::ThemeNameSolarizedDark,
        "solarized-light" => I18nKey::ThemeNameSolarizedLight,
        "nord" => I18nKey::ThemeNameNord,
        "one-dark" => I18nKey::ThemeNameOneDark,
        "catppuccin-mocha" => I18nKey::ThemeNameCatppuccinMocha,
        "catppuccin-latte" => I18nKey::ThemeNameCatppuccinLatte,
        "tokyo-night" => I18nKey::ThemeNameTokyoNight,
        _ => return None,
    })
}

#[must_use]
pub fn theme_desc_key(theme_id: &str) -> Option<I18nKey> {
    Some(match theme_id {
        "blxcode-dark" => I18nKey::ThemeDescBlxcodeDark,
        "blxcode-light" => I18nKey::ThemeDescBlxcodeLight,
        "dracula" => I18nKey::ThemeDescDracula,
        "gruvbox-dark" => I18nKey::ThemeDescGruvboxDark,
        "gruvbox-light" => I18nKey::ThemeDescGruvboxLight,
        "solarized-dark" => I18nKey::ThemeDescSolarizedDark,
        "solarized-light" => I18nKey::ThemeDescSolarizedLight,
        "nord" => I18nKey::ThemeDescNord,
        "one-dark" => I18nKey::ThemeDescOneDark,
        "catppuccin-mocha" => I18nKey::ThemeDescCatppuccinMocha,
        "catppuccin-latte" => I18nKey::ThemeDescCatppuccinLatte,
        "tokyo-night" => I18nKey::ThemeDescTokyoNight,
        _ => return None,
    })
}
