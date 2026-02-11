use tauri::State;

use crate::dto::{FeffFitConfig, FeffFitResultDto};
use crate::state::AppState;

#[tauri::command]
pub fn run_feff_fit(
    state: State<'_, AppState>,
    config: FeffFitConfig,
) -> Result<FeffFitResultDto, String> {
    // TODO: Implement FEFF fitting integration
    // This requires building FeffPathModel, FeffFitDataset, and running the fit
    // For now, return a placeholder error
    let _ = (state, config);
    Err("FEFF fitting not yet implemented in GUI".into())
}

#[tauri::command]
pub fn get_fit_result(
    state: State<'_, AppState>,
    fit_id: String,
) -> Result<FeffFitResultDto, String> {
    let fits = state.fit_results.lock().map_err(|e| e.to_string())?;
    fits.get(&fit_id)
        .cloned()
        .ok_or_else(|| format!("Fit result '{}' not found", fit_id))
}

#[tauri::command]
pub fn list_fit_results(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let fits = state.fit_results.lock().map_err(|e| e.to_string())?;
    Ok(fits.keys().cloned().collect())
}
