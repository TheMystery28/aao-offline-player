//! Commands for managing downloaded cases on the local file system.
//!
//! This module provides functions to list, query, and delete cases that
//! have been successfully downloaded and stored in the app's data directory.

use std::fs;
use tauri::State;

use crate::app_state::AppPaths;
use crate::downloader;
use crate::downloader::paths::normalize_path;
use crate::error::AppError;

/// List all downloaded cases by scanning the `case/` directory for manifests.
///
/// This function iterates through all subdirectories in `{data_dir}/case/`,
/// attempting to read a `manifest.json` from each.
///
/// # Returns
///
/// A `Vec<CaseManifest>` containing the metadata for all valid downloaded cases,
/// sorted alphabetically by title.
#[tauri::command]
pub fn list_cases(
    paths: State<'_, AppPaths>,
) -> Result<Vec<downloader::manifest::CaseManifest>, AppError> {
    let data_dir = &paths.data_dir;

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
                    log::warn!("Skipping {}: {}", path.display(), e);
                }
            }
        }
    }

    // Sort by title
    cases.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(cases)
}

/// Get assets in the manifest whose files are missing from disk.
///
/// Checks two sources:
/// 1. `asset_map` entries whose local files don't exist
/// 2. Asset-like paths in `trial_data.json` that aren't on disk (catches
///    defaults that were never downloaded or imported)
///
/// Called lazily when the Inspect modal opens — avoids disk I/O on every library load.
#[tauri::command]
pub async fn get_missing_assets(
    paths: State<'_, AppPaths>,
    case_id: u32,
) -> Result<Vec<downloader::manifest::MissingAsset>, AppError> {
    let data_dir = paths.data_dir.clone();
    let case_dir = data_dir.join("case").join(case_id.to_string());
    let manifest = downloader::manifest::read_manifest(&case_dir)?;

    let mut missing = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Check asset_map entries
    let case_prefix = format!("case/{}/", case_id);
    for (url, local_path) in &manifest.asset_map {
        if local_path.is_empty() {
            continue;
        }
        let disk_path = if local_path.starts_with("defaults/") {
            local_path.to_string()
        } else {
            format!("{}{}", case_prefix, local_path)
        };
        if !downloader::vfs::asset_exists(&data_dir, &disk_path) {
            seen.insert(url.clone());
            missing.push(downloader::manifest::MissingAsset {
                url: url.clone(),
                local_path: local_path.clone(),
            });
        }
    }

    // 2. Scan trial_data.json for sound/music/voice assets not on disk.
    //    trial_data stores raw paths (e.g. music[i].path = "Game/song"),
    //    resolved at runtime to "defaults/music/Game/song.mp3" etc.
    let trial_data_path = case_dir.join("trial_data.json");
    if let Ok(td_bytes) = std::fs::read(&trial_data_path) {
        if let Ok(td) = serde_json::from_slice::<serde_json::Value>(&td_bytes) {
            // Check music entries
            if let Some(music_arr) = td.get("music").and_then(|v| v.as_array()) {
                for m in music_arr.iter().skip(1) {
                    let external = m.get("external").and_then(|v| v.as_bool()).unwrap_or(false);
                    if external { continue; }
                    if let Some(path) = m.get("path").and_then(|v| v.as_str()) {
                        let local = normalize_path(&format!("defaults/music/{}.mp3", path));
                        if !seen.contains(&local) && !downloader::vfs::asset_exists(&data_dir, &local) {
                            seen.insert(local.clone());
                            missing.push(downloader::manifest::MissingAsset {
                                url: local.clone(), local_path: local,
                            });
                        }
                    }
                }
            }
            // Check sound entries
            if let Some(sounds_arr) = td.get("sounds").and_then(|v| v.as_array()) {
                for s in sounds_arr.iter().skip(1) {
                    let external = s.get("external").and_then(|v| v.as_bool()).unwrap_or(false);
                    if external { continue; }
                    if let Some(path) = s.get("path").and_then(|v| v.as_str()) {
                        let local = normalize_path(&format!("defaults/sounds/{}.mp3", path));
                        if !seen.contains(&local) && !downloader::vfs::asset_exists(&data_dir, &local) {
                            seen.insert(local.clone());
                            missing.push(downloader::manifest::MissingAsset {
                                url: local.clone(), local_path: local,
                            });
                        }
                    }
                }
            }
            // Check voice entries (used by profiles)
            if let Some(profiles_arr) = td.get("profiles").and_then(|v| v.as_array()) {
                for p in profiles_arr.iter().skip(1) {
                    if let Some(voice) = p.get("voice").and_then(|v| v.as_i64()) {
                        if voice < 0 && voice != -4 {
                            let id = -voice;
                            for ext in &["opus", "wav", "mp3"] {
                                let local = format!("defaults/voices/voice_singleblip_{}.{}", id, ext);
                                if !seen.contains(&local) && downloader::vfs::asset_exists(&data_dir, &local) {
                                    // At least one format exists — don't report as missing
                                    seen.insert(format!("voice_{}", id));
                                    break;
                                }
                            }
                            let voice_key = format!("voice_{}", id);
                            if !seen.contains(&voice_key) {
                                seen.insert(voice_key);
                                let local = format!("defaults/voices/voice_singleblip_{}.opus", id);
                                missing.push(downloader::manifest::MissingAsset {
                                    url: local.clone(), local_path: local,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(missing)
}

/// Delete a downloaded case and all its associated files from disk.
///
/// This also unregisters the case assets from the de-duplication index to
/// ensure that shared assets (like defaults) can be properly cleaned up
/// if they are no longer needed by any other case.
///
/// # Errors
///
/// Returns an error if the case directory does not exist or if deletion fails.
#[tauri::command]
pub async fn delete_case(paths: State<'_, AppPaths>, case_id: u32) -> Result<(), AppError> {
    let data_dir = paths.data_dir.clone();

    tokio::task::spawn_blocking(move || {
        let case_dir = data_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() {
            return Err(format!("Case {} not found", case_id).into());
        }

        // Remove case entries from the persistent hash index before deleting files
        if let Ok(index) = downloader::dedup::DedupIndex::open(&data_dir) {
            let _ = index.unregister_prefix(&downloader::asset_paths::case_prefix(case_id));
        }

        fs::remove_dir_all(&case_dir)
            .map_err(|e| format!("Failed to delete case {}: {}", case_id, e))?;

        log::info!("Deleted case {} at {}", case_id, case_dir.display());

        // Auto-clean unused shared defaults
        let _ = downloader::dedup::clear_unused_defaults(&data_dir);

        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("Delete task failed: {}", e)))?
}
