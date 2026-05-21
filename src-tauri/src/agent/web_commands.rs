//! Tauri commands for web tool settings.

use crate::agent::web_settings::{
    self, AgentWebSettings, AgentWebSettingsView, WebKeyKind, WebProviderKind,
};
use serde::Deserialize;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSettingsPatch {
    pub provider: WebProviderKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebKeyPayload {
    pub kind: String,
    pub api_key: String,
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
pub fn agent_web_api_key_set(app: AppHandle, payload: WebKeyPayload) -> Result<AgentWebSettingsView, String> {
    let kind = parse_key_kind(&payload.kind)?;
    web_settings::set_key(&app, kind, payload.api_key)
}

#[tauri::command]
pub fn agent_web_api_key_delete(app: AppHandle, kind: String) -> Result<AgentWebSettingsView, String> {
    web_settings::delete_key(&app, parse_key_kind(&kind)?)
}

#[tauri::command]
pub fn agent_environment_invalidate() {
    crate::agent::environment::invalidate_cache();
}

fn parse_key_kind(s: &str) -> Result<WebKeyKind, String> {
    match s {
        "tavily" => Ok(WebKeyKind::Tavily),
        "brave" => Ok(WebKeyKind::Brave),
        _ => Err(format!("unknown web key kind: {s}")),
    }
}
