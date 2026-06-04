//! In-app code editor preferences, persisted in `localStorage`.
//!
//! Currently holds the Vim key-bindings toggle (default **on**). Mirrors the
//! `ThemeService` pattern: a `Copy` handle over `RwSignal`s, loaded on
//! construction and written through on every setter. Further code-editor
//! settings (tab size, indent style, …) are expected to land here.

use leptos::prelude::*;

use crate::config::CODE_EDITOR_VIM_KEY;

/// Default Vim state when nothing is stored yet.
const DEFAULT_VIM_ENABLED: bool = true;

#[derive(Clone, Copy)]
pub struct EditorSettingsService {
    vim_enabled: RwSignal<bool>,
}

impl EditorSettingsService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            vim_enabled: RwSignal::new(read_vim_storage()),
        }
    }

    /// Reactive Vim-enabled signal (read in the editor + status bar + settings).
    #[must_use]
    pub fn vim_enabled(&self) -> RwSignal<bool> {
        self.vim_enabled
    }

    /// Toggle Vim key bindings and persist the choice.
    pub fn set_vim_enabled(&self, enabled: bool) {
        self.vim_enabled.set(enabled);
        write_string(CODE_EDITOR_VIM_KEY, if enabled { "true" } else { "false" });
    }
}

impl Default for EditorSettingsService {
    fn default() -> Self {
        Self::new()
    }
}

fn read_vim_storage() -> bool {
    match read_string(CODE_EDITOR_VIM_KEY).as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => DEFAULT_VIM_ENABLED,
    }
}

fn read_string(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(key).ok().flatten())
}

fn write_string(key: &str, value: &str) {
    if let Some(w) = web_sys::window() {
        if let Ok(Some(s)) = w.local_storage() {
            let _ = s.set_item(key, value);
        }
    }
}
