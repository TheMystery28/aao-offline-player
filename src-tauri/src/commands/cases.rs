//! Commands for managing downloaded cases on the local file system.
//!
//! This module provides functions to list, query, and delete cases that
//! have been successfully downloaded and stored in the app's data directory.

use std::fs;
use std::path::Path;
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
    crate::library_cache::list_cases_cached(data_dir)
}

/// Return the FULL manifest (including asset_map) for a case.
/// The library listing serves slim cached manifests; this command backs the
/// Inspect modal's asset gallery.
#[tauri::command]
pub fn get_case_manifest(
    paths: State<'_, AppPaths>,
    case_id: u32,
) -> Result<downloader::manifest::CaseManifest, AppError> {
    let case_dir = paths.data_dir.join("case").join(case_id.to_string());
    downloader::manifest::read_manifest(&case_dir)
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

    let trial_data_path = case_dir.join("trial_data.json");
    let trial_data: serde_json::Value = match std::fs::read(&trial_data_path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        Err(_) => serde_json::Value::Null,
    };

    Ok(collect_missing_assets(&manifest, &trial_data, &data_dir))
}

/// Compute missing assets from a manifest + trial data, deduplicating by
/// local path (the same file can appear as a manifest URL entry AND as a
/// trial_data music/sound entry). Manifest entries keep their original URL.
fn collect_missing_assets(
    manifest: &downloader::manifest::CaseManifest,
    trial_data: &serde_json::Value,
    data_dir: &Path,
) -> Vec<downloader::manifest::MissingAsset> {
    let mut missing = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // 1. Manifest entries (keyed by URL) — dedupe by local_path, keep the URL
    let case_prefix = format!("case/{}/", manifest.case_id);
    for (url, local_path) in &manifest.asset_map {
        if local_path.is_empty() {
            continue;
        }
        let disk_path = if local_path.starts_with("defaults/") {
            local_path.to_string()
        } else {
            format!("{}{}", case_prefix, local_path)
        };
        if !downloader::vfs::asset_exists(data_dir, &disk_path) {
            if seen_paths.insert(local_path.clone()) {
                missing.push(downloader::manifest::MissingAsset {
                    url: url.clone(),
                    local_path: local_path.clone(),
                });
            }
        }
    }

    // 2. trial_data music/sound/voice paths (resolved to local paths)
    let mut report_missing = |local: String| {
        if !seen_paths.contains(&local) && !downloader::vfs::asset_exists(data_dir, &local) {
            seen_paths.insert(local.clone());
            missing.push(downloader::manifest::MissingAsset {
                url: local.clone(),
                local_path: local,
            });
        }
    };

    if let Some(music_arr) = trial_data.get("music").and_then(|v| v.as_array()) {
        for m in music_arr.iter().skip(1) {
            let external = m.get("external").and_then(|v| v.as_bool()).unwrap_or(false);
            if external { continue; }
            if let Some(path) = m.get("path").and_then(|v| v.as_str()) {
                report_missing(normalize_path(&format!("defaults/music/{}.mp3", path)));
            }
        }
    }
    if let Some(sounds_arr) = trial_data.get("sounds").and_then(|v| v.as_array()) {
        for s in sounds_arr.iter().skip(1) {
            let external = s.get("external").and_then(|v| v.as_bool()).unwrap_or(false);
            if external { continue; }
            if let Some(path) = s.get("path").and_then(|v| v.as_str()) {
                report_missing(normalize_path(&format!("defaults/sounds/{}.mp3", path)));
            }
        }
    }
    if let Some(profiles_arr) = trial_data.get("profiles").and_then(|v| v.as_array()) {
        for p in profiles_arr.iter().skip(1) {
            if let Some(voice) = p.get("voice").and_then(|v| v.as_i64()) {
                if voice < 0 && voice != -4 {
                    let id = -voice;
                    let mut found = false;
                    for ext in &["opus", "wav", "mp3"] {
                        let local = format!("defaults/voices/voice_singleblip_{}.{}", id, ext);
                        if downloader::vfs::asset_exists(data_dir, &local) {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        report_missing(format!("defaults/voices/voice_singleblip_{}.opus", id));
                    }
                }
            }
        }
    }

    missing
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
    let dedup_index = paths.dedup_index.clone();

    tokio::task::spawn_blocking(move || {
        let case_dir = data_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() {
            return Err(format!("Case {} not found", case_id).into());
        }

        // Remove case entries from the persistent hash index before deleting files
        if let Some(ref index) = dedup_index {
            let _ = index.unregister_prefix(&downloader::asset_paths::case_prefix(case_id));
        }

        fs::remove_dir_all(&case_dir)
            .map_err(|e| format!("Failed to delete case {}: {}", case_id, e))?;

        log::info!("Deleted case {} at {}", case_id, case_dir.display());

        // Auto-clean unused shared defaults
        let _ = downloader::dedup::clear_unused_defaults(&data_dir, dedup_index.as_deref());

        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("Delete task failed: {}", e)))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest_with_map(case_id: u32, entries: &[(&str, &str)]) -> downloader::manifest::CaseManifest {
        let mut asset_map = std::collections::HashMap::new();
        for (k, v) in entries {
            asset_map.insert(k.to_string(), v.to_string());
        }
        downloader::manifest::CaseManifest {
            case_id,
            title: "T".to_string(),
            author: "A".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: downloader::manifest::AssetSummary {
                case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0,
            },
            asset_map,
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        }
    }

    #[test]
    fn test_collect_missing_assets_dedupes_url_and_path_sources() {
        let dir = tempfile::tempdir().unwrap();
        // Same logical file appears BOTH as a manifest entry (url key) and as a
        // trial_data music entry (path key) — must be reported ONCE.
        let manifest = manifest_with_map(1, &[(
            "https://aaonline.fr/Ressources/Musiques/Game/song.mp3",
            "defaults/music/Game/song.mp3",
        )]);
        let trial_data = json!({
            "music": [null, {"path": "Game/song", "external": false}]
        });

        let missing = collect_missing_assets(&manifest, &trial_data, dir.path());
        let paths: Vec<&String> = missing.iter().map(|m| &m.local_path).collect();
        assert_eq!(missing.len(), 1, "duplicate sources must collapse to one entry");
        assert_eq!(paths[0], "defaults/music/Game/song.mp3");
    }

    #[test]
    fn test_collect_missing_assets_reports_true_missing() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest_with_map(1, &[(
            "http://x.com/1.png",
            "defaults/music/ActuallyMissing.mp3",
        )]);
        let trial_data = json!({"music": [], "sounds": [], "profiles": [null]});

        let missing = collect_missing_assets(&manifest, &trial_data, dir.path());
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].local_path, "defaults/music/ActuallyMissing.mp3");
        assert_eq!(missing[0].url, "http://x.com/1.png", "manifest entries keep the original URL");
    }
}
