#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

/// Colors for the mini preview mockup on theme cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePreviewColors {
    pub sidebar: &'static str,
    pub background: &'static str,
    pub accent: &'static str,
    pub text: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTheme {
    pub id: &'static str,
    pub mode: ThemeMode,
    pub preview: ThemePreviewColors,
}

pub const DEFAULT_THEME_ID: &str = "blxcode-dark";

pub const THEMES: &[AppTheme] = &[
    AppTheme {
        id: "blxcode-dark",
        mode: ThemeMode::Dark,
        preview: ThemePreviewColors {
            sidebar: "#101116",
            background: "#15171d",
            accent: "#58a6ff",
            text: "#f1f2f5",
        },
    },
    AppTheme {
        id: "blxcode-light",
        mode: ThemeMode::Light,
        preview: ThemePreviewColors {
            sidebar: "#eef0f4",
            background: "#ffffff",
            accent: "#0969da",
            text: "#1a1d24",
        },
    },
    AppTheme {
        id: "dracula",
        mode: ThemeMode::Dark,
        preview: ThemePreviewColors {
            sidebar: "#21222c",
            background: "#282a36",
            accent: "#bd93f9",
            text: "#f8f8f2",
        },
    },
    AppTheme {
        id: "gruvbox-dark",
        mode: ThemeMode::Dark,
        preview: ThemePreviewColors {
            sidebar: "#1d2021",
            background: "#282828",
            accent: "#fe8019",
            text: "#ebdbb2",
        },
    },
    AppTheme {
        id: "gruvbox-light",
        mode: ThemeMode::Light,
        preview: ThemePreviewColors {
            sidebar: "#ebdbb2",
            background: "#fbf1c7",
            accent: "#af3a03",
            text: "#3c3836",
        },
    },
    AppTheme {
        id: "solarized-dark",
        mode: ThemeMode::Dark,
        preview: ThemePreviewColors {
            sidebar: "#00212b",
            background: "#002b36",
            accent: "#268bd2",
            text: "#839496",
        },
    },
    AppTheme {
        id: "solarized-light",
        mode: ThemeMode::Light,
        preview: ThemePreviewColors {
            sidebar: "#eee8d5",
            background: "#fdf6e3",
            accent: "#268bd2",
            text: "#657b83",
        },
    },
    AppTheme {
        id: "nord",
        mode: ThemeMode::Dark,
        preview: ThemePreviewColors {
            sidebar: "#2e3440",
            background: "#3b4252",
            accent: "#88c0d0",
            text: "#eceff4",
        },
    },
    AppTheme {
        id: "one-dark",
        mode: ThemeMode::Dark,
        preview: ThemePreviewColors {
            sidebar: "#21252b",
            background: "#282c34",
            accent: "#61afef",
            text: "#abb2bf",
        },
    },
    AppTheme {
        id: "catppuccin-mocha",
        mode: ThemeMode::Dark,
        preview: ThemePreviewColors {
            sidebar: "#181825",
            background: "#1e1e2e",
            accent: "#cba6f7",
            text: "#cdd6f4",
        },
    },
    AppTheme {
        id: "catppuccin-latte",
        mode: ThemeMode::Light,
        preview: ThemePreviewColors {
            sidebar: "#e6e9ef",
            background: "#eff1f5",
            accent: "#8839ef",
            text: "#4c4f69",
        },
    },
    AppTheme {
        id: "tokyo-night",
        mode: ThemeMode::Dark,
        preview: ThemePreviewColors {
            sidebar: "#16161e",
            background: "#1a1b26",
            accent: "#7aa2f7",
            text: "#c0caf5",
        },
    },
];

#[must_use]
pub fn theme_by_id(id: &str) -> Option<&'static AppTheme> {
    THEMES.iter().find(|t| t.id == id)
}

#[must_use]
pub fn is_valid_theme_id(id: &str) -> bool {
    theme_by_id(id).is_some()
}

#[must_use]
#[allow(dead_code)]
pub fn themes_for_mode(mode: Option<ThemeMode>) -> Vec<AppTheme> {
    match mode {
        None => THEMES.iter().copied().collect(),
        Some(m) => THEMES
            .iter()
            .copied()
            .filter(|t| t.mode == m)
            .collect(),
    }
}
