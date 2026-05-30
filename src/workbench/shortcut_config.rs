//! Data-driven keyboard shortcut configuration.
//!
//! Replaces the previously hard-coded per-mode key tables. A [`ShortcutConfig`]
//! holds a single configurable [`KeyChord`] prefix (used by tmux-style chords)
//! plus one [`Binding`] per [`ShortcutAction`]. The same config feeds both the
//! key matching in `harness_chords` and the on-screen display (welcome screen,
//! settings pane), and is persisted to `localStorage`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use web_sys::KeyboardEvent;

use super::app_prefs::ShortcutMode;
use super::harness_chords::HarnessShortcutAction;
use super::state::RightPanelTab;
use crate::i18n::I18nKey;

/// A single key combination. `ctrl` folds Ctrl and Cmd/Meta together so the
/// same binding works cross-platform (matching the previous `ctrl_or_meta`
/// behaviour). `key` is normalised: single letters are stored lowercase,
/// named keys (`Escape`, `ArrowUp`, `` ` ``) keep their `KeyboardEvent.key`
/// spelling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyChord {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    pub key: String,
}

impl KeyChord {
    #[must_use]
    pub fn new(ctrl: bool, shift: bool, alt: bool, key: &str) -> Self {
        Self {
            ctrl,
            shift,
            alt,
            key: normalize_key(key),
        }
    }

    /// Build a chord from a live key event. Returns `None` for lone modifier
    /// presses (Ctrl/Shift/Alt/Meta on their own), which can never form a
    /// usable binding.
    #[must_use]
    pub fn from_event(ev: &KeyboardEvent) -> Option<Self> {
        let key = ev.key();
        if matches!(key.as_str(), "Control" | "Shift" | "Alt" | "Meta") {
            return None;
        }
        Some(Self {
            ctrl: ev.ctrl_key() || ev.meta_key(),
            shift: ev.shift_key(),
            alt: ev.alt_key(),
            key: normalize_key(&key),
        })
    }

    /// True when `ev` matches this chord exactly (modifiers + key).
    #[must_use]
    pub fn matches(&self, ev: &KeyboardEvent) -> bool {
        self.ctrl == (ev.ctrl_key() || ev.meta_key())
            && self.shift == ev.shift_key()
            && self.alt == ev.alt_key()
            && self.key == normalize_key(&ev.key())
    }

    /// Display segments, e.g. `["Ctrl", "Shift", "N"]`.
    #[must_use]
    pub fn parts(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.ctrl {
            out.push("Ctrl".to_owned());
        }
        if self.shift {
            out.push("Shift".to_owned());
        }
        if self.alt {
            out.push("Alt".to_owned());
        }
        out.push(display_key(&self.key));
        out
    }
}

/// How a single action is triggered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Binding {
    /// Direct combination (classic style), e.g. `Ctrl+Shift+N`.
    Combo(KeyChord),
    /// Prefix-then-key (tmux style); the prefix is the shared
    /// [`ShortcutConfig::prefix`]. `second` is a normalised bare key.
    Chord { second: String },
}

impl Binding {
    /// Human-readable rendering. `then_word` is the localized separator used
    /// between the prefix and the second key (e.g. "then").
    #[must_use]
    pub fn display(&self, prefix: &KeyChord, then_word: &str) -> String {
        match self {
            Self::Combo(chord) => chord.parts().join(" + "),
            Self::Chord { second } => {
                format!("{} {} {}", prefix.parts().join(" + "), then_word, display_key(second))
            }
        }
    }
}

/// The bindable harness actions (the 7 rows shown on the welcome screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ShortcutAction {
    QuickOpen,
    SidePanel,
    Agent,
    Browser,
    Memory,
    Terminal,
    CommandPalette,
}

impl ShortcutAction {
    /// Stable iteration order (mirrors the welcome-screen layout).
    pub const ALL: [Self; 7] = [
        Self::QuickOpen,
        Self::SidePanel,
        Self::Agent,
        Self::Browser,
        Self::Memory,
        Self::Terminal,
        Self::CommandPalette,
    ];

