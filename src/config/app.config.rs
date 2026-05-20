/// `localStorage` key for persisted EULA acceptance.
pub const EULA_STORAGE_KEY: &str = "blxcode_eula_v2";

/// `localStorage` key for the one-time ".blxcode in .gitignore" prompt (`yes` / `no`).
pub const GITIGNORE_PROMPT_STORAGE_KEY: &str = "blxcode_gitignore_prompt_v1";

/// Stored when the user completed the gitignore prompt (`yes` or `no`).
pub const GITIGNORE_PROMPT_ANSWER_YES: &str = "yes";
pub const GITIGNORE_PROMPT_ANSWER_NO: &str = "no";

/// `localStorage` key for UI locale (BCP-47, e.g. `de-DE`, `en-US`).
pub const I18N_LOCALE_STORAGE_KEY: &str = "blxcode_locale_v1";

/// `localStorage` key for memory graph rendering mode (`2d` / `3d`).
pub const GRAPH_MODE_STORAGE_KEY: &str = "blxcode_memory_graph_mode_v1";

/// `localStorage` key for reusable Memory category color presets.
pub const MEMORY_COLOR_PRESETS_STORAGE_KEY: &str = "blxcode_memory_color_presets_v1";

/// `localStorage` key for agent workspace root (tool sandbox).
pub const HARNESS_WORKSPACE_ROOT_KEY: &str = "blxcode_harness_workspace_root_v1";

/// Default URL for embedded browser tab.
pub const HARNESS_BROWSER_DEFAULT_URL: &str = "https://blxcode.com";

/// Schnellwahl auf der „Neuer Tab"-Seite (`(Label, Url)`).
pub const NEW_TAB_BROWSER_SHORTLINKS: &[(&str, &str)] = &[
    ("BLXCode", "https://blxcode.com"),
    ("GitHub", "https://github.com"),
    ("Rust", "https://doc.rust-lang.org"),
];

/// `localStorage` key for embedded browser home URL.
pub const HARNESS_BROWSER_URL_KEY: &str = "blxcode_harness_browser_url_v1";

/// `localStorage` key for success action toasts (`1` / `0`).
pub const SUCCESS_TOAST_STORAGE_KEY: &str = "blxcode_success_toast_v1";

/// `localStorage` key for success action sounds (`1` / `0`).
pub const SUCCESS_SOUND_STORAGE_KEY: &str = "blxcode_success_sound_v1";

/// `localStorage` key for keyboard shortcut mode (`tmux` / `legacy`).
pub const SHORTCUT_MODE_STORAGE_KEY: &str = "blxcode_shortcut_mode_v1";

/// Stored value when tmux-style prefix chords are enabled (default).
pub const SHORTCUT_MODE_TMUX: &str = "tmux";

/// Stored value for direct Ctrl/Ctrl+Shift shortcuts.
pub const SHORTCUT_MODE_LEGACY: &str = "legacy";

/// `localStorage` key for the combined explorer+graph panel height (percent of sidebar).
pub const SIDEBAR_PANELS_HEIGHT_PCT_KEY: &str = "blxcode_sidebar_panels_height_pct_v1";

/// Default height of the explorer+graph block within the sidebar (percent).
pub const SIDEBAR_PANELS_HEIGHT_PCT_DEFAULT: f64 = 50.0;

pub const SIDEBAR_PANELS_HEIGHT_PCT_MIN: f64 = 25.0;
pub const SIDEBAR_PANELS_HEIGHT_PCT_MAX: f64 = 75.0;

/// `localStorage` key for the sidebar Project Explorer slot height (percent of panels block).
pub const SIDEBAR_EXPLORER_HEIGHT_PCT_KEY: &str = "blxcode_sidebar_explorer_height_pct_v1";

/// Default Project Explorer slot height (percent of the panels block).
pub const SIDEBAR_EXPLORER_HEIGHT_PCT_DEFAULT: f64 = 50.0;

/// Min/max clamp range for the Project Explorer slot height (percent of panels block).
pub const SIDEBAR_EXPLORER_HEIGHT_PCT_MIN: f64 = 15.0;
pub const SIDEBAR_EXPLORER_HEIGHT_PCT_MAX: f64 = 85.0;

/// `localStorage` key for sidebar width in pixels.
pub const SIDEBAR_WIDTH_PX_KEY: &str = "blxcode_sidebar_width_px_v1";

/// Default sidebar width when expanded.
pub const SIDEBAR_WIDTH_PX_DEFAULT: f64 = 260.0;

pub const SIDEBAR_WIDTH_PX_MIN: f64 = 200.0;

/// `localStorage` key for Plans panel plan-list column width (pixels).
pub const PLANS_LIST_WIDTH_PX_KEY: &str = "blxcode_plans_list_width_px_v1";

/// Default width of the plan list column in the Plans panel.
pub const PLANS_LIST_WIDTH_PX_DEFAULT: f64 = 320.0;

pub const PLANS_LIST_WIDTH_PX_MIN: f64 = 200.0;

pub const PLANS_LIST_WIDTH_PX_MAX: f64 = 520.0;

/// `localStorage` key for showing dot-hidden files in the project explorer (`1` / `0`).
pub const SIDEBAR_EXPLORER_SHOW_HIDDEN_KEY: &str = "blxcode_sidebar_explorer_show_hidden_v1";
