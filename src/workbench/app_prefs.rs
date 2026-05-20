//! App-wide UI preferences persisted in `localStorage`.

use crate::config::{SUCCESS_SOUND_STORAGE_KEY, SUCCESS_TOAST_STORAGE_KEY};
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct AppPrefsService {
    success_toast: RwSignal<bool>,
    success_sound: RwSignal<bool>,
}

impl AppPrefsService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            success_toast: RwSignal::new(read_bool_storage(SUCCESS_TOAST_STORAGE_KEY, true)),
            success_sound: RwSignal::new(read_bool_storage(SUCCESS_SOUND_STORAGE_KEY, true)),
        }
    }

    pub fn success_toast_enabled(&self) -> RwSignal<bool> {
        self.success_toast
    }

    pub fn success_sound_enabled(&self) -> RwSignal<bool> {
        self.success_sound
    }

    pub fn set_success_toast(&self, enabled: bool) {
        self.success_toast.set(enabled);
        write_bool_storage(SUCCESS_TOAST_STORAGE_KEY, enabled);
    }

    pub fn set_success_sound(&self, enabled: bool) {
        self.success_sound.set(enabled);
        write_bool_storage(SUCCESS_SOUND_STORAGE_KEY, enabled);
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

fn write_bool_storage(key: &str, enabled: bool) {
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return;
    };
    let _ = storage.set_item(key, if enabled { "1" } else { "0" });
}