    #[must_use]
    pub fn label_key(self) -> I18nKey {
        match self {
            Self::QuickOpen => I18nKey::WsKwQuickOpen,
            Self::SidePanel => I18nKey::WsKwSidePanel,
            Self::Agent => I18nKey::WsKwAgent,
            Self::Browser => I18nKey::WsKwBrowser,
            Self::Memory => I18nKey::WsKwMemory,
            Self::Terminal => I18nKey::WsKwTerminal,
            Self::CommandPalette => I18nKey::WsKwCmdPalette,
        }
    }

    #[must_use]
    pub fn to_harness_action(self) -> HarnessShortcutAction {
        match self {
            Self::QuickOpen => HarnessShortcutAction::OpenQuickOpen,
            Self::SidePanel => HarnessShortcutAction::ToggleRightPanel,
            Self::Agent => HarnessShortcutAction::RightTab(RightPanelTab::Agent),
            Self::Browser => HarnessShortcutAction::RightTab(RightPanelTab::Browser),
            Self::Memory => HarnessShortcutAction::RightTab(RightPanelTab::Memory),
            Self::Terminal => HarnessShortcutAction::OpenNewTerminal,
            Self::CommandPalette => HarnessShortcutAction::ToggleCommandPalette,
        }
    }

    /// Default tmux second key.
    #[must_use]
    fn default_second(self) -> &'static str {
        match self {
            Self::QuickOpen => "o",
            Self::SidePanel => "r",
            Self::Agent => "a",
            Self::Browser => "b",
            Self::Memory => "m",
            Self::Terminal => "n",
            Self::CommandPalette => "p",
        }
    }

    /// Default classic (direct) combination.
    #[must_use]
    fn default_combo(self) -> KeyChord {
        match self {
            Self::QuickOpen => KeyChord::new(true, false, false, "o"),
            Self::SidePanel => KeyChord::new(true, false, false, "p"),
            Self::Agent => KeyChord::new(true, true, false, "a"),
            Self::Browser => KeyChord::new(true, true, false, "b"),
            Self::Memory => KeyChord::new(true, true, false, "m"),
            Self::Terminal => KeyChord::new(true, true, false, "n"),
            Self::CommandPalette => KeyChord::new(true, true, false, "p"),
        }
    }
}

/// The full configurable keymap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub prefix: KeyChord,
    pub bindings: BTreeMap<ShortcutAction, Binding>,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self::preset(ShortcutMode::Tmux)
    }
}

impl ShortcutConfig {
    /// The default prefix (`Ctrl+B`).
    #[must_use]
    pub fn default_prefix() -> KeyChord {
        KeyChord::new(true, false, false, "b")
    }

    /// Build the stock config for a preset mode.
    #[must_use]
    pub fn preset(mode: ShortcutMode) -> Self {
        let bindings = ShortcutAction::ALL
            .into_iter()
            .map(|action| {
                let binding = match mode {
                    ShortcutMode::Tmux => Binding::Chord {
                        second: action.default_second().to_owned(),
                    },
                    ShortcutMode::Legacy => Binding::Combo(action.default_combo()),
                };
                (action, binding)
            })
            .collect();
        Self {
            prefix: Self::default_prefix(),
            bindings,
        }
    }

    /// The binding for `action`, falling back to the tmux default if (somehow)
    /// missing from the map.
    #[must_use]
    pub fn binding(&self, action: ShortcutAction) -> Binding {
        self.bindings.get(&action).cloned().unwrap_or(Binding::Chord {
            second: action.default_second().to_owned(),
        })
    }

    /// Find the action bound as a tmux *chord* whose second key matches `ev`
    /// (modifiers on the second key are ignored, matching tmux semantics).
    #[must_use]
    pub fn chord_match(&self, ev: &KeyboardEvent) -> Option<ShortcutAction> {
        let key = normalize_key(&ev.key());
        self.bindings.iter().find_map(|(action, binding)| match binding {
            Binding::Chord { second } if *second == key => Some(*action),
            _ => None,
        })
    }

