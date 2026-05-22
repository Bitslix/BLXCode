//! Tauri commands for web tool settings.

use crate::agent::web_settings::{self, AgentWebSettings, AgentWebSettingsView, WebProviderKind};
use serde::Deserialize;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSettingsPatch {
    pub provider: WebProviderKind,
}

#[tauri::command]
pub fn agent_web_settings_get(app: AppHandle) -> Result<AgentWebSettingsView, String> {
    web_settings::refresh_runtime_from_app(&app);
    web_settings::view(&app)
}

#[tauri::command]
pub fn agent_web_settings_save(
    app: AppHandle,
    patch: WebSettingsPatch,
) -> Result<AgentWebSettingsView, String> {
    let settings = AgentWebSettings {
        provider: patch.provider,
    };
    web_settings::save(&app, &settings)
}

#[tauri::command]
pub fn agent_environment_invalidate() {
    crate::agent::environment::invalidate_cache();
}
