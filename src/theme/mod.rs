mod catalog;
mod i18n;

pub use catalog::{is_valid_theme_id, AppTheme, ThemeMode, DEFAULT_THEME_ID, THEMES};
pub use i18n::{theme_desc_key, theme_name_key};
