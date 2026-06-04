use super::install::{
    install_from_github, PluginInstallProgress, PluginInstallRequest, PluginInstallState,
};
use super::store;
use super::types::PluginRegistry;
use tauri::State;

#[tauri::command]
pub fn plugins_list() -> Result<PluginRegistry, String> {
    store::load_registry_with_builtins()
}

#[tauri::command]
pub async fn plugins_install_from_github(
    state: State<'_, PluginInstallState>,
    request: PluginInstallRequest,
) -> Result<PluginRegistry, String> {
    let state = state.inner().clone();
    crate::proc::run_blocking(move || {
        install_from_github(&state, request)?;
        store::load_registry_with_builtins()
    })
    .await
}

#[tauri::command]
pub fn plugins_install_progress(
    state: State<'_, PluginInstallState>,
) -> Result<PluginInstallProgress, String> {
    state.snapshot()
}

#[tauri::command]
pub fn plugins_set_enabled(plugin_id: String, enabled: bool) -> Result<PluginRegistry, String> {
    store::set_enabled(&plugin_id, enabled)
}

#[tauri::command]
pub fn plugins_remove(plugin_id: String) -> Result<PluginRegistry, String> {
    store::remove_entry(&plugin_id)
}
