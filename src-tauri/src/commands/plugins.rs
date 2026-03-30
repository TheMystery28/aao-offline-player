use std::fs;
use std::sync::Mutex;
use tauri::State;

use crate::app_state::AppState;
use crate::importer;

/// Import a .aaoplug plugin file into one or more existing cases.
#[tauri::command]
pub async fn import_plugin(
    state: State<'_, Mutex<AppState>>,
    source_path: String,
    target_case_ids: Vec<u32>,
) -> Result<Vec<u32>, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    let path = std::path::PathBuf::from(&source_path);
    importer::import_aaoplug(&path, &target_case_ids, &data_dir).await
}

/// Import a .aaoplug ZIP as a global plugin.
#[tauri::command]
pub fn import_aaoplug_global(
    state: State<'_, Mutex<AppState>>,
    source_path: String,
) -> Result<Vec<String>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    importer::import_aaoplug_global(&std::path::PathBuf::from(&source_path), &data_dir)
}

/// Attach raw plugin JS code to one or more existing cases.
#[tauri::command]
pub fn attach_plugin_code(
    state: State<'_, Mutex<AppState>>,
    code: String,
    filename: String,
    target_case_ids: Vec<u32>,
) -> Result<Vec<u32>, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    importer::attach_plugin_code(&code, &filename, &target_case_ids, &data_dir)
}

/// List plugins installed for a given case.
#[tauri::command]
pub fn list_plugins(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
) -> Result<serde_json::Value, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    importer::list_plugins(case_id, &data_dir)
}

/// Remove a plugin from a case by filename.
#[tauri::command]
pub fn remove_plugin(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    filename: String,
) -> Result<(), String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    importer::remove_plugin(case_id, &filename, &data_dir)
}

/// Toggle a plugin's enabled/disabled state.
#[tauri::command]
pub fn toggle_plugin(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    filename: String,
    enabled: bool,
) -> Result<(), String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    importer::toggle_plugin(case_id, &filename, enabled, &data_dir)
}

/// List global plugins.
#[tauri::command]
pub fn list_global_plugins(
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    importer::list_global_plugins(&data_dir)
}

/// Attach raw plugin code as a global plugin.
#[tauri::command]
pub fn attach_global_plugin_code(
    state: State<'_, Mutex<AppState>>,
    code: String,
    filename: String,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    importer::attach_global_plugin_code(&code, &filename, &data_dir)
}

/// Remove a global plugin.
#[tauri::command]
pub fn remove_global_plugin(
    state: State<'_, Mutex<AppState>>,
    filename: String,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    importer::remove_global_plugin(&filename, &data_dir)
}

/// Toggle a global plugin's enabled/disabled state.
#[tauri::command]
pub fn toggle_global_plugin(
    state: State<'_, Mutex<AppState>>,
    filename: String,
    enabled: bool,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    importer::toggle_global_plugin(&filename, enabled, &data_dir)
}

/// Toggle a plugin's enabled/disabled state for a specific scope.
#[tauri::command]
pub fn toggle_plugin_for_scope(
    state: State<'_, Mutex<AppState>>,
    filename: String,
    scope_type: String,
    scope_key: String,
    enabled: bool,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    importer::toggle_plugin_for_scope(&filename, &scope_type, &scope_key, enabled, &data_dir)
}

/// Check for duplicate plugin code across global and all case plugins.
#[tauri::command]
pub fn check_plugin_duplicate(
    state: State<'_, Mutex<AppState>>,
    code: String,
) -> Result<Vec<importer::DuplicateMatch>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    Ok(importer::check_plugin_duplicate(&code, &data_dir))
}

/// Set params for a global plugin at a specific cascade level.
#[tauri::command]
pub fn set_global_plugin_params(
    state: State<'_, Mutex<AppState>>,
    filename: String,
    level: String,
    key: String,
    params: serde_json::Value,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    importer::set_global_plugin_params(&filename, &level, &key, &params, &data_dir)
}

/// Get all param overrides for a plugin across all cascade levels.
/// Returns { default: {...}, by_collection: {...}, by_sequence: {...}, by_case: {...} }
#[tauri::command]
pub fn get_plugin_params(
    state: State<'_, Mutex<AppState>>,
    filename: String,
) -> Result<serde_json::Value, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let manifest_path = data_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(serde_json::json!({}));
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;

    let params = val.get("plugins")
        .and_then(|p| p.get(&filename))
        .and_then(|e| e.get("params"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    Ok(params)
}

/// Get param descriptors for a plugin (extracted from JS source at attach time).
/// Returns null if descriptors were not extracted (parse failure or old plugin).
#[tauri::command]
pub fn get_plugin_descriptors(
    state: State<'_, Mutex<AppState>>,
    filename: String,
) -> Result<serde_json::Value, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let manifest_path = data_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(serde_json::Value::Null);
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;

    Ok(val.get("plugins")
        .and_then(|p| p.get(&filename))
        .and_then(|e| e.get("descriptors"))
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

/// Promote a case plugin to a global plugin.
#[tauri::command]
pub fn promote_plugin_to_global(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    filename: String,
    scope: serde_json::Value,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    importer::promote_plugin_to_global(case_id, &filename, &scope, &data_dir)
}

/// Export a case's plugins as a .aaoplug file.
#[tauri::command]
pub fn export_case_plugins(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    dest_path: String,
) -> Result<u64, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let path = std::path::PathBuf::from(&dest_path);
    importer::export_case_plugins(case_id, &path, &data_dir)
}
