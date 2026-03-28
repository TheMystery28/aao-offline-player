use std::fs;
use std::sync::Mutex;
use tauri::State;

use crate::app_state::AppState;

/// Back up game saves to a file in the data directory.
/// Called from JS after reading saves from localStorage via the bridge.
#[tauri::command]
pub fn backup_saves(
    state: State<'_, Mutex<AppState>>,
    saves: serde_json::Value,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let path = data_dir.join("saves_backup.json");
    let json = serde_json::to_string(&saves)
        .map_err(|e| format!("Failed to serialize saves: {}", e))?;
    fs::write(&path, json)
        .map_err(|e| format!("Failed to write saves backup: {}", e))?;
    Ok(())
}

/// Read backed-up saves from the data directory.
/// Returns the saves JSON or null if no backup exists.
#[tauri::command]
pub fn load_saves_backup(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<serde_json::Value>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
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
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
) -> Result<Option<serde_json::Value>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
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

/// Find the latest save across the given case IDs from the disk backup.
/// Returns { partId, saveDate, saveString } or null.
#[tauri::command]
pub fn find_latest_save(
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
) -> Result<Option<serde_json::Value>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
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
