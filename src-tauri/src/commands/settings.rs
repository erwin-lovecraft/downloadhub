//! Tauri commands for persisted app settings.

use crate::state::AppState;
use downloadhub_core::settings::{self, AppSettings};
use tauri::State;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    settings::load(state.settings_path()?)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_settings(
    new_settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    settings::save(state.settings_path()?, &new_settings)
        .await
        .map_err(|e| e.to_string())
}
