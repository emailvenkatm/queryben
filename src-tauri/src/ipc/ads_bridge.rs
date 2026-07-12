//! Onboarding wizard IPC: detect an ADS install and import from it.

use tauri::State;

use crate::error::AppError;
use crate::adapters::ads_bridge::{self, AdsDetectionSummary, AdsImportSummary};
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn detect_ads_installation() -> Result<Option<AdsDetectionSummary>, AppError> {
    Ok(ads_bridge::detect_ads_installation())
}

#[tauri::command]
#[specta::specta]
pub async fn import_from_ads(
    state: State<'_, AppState>,
) -> Result<AdsImportSummary, AppError> {
    ads_bridge::import_from_ads(&state.registry).await
}
