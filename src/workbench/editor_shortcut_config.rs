//! Configurable key bindings for the in-app file editor / preview.
//!
//! Distinct from [`super::shortcut_config::ShortcutAction`] (global harness
//! actions): these only fire while a CodeMirror editor is focused and are
//! dispatched through the editor's own keymap, not the global key handler.
//! They are always direct combos (never tmux chords) and are **disabled while
//! Vim mode is active**, since Vim owns the keymap.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::shortcut_config::KeyChord;
use crate::i18n::I18nKey;

/// An editor command that can be bound to a key. The string [`Self::id`] is the
/// command name handed to the CodeMirror bundle, which maps it to a concrete
/// CM6 command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EditorShortcutAction {
    Save,
    Find,
    Replace,
    GoToLine,
    ToggleComment,
    Fold,
    Unfold,
    MoveLineUp,
    MoveLineDown,
    DuplicateLine,
    Format,
}

impl EditorShortcutAction {
    /// Stable iteration / display order.
    pub const ALL: [Self; 11] = [
        Self::Save,
        Self::Find,
        Self::Replace,
        Self::GoToLine,
        Self::ToggleComment,
        Self::Fold,
        Self::Unfold,
        Self::MoveLineUp,
        Self::MoveLineDown,
        Self::DuplicateLine,
        Self::Format,
    ];

    /// Command id passed to the CodeMirror bundle keymap builder.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Save => "save",
            Self::Find => "find",
            Self::Replace => "replace",
            Self::GoToLine => "gotoLine",
            Self::ToggleComment => "toggleComment",
            Self::Fold => "fold",
            Self::Unfold => "unfold",
            Self::MoveLineUp => "moveLineUp",
            Self::MoveLineDown => "moveLineDown",
            Self::DuplicateLine => "duplicateLine",
            Self::Format => "format",
        }
    }

    #[must_use]
    pub const fn label_key(self) -> I18nKey {
        match self {
            Self::Save => I18nKey::EdKwSave,
            Self::Find => I18nKey::EdKwFind,
            Self::Replace => I18nKey::EdKwReplace,
            Self::GoToLine => I18nKey::EdKwGoToLine,
            Self::ToggleComment => I18nKey::EdKwToggleComment,
            Self::Fold => I18nKey::EdKwFold,
            Self::Unfold => I18nKey::EdKwUnfold,
            Self::MoveLineUp => I18nKey::EdKwMoveLineUp,
            Self::MoveLineDown => I18nKey::EdKwMoveLineDown,
            Self::DuplicateLine => I18nKey::EdKwDuplicateLine,
            Self::Format => I18nKey::EdKwFormat,
        }
    }

    /// Default key combination. `ctrl` maps to Ctrl-or-Cmd cross-platform.
    #[must_use]
    pub fn default_combo(self) -> KeyChord {
        match self {
            Self::Save => KeyChord::new(true, false, false, "s"),
            Self::Find => KeyChord::new(true, false, false, "f"),
            Self::Replace => KeyChord::new(true, false, true, "f"),
            Self::GoToLine => KeyChord::new(true, false, false, "g"),
            Self::ToggleComment => KeyChord::new(true, false, false, "/"),
            Self::Fold => KeyChord::new(true, true, false, "["),
            Self::Unfold => KeyChord::new(true, true, false, "]"),
            Self::MoveLineUp => KeyChord::new(false, false, true, "ArrowUp"),
            Self::MoveLineDown => KeyChord::new(false, false, true, "ArrowDown"),
            Self::DuplicateLine => KeyChord::new(true, true, false, "d"),
            Self::Format => KeyChord::new(true, false, true, "l"),
        }
    }
}

/// The full editor keymap: one [`KeyChord`] per [`EditorShortcutAction`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorShortcutConfig {
    pub bindings: BTreeMap<EditorShortcutAction, KeyChord>,
}

impl Default for EditorShortcutConfig {
    fn default() -> Self {
        Self::preset()
    }
}

impl EditorShortcutConfig {
    /// Stock defaults.
    #[must_use]
    pub fn preset() -> Self {
        Self {
            bindings: EditorShortcutAction::ALL
                .into_iter()
                .map(|action| (action, action.default_combo()))
                .collect(),
        }
    }

    /// The binding for `action`, falling back to its default if missing.
    #[must_use]
    pub fn binding(&self, action: EditorShortcutAction) -> KeyChord {
        self.bindings
            .get(&action)
            .cloned()
            .unwrap_or_else(|| action.default_combo())
    }

    /// Actions whose binding collides with `action`'s. Drives conflict warnings.
    #[must_use]
    pub fn conflicts(&self, action: EditorShortcutAction) -> Vec<EditorShortcutAction> {
        let target = self.binding(action);
        EditorShortcutAction::ALL
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_has_every_action() {
        let cfg = EditorShortcutConfig::preset();
        for action in EditorShortcutAction::ALL {
            assert_eq!(cfg.binding(action), action.default_combo());
        }
    }

    #[test]
    fn json_round_trip_preserves_bindings() {
        let cfg = EditorShortcutConfig::preset();
        let restored = EditorShortcutConfig::from_json(&cfg.to_json()).expect("parse");
        assert_eq!(cfg, restored);
    }

    #[test]
    fn duplicate_binding_is_reported_as_conflict() {
        let mut cfg = EditorShortcutConfig::preset();
        let find = cfg.binding(EditorShortcutAction::Find);
        cfg.bindings.insert(EditorShortcutAction::Save, find);
        assert!(cfg
            .conflicts(EditorShortcutAction::Save)
            .contains(&EditorShortcutAction::Find));
    }
}
