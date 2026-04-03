//! Commands for managing and backing up game saves.
//!
//! Since the AAO player runs in a WebView and uses `localStorage` for saves,
//! this module provides a way to back up those saves to the local disk and
//! restore them, as well as use them during case export.

use std::fs;
use tauri::State;

use crate::app_state::AppPaths;
use crate::error::AppError;

/// Back up game saves to a JSON file in the app's data directory.
///
/// This is typically called from the frontend after it extracts saves from
/// the WebView's `localStorage` via a bridge.
#[tauri::command]
pub fn backup_saves(
    paths: State<'_, AppPaths>,
    saves: serde_json::Value,
) -> Result<(), AppError> {
    let data_dir = &paths.data_dir;
    let path = data_dir.join("saves_backup.json");
    let json = serde_json::to_string(&saves)
        .map_err(|e| format!("Failed to serialize saves: {}", e))?;
    fs::write(&path, json)
        .map_err(|e| format!("Failed to write saves backup: {}", e))?;
    Ok(())
}

/// Read the backed-up game saves from the data directory.
///
/// # Returns
///
/// The saves JSON object, or `None` if no backup exists.
#[tauri::command]
pub fn load_saves_backup(
    paths: State<'_, AppPaths>,
) -> Result<Option<serde_json::Value>, AppError> {
    let data_dir = &paths.data_dir;
    let path = data_dir.join("saves_backup.json");
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read saves backup: {}", e))?;
    let value: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse saves backup: {}", e))?;
    Ok(Some(value))
}

/// Read saves from the backup file, filtered by case IDs.
/// Returns the saves for only the requested cases, or None if no backup or no matching saves.
#[tauri::command]
pub fn read_saves_for_export(
    paths: State<'_, AppPaths>,
    case_ids: Vec<u32>,
) -> Result<Option<serde_json::Value>, AppError> {
    let data_dir = &paths.data_dir;
    let path = data_dir.join("saves_backup.json");
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read saves backup: {}", e))?;
    let all: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse saves backup: {}", e))?;

    let mut filtered = serde_json::Map::new();
    for id in &case_ids {
        let key = id.to_string();
        if let Some(val) = all.get(&key) {
            filtered.insert(key, val.clone());
        }
    }
    if filtered.is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::Value::Object(filtered)))
}

/// Find the most recent game save across a list of case IDs.
///
/// # Arguments
///
/// * `case_ids` - A list of case IDs to search through.
///
/// # Returns
///
/// A JSON object containing `partId`, `saveDate`, and `saveString`, or `None`.
#[tauri::command]
pub fn find_latest_save(
    paths: State<'_, AppPaths>,
    case_ids: Vec<u32>,
) -> Result<Option<serde_json::Value>, AppError> {
    let data_dir = &paths.data_dir;
    let path = data_dir.join("saves_backup.json");
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read saves backup: {}", e))?;
    let all: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse saves backup: {}", e))?;

    let mut latest_ts: u64 = 0;
    let mut latest_part: Option<u32> = None;
    let mut latest_save: Option<String> = None;

    for &id in &case_ids {
        let key = id.to_string();
        if let Some(saves) = all.get(&key).and_then(|v| v.as_object()) {
            for (ts_str, save_val) in saves {
                if let Ok(ts) = ts_str.parse::<u64>() {
                    if ts > latest_ts {
                        latest_ts = ts;
                        latest_part = Some(id);
                        latest_save = save_val.as_str().map(|s| s.to_string());
                    }
                }
            }
        }
    }

    match (latest_part, latest_save) {
        (Some(part_id), Some(save_string)) => Ok(Some(serde_json::json!({
            "partId": part_id,
            "saveDate": latest_ts,
            "saveString": save_string
        }))),
        _ => Ok(None),
    }
}
