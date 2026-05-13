pub const API_URL: &str = "http://localhost:3005";
pub const API_PATH: &str = "/api/";
/// Relativer Pfad unter `join_api_base(API_URL, API_PATH)` zur Better-Auth-Instanz (z. B. Handler auf `/api/auth/*`).
pub const AUTH_PATH_PREFIX: &str = "auth";
/// Für Device Authorization Grant – muss zur Server-Konfiguration (`validateClient` / registrierten Clients) passen.
pub const AUTH_DEVICE_CLIENT_ID: &str = "blxcode-desktop";

/// `localStorage` key for persisted EULA acceptance.
pub const EULA_STORAGE_KEY: &str = "blxcode_eula_v1";

/// `localStorage` key for UI locale (BCP-47, e.g. `de-DE`, `en-US`).
pub const I18N_LOCALE_STORAGE_KEY: &str = "blxcode_locale_v1";

/// `localStorage` key for agent workspace root (tool sandbox).
pub const HARNESS_WORKSPACE_ROOT_KEY: &str = "blxcode_harness_workspace_root_v1";

/// Default URL for embedded browser tab.
pub const HARNESS_BROWSER_DEFAULT_URL: &str = "https://bitslix.com";

/// Schnellwahl auf der „Neuer Tab“-Seite (`(Label, Url)`).
pub const NEW_TAB_BROWSER_SHORTLINKS: &[(&str, &str)] = &[
    ("Bitslix", "https://bitslix.com"),
    ("GitHub", "https://github.com"),
    ("Rust", "https://doc.rust-lang.org"),
];

/// `localStorage` key for embedded browser home URL.
pub const HARNESS_BROWSER_URL_KEY: &str = "blxcode_harness_browser_url_v1";

/// `localStorage` key for persisted device-flow Bearer token.
pub const AUTH_DEVICE_BEARER_KEY: &str = "blxcode_device_bearer_v1";
