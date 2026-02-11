use std::path::PathBuf;

use tauri::State;

use crate::dto::WorkspaceData;
use crate::state::AppState;

#[tauri::command]
pub fn save_workspace(
    state: State<'_, AppState>,
    path: String,
    data: WorkspaceData,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;

    let mut ws_path = state.workspace_path.lock().map_err(|e| e.to_string())?;
    *ws_path = Some(PathBuf::from(&path));

    // Also save spectral data as BSON alongside workspace
    let group = state.group.lock().map_err(|e| e.to_string())?;
    if !group.is_empty() {
        let data_path = PathBuf::from(&path).with_extension("xas.bson");
        group
            .write_bson(
                data_path
                    .to_str()
                    .ok_or("Invalid path for data file")?,
            )
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn load_workspace(
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspaceData, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let data: WorkspaceData = serde_json::from_str(&json).map_err(|e| e.to_string())?;

    // Load spectral data from companion BSON file
    let data_path = PathBuf::from(&path).with_extension("xas.bson");
    if data_path.exists() {
        let mut group = state.group.lock().map_err(|e| e.to_string())?;
        *group = xraytsubaki::prelude::XASGroup::new();
        group
            .read_bson(
                data_path
                    .to_str()
                    .ok_or("Invalid path for data file")?,
            )
            .map_err(|e| e.to_string())?;
    }

    let mut ws_path = state.workspace_path.lock().map_err(|e| e.to_string())?;
    *ws_path = Some(PathBuf::from(&path));

    Ok(data)
}

#[tauri::command]
pub fn get_workspace_path(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let ws_path = state.workspace_path.lock().map_err(|e| e.to_string())?;
    Ok(ws_path.as_ref().map(|p| p.to_string_lossy().to_string()))
}
