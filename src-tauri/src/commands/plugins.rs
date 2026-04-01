use std::fs;
use std::sync::Mutex;
use tauri::State;

use crate::app_state::{AppState, AppStateLock};
use crate::importer;

/// Import a .aaoplug plugin file with scoped activation.
#[tauri::command]
pub async fn import_plugin(
    state: State<'_, Mutex<AppState>>,
    source_path: String,
    target_case_ids: Vec<u32>,
    origin: Option<String>,
) -> Result<Vec<u32>, String> {
    let (data_dir, client) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.data_dir.clone(), s.http_client.clone())
    };
    let path = std::path::PathBuf::from(&source_path);
    let origin = origin.unwrap_or_else(|| "case".to_string());
    importer::import_aaoplug(&path, &target_case_ids, &data_dir, &client, &origin).await
}

/// Import a .aaoplug ZIP as a global plugin (backward compat — delegates to import_plugin).
#[tauri::command]
pub async fn import_aaoplug_global(
    state: State<'_, Mutex<AppState>>,
    source_path: String,
) -> Result<Vec<u32>, String> {
    let (data_dir, client) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.data_dir.clone(), s.http_client.clone())
    };
    let path = std::path::PathBuf::from(&source_path);
    importer::import_aaoplug(&path, &[], &data_dir, &client, "global").await
}

/// Attach raw plugin JS code with scoped activation.
#[tauri::command]
pub async fn attach_plugin_code(
    state: State<'_, Mutex<AppState>>,
    code: String,
    filename: String,
    target_case_ids: Vec<u32>,
    origin: Option<String>,
) -> Result<Vec<u32>, String> {
    let (data_dir, client) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.data_dir.clone(), s.http_client.clone())
    };
    let origin = origin.unwrap_or_else(|| if target_case_ids.is_empty() { "global".to_string() } else { "case".to_string() });
    importer::attach_plugin_code(&code, &filename, &target_case_ids, &data_dir, &client, &origin).await
}

/// Attach raw plugin code as a global plugin (backward compat — delegates to attach_plugin_code).
#[tauri::command]
pub async fn attach_global_plugin_code(
    state: State<'_, Mutex<AppState>>,
    code: String,
    filename: String,
) -> Result<Vec<u32>, String> {
    let (data_dir, client) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.data_dir.clone(), s.http_client.clone())
    };
    importer::attach_plugin_code(&code, &filename, &[], &data_dir, &client, "global").await
}

/// List plugins active for a given case.
#[tauri::command]
pub fn list_plugins(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
) -> Result<serde_json::Value, String> {
    let data_dir = state.data_dir()?;
    importer::list_plugins(case_id, &data_dir)
}

/// Remove a plugin's scope for a case. If no scopes remain, deletes the plugin.
#[tauri::command]
pub fn remove_plugin(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    filename: String,
) -> Result<(), String> {
    let data_dir = state.data_dir()?;
    importer::remove_plugin(case_id, &filename, &data_dir)
}

/// Toggle a plugin for a case (backward compat — delegates to toggle_plugin_for_scope).
#[tauri::command]
pub fn toggle_plugin(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    filename: String,
    enabled: bool,
) -> Result<(), String> {
    let data_dir = state.data_dir()?;
    importer::toggle_plugin(case_id, &filename, enabled, &data_dir)
}

/// List all plugins (global manifest).
#[tauri::command]
pub fn list_global_plugins(
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let data_dir = state.data_dir()?;
    importer::list_global_plugins(&data_dir)
}

