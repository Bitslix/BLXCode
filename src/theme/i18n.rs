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
        "rose-pine" => I18nKey::ThemeNameRosePine,
        "rose-pine-dawn" => I18nKey::ThemeNameRosePineDawn,
        "everforest-dark" => I18nKey::ThemeNameEverforestDark,
        "kanagawa" => I18nKey::ThemeNameKanagawa,
        "github-dark" => I18nKey::ThemeNameGithubDark,
        "night-owl" => I18nKey::ThemeNameNightOwl,
        "ayu-mirage" => I18nKey::ThemeNameAyuMirage,
        "catppuccin-frappe" => I18nKey::ThemeNameCatppuccinFrappe,
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
        "rose-pine" => I18nKey::ThemeDescRosePine,
        "rose-pine-dawn" => I18nKey::ThemeDescRosePineDawn,
        "everforest-dark" => I18nKey::ThemeDescEverforestDark,
        "kanagawa" => I18nKey::ThemeDescKanagawa,
        "github-dark" => I18nKey::ThemeDescGithubDark,
        "night-owl" => I18nKey::ThemeDescNightOwl,
        "ayu-mirage" => I18nKey::ThemeDescAyuMirage,
        "catppuccin-frappe" => I18nKey::ThemeDescCatppuccinFrappe,
        _ => return None,
    })
}