    /// Find the action bound as a direct *combo* matching `ev` exactly.
    #[must_use]
    pub fn combo_match(&self, ev: &KeyboardEvent) -> Option<ShortcutAction> {
        self.bindings.iter().find_map(|(action, binding)| match binding {
            Binding::Combo(chord) if chord.matches(ev) => Some(*action),
            _ => None,
        })
    }

    /// Actions that collide with `action`'s current binding (same combo, or
    /// same tmux second key). Used to surface conflict warnings in the UI.
    #[must_use]
    pub fn conflicts(&self, action: ShortcutAction) -> Vec<ShortcutAction> {
        let target = self.binding(action);
        ShortcutAction::ALL
            .into_iter()
            .filter(|&other| other != action && self.binding(other) == target)
            .collect()
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    #[must_use]
    pub fn from_json(raw: &str) -> Option<Self> {
        serde_json::from_str(raw).ok()
    }
}

/// Normalise a `KeyboardEvent.key` for storage/comparison: single letters are
/// lowercased; everything else is kept verbatim.
fn normalize_key(key: &str) -> String {
    if key.chars().count() == 1 {
        key.to_lowercase()
    } else {
        key.to_owned()
    }
}

/// Render a stored key for display: single letters uppercased, the space key
/// spelled out, everything else verbatim.
fn display_key(key: &str) -> String {
    match key {
        " " => "Space".to_owned(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => k.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_cover_all_actions() {
        for mode in [ShortcutMode::Tmux, ShortcutMode::Legacy] {
            let cfg = ShortcutConfig::preset(mode);
            for action in ShortcutAction::ALL {
                assert!(cfg.bindings.contains_key(&action), "{action:?} missing");
            }
        }
    }

    #[test]
    fn tmux_preset_uses_chords_with_default_prefix() {
        let cfg = ShortcutConfig::preset(ShortcutMode::Tmux);
        assert_eq!(cfg.prefix, KeyChord::new(true, false, false, "b"));
        assert_eq!(
            cfg.binding(ShortcutAction::Terminal),
            Binding::Chord { second: "n".to_owned() }
        );
    }

    #[test]
    fn legacy_preset_uses_combos() {
        let cfg = ShortcutConfig::preset(ShortcutMode::Legacy);
        assert_eq!(
            cfg.binding(ShortcutAction::Terminal),
            Binding::Combo(KeyChord::new(true, true, false, "n"))
        );
    }

    #[test]
    fn display_renders_combo_and_chord() {
        let prefix = KeyChord::new(true, false, false, "b");
        assert_eq!(
            Binding::Combo(KeyChord::new(true, true, false, "n")).display(&prefix, "then"),
            "Ctrl + Shift + N"
        );
        assert_eq!(
            Binding::Chord { second: "n".to_owned() }.display(&prefix, "then"),
            "Ctrl + B then N"
        );
    }

    #[test]
    fn json_roundtrip_preserves_config() {
        let cfg = ShortcutConfig::preset(ShortcutMode::Legacy);
        let restored = ShortcutConfig::from_json(&cfg.to_json()).expect("parse");
        assert_eq!(cfg, restored);
    }

    #[test]
    fn conflicts_detects_duplicate_binding() {
        let mut cfg = ShortcutConfig::preset(ShortcutMode::Legacy);
        let dup = cfg.binding(ShortcutAction::QuickOpen);
        cfg.bindings.insert(ShortcutAction::Terminal, dup);
        assert!(cfg.conflicts(ShortcutAction::Terminal).contains(&ShortcutAction::QuickOpen));
        assert!(cfg.conflicts(ShortcutAction::Agent).is_empty());
    }

    #[test]
    fn key_normalisation_lowercases_single_letters_only() {
        assert_eq!(normalize_key("N"), "n");
        assert_eq!(normalize_key("Escape"), "Escape");
        assert_eq!(normalize_key("`"), "`");
    }
}