/// Remove a global plugin entirely (removes all scopes + deletes file).
#[tauri::command]
pub fn remove_global_plugin(
    state: State<'_, Mutex<AppState>>,
    filename: String,
) -> Result<(), String> {
    let data_dir = state.data_dir()?;
    // Remove with case_id 0 — the function handles no-scope-remaining by deleting
    // Actually, we need to force-delete: remove ALL scopes then delete
    let manifest_path = data_dir.join("plugins").join("manifest.json");
    if manifest_path.exists() {
        let text = fs::read_to_string(&manifest_path).unwrap_or_default();
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(arr) = val.get_mut("scripts").and_then(|s| s.as_array_mut()) {
                arr.retain(|s| s.as_str() != Some(&filename));
            }
            if let Some(plugins) = val.get_mut("plugins").and_then(|p| p.as_object_mut()) {
                plugins.remove(&filename);
            }
            match serde_json::to_string_pretty(&val) {
                Ok(json) => {
                    if let Err(e) = fs::write(&manifest_path, json) {
                        eprintln!("[PLUGINS] Failed to write {}: {}", manifest_path.display(), e);
                    }
                }
                Err(e) => eprintln!("[PLUGINS] Failed to serialize manifest: {}", e),
            }
        }
    }
    // Delete the plugin's declared assets, then the JS file itself
    let plugins_dir = data_dir.join("plugins");
    importer::delete_plugin_assets(&filename, &plugins_dir);
    let _ = fs::remove_file(plugins_dir.join(&filename));
    Ok(())
}

/// Toggle a global plugin (backward compat — sets scope.all).
#[tauri::command]
pub fn toggle_global_plugin(
    state: State<'_, Mutex<AppState>>,
    filename: String,
    enabled: bool,
) -> Result<(), String> {
    let data_dir = state.data_dir()?;
    importer::toggle_plugin_for_scope(&filename, "global", "", enabled, &data_dir)
}

/// Toggle a plugin for a specific scope.
#[tauri::command]
pub fn toggle_plugin_for_scope(
    state: State<'_, Mutex<AppState>>,
    filename: String,
    scope_type: String,
    scope_key: String,
    enabled: bool,
) -> Result<(), String> {
    let data_dir = state.data_dir()?;
    importer::toggle_plugin_for_scope(&filename, &scope_type, &scope_key, enabled, &data_dir)
}

/// Check for duplicate plugin code.
#[tauri::command]
pub fn check_plugin_duplicate(
    state: State<'_, Mutex<AppState>>,
    code: String,
) -> Result<Vec<importer::DuplicateMatch>, String> {
    let data_dir = state.data_dir()?;
    Ok(importer::check_plugin_duplicate(&code, &data_dir))
}

/// Set params for a plugin at a specific cascade level.
#[tauri::command]
pub fn set_global_plugin_params(
    state: State<'_, Mutex<AppState>>,
    filename: String,
    level: String,
    key: String,
    params: serde_json::Value,
) -> Result<(), String> {
    let data_dir = state.data_dir()?;
    importer::set_global_plugin_params(&filename, &level, &key, &params, &data_dir)
}

/// Get all param overrides for a plugin.
#[tauri::command]
pub fn get_plugin_params(
    state: State<'_, Mutex<AppState>>,
    filename: String,
) -> Result<serde_json::Value, String> {
    let data_dir = state.data_dir()?;
    let manifest_path = data_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(serde_json::json!({}));
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;
    Ok(val.get("plugins")
        .and_then(|p| p.get(&filename))
        .and_then(|e| e.get("params"))
        .cloned()
        .unwrap_or(serde_json::json!({})))
}

/// Get param descriptors for a plugin.
#[tauri::command]
pub fn get_plugin_descriptors(
    state: State<'_, Mutex<AppState>>,
    filename: String,
) -> Result<serde_json::Value, String> {
    let data_dir = state.data_dir()?;
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

/// Export a case's plugins as a .aaoplug file.
#[tauri::command]
pub async fn export_case_plugins(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    dest_path: String,
) -> Result<u64, String> {
    let data_dir = state.data_dir()?;
    let path = std::path::PathBuf::from(&dest_path);
    tokio::task::spawn_blocking(move || {
        importer::export_case_plugins(case_id, &path, &data_dir)
    }).await.map_err(|e| format!("Export task failed: {}", e))?
}
