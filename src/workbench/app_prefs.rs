//! App-wide UI preferences persisted in `localStorage`.

use crate::config::{
    SHORTCUT_MODE_LEGACY, SHORTCUT_MODE_STORAGE_KEY, SHORTCUT_MODE_TMUX, SUCCESS_SOUND_STORAGE_KEY,
    SUCCESS_TOAST_STORAGE_KEY,
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
}

impl AppPrefsService {
    #[must_use]
    pub fn new() -> Self {
        let shortcut_mode =
            ShortcutMode::from_storage(read_string_storage(SHORTCUT_MODE_STORAGE_KEY).as_deref());
        Self {
            success_toast: RwSignal::new(read_bool_storage(SUCCESS_TOAST_STORAGE_KEY, true)),
            success_sound: RwSignal::new(read_bool_storage(SUCCESS_SOUND_STORAGE_KEY, true)),
            shortcut_mode: RwSignal::new(shortcut_mode),
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
