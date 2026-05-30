//! App-wide UI preferences persisted in `localStorage`.

use super::shortcut_config::{Binding, KeyChord, ShortcutAction, ShortcutConfig};
use crate::config::{
    CONFIRM_CLOSE_WORKSPACE_KEY, SHORTCUT_BINDINGS_STORAGE_KEY, SHORTCUT_MODE_LEGACY,
    SHORTCUT_MODE_STORAGE_KEY, SHORTCUT_MODE_TMUX, SUCCESS_SOUND_STORAGE_KEY,
    SUCCESS_TOAST_STORAGE_KEY, UPDATE_AUTO_CHECK_KEY,
};
use leptos::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutMode {
    Tmux,
    Legacy,
}

impl ShortcutMode {
    #[must_use]
    pub fn from_storage(value: Option<&str>) -> Self {
        match value {
            Some(SHORTCUT_MODE_LEGACY) => Self::Legacy,
            _ => Self::Tmux,
        }
    }

    #[must_use]
    pub const fn storage_value(self) -> &'static str {
        match self {
            Self::Tmux => SHORTCUT_MODE_TMUX,
            Self::Legacy => SHORTCUT_MODE_LEGACY,
        }
    }
}

#[derive(Clone, Copy)]
pub struct AppPrefsService {
    success_toast: RwSignal<bool>,
    success_sound: RwSignal<bool>,
    shortcut_mode: RwSignal<ShortcutMode>,
    shortcut_config: RwSignal<ShortcutConfig>,
    update_auto_check: RwSignal<bool>,
    confirm_close_workspace: RwSignal<bool>,
}

impl AppPrefsService {
    #[must_use]
    pub fn new() -> Self {
        let shortcut_mode =
            ShortcutMode::from_storage(read_string_storage(SHORTCUT_MODE_STORAGE_KEY).as_deref());
        // Load custom bindings if present; otherwise seed from the stored
        // preset mode (migration path for existing installs).
        let shortcut_config = read_string_storage(SHORTCUT_BINDINGS_STORAGE_KEY)
            .as_deref()
            .and_then(ShortcutConfig::from_json)
            .unwrap_or_else(|| ShortcutConfig::preset(shortcut_mode));
        Self {
            success_toast: RwSignal::new(read_bool_storage(SUCCESS_TOAST_STORAGE_KEY, true)),
            success_sound: RwSignal::new(read_bool_storage(SUCCESS_SOUND_STORAGE_KEY, true)),
            shortcut_mode: RwSignal::new(shortcut_mode),
            shortcut_config: RwSignal::new(shortcut_config),
            update_auto_check: RwSignal::new(read_bool_storage(UPDATE_AUTO_CHECK_KEY, true)),
            confirm_close_workspace: RwSignal::new(read_bool_storage(
                CONFIRM_CLOSE_WORKSPACE_KEY,
                true,
            )),
        }
    }

    pub fn success_toast_enabled(&self) -> RwSignal<bool> {
        self.success_toast
    }

    pub fn success_sound_enabled(&self) -> RwSignal<bool> {
        self.success_sound
    }

    pub fn shortcut_mode(&self) -> RwSignal<ShortcutMode> {
        self.shortcut_mode
    }

    pub fn update_auto_check_enabled(&self) -> RwSignal<bool> {
        self.update_auto_check
    }

    pub fn confirm_close_workspace_enabled(&self) -> RwSignal<bool> {
        self.confirm_close_workspace
    }

    pub fn set_success_toast(&self, enabled: bool) {
        self.success_toast.set(enabled);
        write_bool_storage(SUCCESS_TOAST_STORAGE_KEY, enabled);
    }

    pub fn set_success_sound(&self, enabled: bool) {
        self.success_sound.set(enabled);
        write_bool_storage(SUCCESS_SOUND_STORAGE_KEY, enabled);
    }

    pub fn set_shortcut_mode(&self, mode: ShortcutMode) {
        self.shortcut_mode.set(mode);
        write_string_storage(SHORTCUT_MODE_STORAGE_KEY, mode.storage_value());
    }

    #[must_use]
    pub fn shortcut_config(&self) -> RwSignal<ShortcutConfig> {
        self.shortcut_config
    }

    fn persist_config(&self) {
        write_string_storage(
            SHORTCUT_BINDINGS_STORAGE_KEY,
            &self.shortcut_config.get_untracked().to_json(),
        );
    }

    /// Replace all bindings (and the prefix) with a preset, and record the
    /// preset as the active mode.
    pub fn apply_shortcut_preset(&self, mode: ShortcutMode) {
        self.set_shortcut_mode(mode);
        self.shortcut_config.set(ShortcutConfig::preset(mode));
        self.persist_config();
    }

    /// Reset a single action to its current preset default.
    pub fn reset_shortcut_binding(&self, action: ShortcutAction) {
        let preset = ShortcutConfig::preset(self.shortcut_mode.get_untracked());
        let binding = preset.binding(action);
        self.shortcut_config.update(|cfg| {
            cfg.bindings.insert(action, binding);
        });
        self.persist_config();
    }

    pub fn set_shortcut_binding(&self, action: ShortcutAction, binding: Binding) {
        self.shortcut_config.update(|cfg| {
            cfg.bindings.insert(action, binding);
        });
        self.persist_config();
    }

    pub fn set_shortcut_prefix(&self, prefix: KeyChord) {
        self.shortcut_config.update(|cfg| cfg.prefix = prefix);
        self.persist_config();
    }

    pub fn set_update_auto_check(&self, enabled: bool) {
        self.update_auto_check.set(enabled);
        write_bool_storage(UPDATE_AUTO_CHECK_KEY, enabled);
    }

    pub fn set_confirm_close_workspace(&self, enabled: bool) {
        self.confirm_close_workspace.set(enabled);
        write_bool_storage(CONFIRM_CLOSE_WORKSPACE_KEY, enabled);
    }
}

fn read_bool_storage(key: &str, default: bool) -> bool {
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return default;
    };
    match storage.get_item(key).ok().flatten() {
        Some(v) if v == "0" || v == "false" => false,
        Some(_) => true,
        None => default,
    }
}

fn read_string_storage(key: &str) -> Option<String> {
    let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten())?;
    storage.get_item(key).ok().flatten()
}

fn write_bool_storage(key: &str, enabled: bool) {
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return;
    };
    let _ = storage.set_item(key, if enabled { "1" } else { "0" });
}

fn write_string_storage(key: &str, value: &str) {
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return;
    };
    let _ = storage.set_item(key, value);
}
