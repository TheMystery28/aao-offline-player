use std::fs;
use std::sync::Mutex;
use tauri::State;

use crate::app_state::{AppState, AppStateLock};
use crate::downloader;

/// List all downloaded cases by scanning the case directory for manifests.
#[tauri::command]
pub fn list_cases(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<downloader::manifest::CaseManifest>, String> {
    let data_dir = state.data_dir()?;

    let cases_dir = data_dir.join("case");
    if !cases_dir.exists() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    let entries =
        fs::read_dir(&cases_dir).map_err(|e| format!("Failed to read cases directory: {}", e))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("manifest.json");
        if manifest_path.exists() {
            match downloader::manifest::read_manifest(&path) {
                Ok(manifest) => cases.push(manifest),
                Err(e) => {
                    debug_log!("Warning: skipping {}: {}", path.display(), e);
                }
            }
        }
    }

    // Sort by title
    cases.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(cases)
}

/// Delete a downloaded case and all its files.
#[tauri::command]
pub fn delete_case(state: State<'_, Mutex<AppState>>, case_id: u32) -> Result<(), String> {
    let data_dir = state.data_dir()?;

    let case_dir = data_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id));
    }

    // Remove case entries from the persistent hash index before deleting files
    if let Ok(index) = downloader::dedup::DedupIndex::open(&data_dir) {
        let _ = index.unregister_prefix(&downloader::asset_paths::case_prefix(case_id));
    }

    fs::remove_dir_all(&case_dir)
        .map_err(|e| format!("Failed to delete case {}: {}", case_id, e))?;

    debug_log!("Deleted case {} at {}", case_id, case_dir.display());

    // Auto-clean unused shared defaults
    let _ = downloader::dedup::clear_unused_defaults(&data_dir);

    Ok(())
}
