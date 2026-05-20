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
