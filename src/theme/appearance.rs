//! User-tunable, theme-independent appearance knobs: corner roundings scale,
//! font family, and font size. These are applied as inline custom properties on
//! `<html>` and persisted in `localStorage`.

/// Global corner-roundings multiplier applied to every `--radius-*` token.
/// Pills and circles are not affected (they use fixed tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadiusScale {
    /// Square corners (`×0`).
    Sharp,
    /// Default look (`×1`).
    Default,
    /// Softer corners (`×1.5`).
    Rounded,
    /// Pronounced corners (`×2`).
    Extra,
}

impl RadiusScale {
    /// Stable storage / DOM token (matches `from_storage`).
    #[must_use]
    pub fn storage_value(self) -> &'static str {
        match self {
            RadiusScale::Sharp => "sharp",
            RadiusScale::Default => "default",
            RadiusScale::Rounded => "rounded",
            RadiusScale::Extra => "extra",
        }
    }

    /// The numeric multiplier written to `--radius-scale`.
    #[must_use]
    pub fn multiplier(self) -> &'static str {
        match self {
            RadiusScale::Sharp => "0",
            RadiusScale::Default => "1",
            RadiusScale::Rounded => "1.5",
            RadiusScale::Extra => "2",
        }
    }

    /// Parse a persisted value, falling back to [`RadiusScale::Default`].
    #[must_use]
    pub fn from_storage(value: Option<&str>) -> Self {
        match value {
            Some("sharp") => RadiusScale::Sharp,
            Some("rounded") => RadiusScale::Rounded,
            Some("extra") => RadiusScale::Extra,
            _ => RadiusScale::Default,
        }
    }
}

impl Default for RadiusScale {
    fn default() -> Self {
        RadiusScale::Default
    }
}

/// A curated font choice. `stack` is the full CSS `font-family` value written to
/// `--font-mono`, so a missing primary face degrades cleanly to the fallbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontChoice {
    /// Stable id used for storage and as the radio value.
    pub id: &'static str,
    /// Human-facing display name (also the preview label).
    pub label: &'static str,
    /// Full `font-family` stack written to `--font-mono`.
    pub stack: &'static str,
}

/// Shared monospace fallback tail appended to every choice.
const MONO_TAIL: &str = "ui-monospace, monospace";

/// The default font id (JetBrains Mono is bundled via `@font-face`).
pub const DEFAULT_FONT_ID: &str = "jetbrains-mono";

/// Default global interface / terminal font size in pixels.
pub const DEFAULT_FONT_SIZE_PX: u8 = 15;

/// Smallest supported global font size in pixels.
pub const MIN_FONT_SIZE_PX: u8 = 12;

/// Largest supported global font size in pixels.
pub const MAX_FONT_SIZE_PX: u8 = 20;

/// Clamp a user-provided font size to the supported range.
#[must_use]
pub fn clamp_font_size_px(size: u8) -> u8 {
    size.clamp(MIN_FONT_SIZE_PX, MAX_FONT_SIZE_PX)
}

/// Curated font catalog. Only JetBrains Mono ships with the app; the others are
/// used when present on the system and otherwise fall through the stack.
pub const FONTS: &[FontChoice] = &[
    FontChoice {
        id: "jetbrains-mono",
        label: "JetBrains Mono",
        stack: "\"JetBrains Mono\", \"Cascadia Mono\", Consolas, \"SF Mono\", Menlo, ui-monospace, monospace",
    },
    FontChoice {
        id: "cascadia-code",
        label: "Cascadia Code",
        stack: "\"Cascadia Code\", \"Cascadia Mono\", Consolas, ui-monospace, monospace",
    },
    FontChoice {
        id: "fira-code",
        label: "Fira Code",
        stack: "\"Fira Code\", \"JetBrains Mono\", Menlo, ui-monospace, monospace",
    },
    FontChoice {
        id: "sf-mono",
        label: "SF Mono",
        stack: "\"SF Mono\", \"JetBrains Mono\", Menlo, Consolas, ui-monospace, monospace",
    },
    FontChoice {
        id: "menlo",
        label: "Menlo",
        stack: "Menlo, \"JetBrains Mono\", Consolas, ui-monospace, monospace",
    },
    FontChoice {
        id: "consolas",
        label: "Consolas",
        stack: "Consolas, \"Cascadia Mono\", \"JetBrains Mono\", ui-monospace, monospace",
    },
    FontChoice {
        id: "system-mono",
        label: "System Monospace",
        stack: MONO_TAIL,
    },
];

/// Look up a font by id.
#[must_use]
pub fn font_by_id(id: &str) -> Option<&'static FontChoice> {
    FONTS.iter().find(|f| f.id == id)
}

/// The font stack for a (possibly persisted) id, falling back to the default.
#[must_use]
pub fn font_stack_for(id: &str) -> &'static str {
    font_by_id(id)
        .or_else(|| font_by_id(DEFAULT_FONT_ID))
        .map(|f| f.stack)
        .unwrap_or(MONO_TAIL)
}

/// Whether `id` names a known font.
#[must_use]
pub fn is_valid_font_id(id: &str) -> bool {
    font_by_id(id).is_some()
}
