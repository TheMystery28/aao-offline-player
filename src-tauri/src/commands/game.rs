use std::sync::Mutex;
use tauri::State;

use crate::app_state::AppState;
use crate::importer;

/// Returns the localhost URL for playing a specific case, including language preference.
#[tauri::command]
pub fn open_game(state: State<'_, Mutex<AppState>>, case_id: u32) -> Result<String, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    // Resolve which global plugins apply to this case (writes resolved_plugins.json)
    let _ = importer::resolve_plugins_for_case(case_id, &state.data_dir);
    Ok(format!(
        "http://localhost:{}/player.html?trial_id={}&lang={}",
        state.server_port, case_id, state.config.language
    ))
}

/// Returns the asset server's base URL.
#[tauri::command]
pub fn get_server_url(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(format!("http://localhost:{}", state.server_port))
}
