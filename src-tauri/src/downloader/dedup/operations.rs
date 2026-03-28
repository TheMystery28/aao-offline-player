use std::fs;
use std::path::Path;

use serde_json::Value;

use super::helpers::rewrite_value_recursive;
use super::index::DedupIndex;
use crate::downloader::manifest::{read_manifest, write_manifest};
use crate::downloader::paths::normalize_path;

/// Run all post-import processing for a case: register assets in the dedup index,
/// then deduplicate case assets against the shared defaults pool.
///
/// **All import paths must call this** after a case is installed — whether from
/// aaoffline folder, .aaocase ZIP, or the download pipeline. This single entry
/// point prevents omissions (previously aaoffline imports forgot to dedup).
///
/// Steps:
/// 1. Open (or create) the dedup index
/// 2. Register the case's assets + any new defaults in the index
/// 3. Deduplicate: replace case assets identical to shared defaults with references
///
/// Returns (dedup_count, bytes_saved). Errors are non-fatal — import succeeds even if dedup fails.
pub fn finalize_case_import(case_id: u32, data_dir: &Path) -> (usize, u64) {
    // Register in index + dedup — both handled by dedup_case_assets which
    // internally opens the index, scans defaults, and performs dedup.
    match dedup_case_assets(case_id, data_dir) {
        Ok((count, bytes)) => {
            if count > 0 {
                eprintln!("[DEDUP] Case {}: {} files deduplicated, {} bytes saved", case_id, count, bytes);
            }
            (count, bytes)
        }
        Err(e) => {
            eprintln!("[DEDUP] Case {}: dedup failed (non-fatal): {}", case_id, e);
            (0, 0)
        }
    }
}

/// Batch version: run post-import processing for multiple cases.
/// Used after batch aaoffline imports and multi-case .aaocase imports.
pub fn finalize_batch_import(case_ids: &[u32], data_dir: &Path) -> (usize, u64) {
    let mut total_count = 0;
    let mut total_bytes = 0;
    for &case_id in case_ids {
        let (c, b) = finalize_case_import(case_id, data_dir);
        total_count += c;
        total_bytes += b;
    }
    (total_count, total_bytes)
}

/// Dedup a single case's assets against the shared defaults pool.
/// Opens its own DedupIndex. For use from download/import pipelines.
pub fn dedup_case_assets(case_id: u32, data_dir: &Path) -> Result<(usize, u64), String> {
    let case_dir = data_dir.join("case").join(case_id.to_string());
    let assets_dir = case_dir.join("assets");
    if !assets_dir.is_dir() {
        return Ok((0, 0));
    }
    let defaults_dir = data_dir.join("defaults");
    if !defaults_dir.is_dir() {
        return Ok((0, 0));
    }
    let index = DedupIndex::open(data_dir)?;
    index.scan_and_register(data_dir, "defaults")?;
    dedup_case_assets_with_index(case_id, data_dir, &index)
}

/// Dedup a single case's assets using a pre-opened index.
/// Avoids opening a second DedupIndex when called from optimize_all_cases.
pub fn dedup_case_assets_with_index(
    case_id: u32,
    data_dir: &Path,
    index: &DedupIndex,
) -> Result<(usize, u64), String> {
    let case_dir = data_dir.join("case").join(case_id.to_string());
    let assets_dir = case_dir.join("assets");
    if !assets_dir.is_dir() {
        return Ok((0, 0));
    }

    // Read manifest
    let mut manifest = read_manifest(&case_dir)?;

    // Read trial_data.json for URL rewriting
    let trial_data_path = case_dir.join("trial_data.json");
    let mut trial_data: Option<Value> = if trial_data_path.exists() {
        let text = fs::read_to_string(&trial_data_path)
            .map_err(|e| format!("Failed to read trial_data.json: {}", e))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse trial_data.json: {}", e))
            .ok()
    } else {
        None
    };

    let mut deduped_count = 0usize;
    let mut bytes_saved = 0u64;
    let mut trial_data_modified = false;

    // Collect asset files first to avoid borrow issues with fs::remove_file during iteration
    let asset_files: Vec<_> = match fs::read_dir(&assets_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect(),
        Err(_) => return Ok((0, 0)),
    };

    for entry in &asset_files {
        let file_path = entry.path();
        let file_size = match file_path.metadata() {
            Ok(m) => m.len(),
            Err(_) => continue,
        };

        // Check if this file has a duplicate in defaults/ (skip matches against other case assets)
        if let Some(default_relative_path) = index.find_duplicate(&file_path, data_dir) {
            if !default_relative_path.starts_with("defaults/") {
                continue; // Only dedup against defaults, not other case assets
            }
            let asset_filename = match file_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let old_local_path = format!("assets/{}", asset_filename);

            // Find the URL(s) in asset_map that point to this assets/ path
            let urls_to_update: Vec<String> = manifest
                .asset_map
                .iter()
                .filter(|(_, v)| **v == old_local_path)
                .map(|(k, _)| k.clone())
                .collect();

            if urls_to_update.is_empty() {
                continue; // No manifest entry points here, skip
            }

            // Verify the default file actually exists on disk before deleting the case copy
            let default_full_path = data_dir.join(&default_relative_path);
            if !default_full_path.is_file() {
                continue;
            }

            // Update manifest
            for url in &urls_to_update {
                manifest
                    .asset_map
                    .insert(url.clone(), default_relative_path.clone());
            }

            // Rewrite references in trial_data.json
            if let Some(ref mut td) = trial_data {
                let old_server_path = format!("case/{}/{}", case_id, old_local_path);
                rewrite_value_recursive(td, &old_server_path, &default_relative_path);
                trial_data_modified = true;
            }

            // Delete the duplicate file
            if fs::remove_file(&file_path).is_ok() {
                // Unregister case asset from the persistent index
                let reg_key = format!("case/{}/assets/{}", case_id, asset_filename);
                let _ = index.unregister(&reg_key);
                deduped_count += 1;
                bytes_saved += file_size;
            }
        }
    }

    if deduped_count > 0 {
        // Save updated manifest
        manifest.assets.total_downloaded = manifest.asset_map.len();
        write_manifest(&manifest, &case_dir)?;

        // Save updated trial_data
        if trial_data_modified {
            if let Some(td) = &trial_data {
                let json = serde_json::to_string_pretty(td)
                    .map_err(|e| format!("Failed to serialize trial_data: {}", e))?;
                fs::write(&trial_data_path, json)
                    .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;
            }
        }
    }

    Ok((deduped_count, bytes_saved))
}

/// Clear default assets that are not referenced by any case manifest.
/// Scans all manifests to build a set of used defaults/ paths, then deletes the rest.
/// Returns (files_deleted, bytes_freed).
pub fn clear_unused_defaults(data_dir: &Path) -> Result<(usize, u64), String> {
    // Collect all defaults/ paths referenced by any case manifest
    let mut used_defaults: std::collections::HashSet<String> = std::collections::HashSet::new();
    let cases_dir = data_dir.join("case");
    if cases_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&cases_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Ok(manifest) = read_manifest(&path) {
                    for local_path in manifest.asset_map.values() {
                        if local_path.starts_with("defaults/") {
                            used_defaults.insert(normalize_path(local_path));
                        }
                    }
                }
            }
        }
    }

    // Walk defaults/ and delete files not in the used set
    let defaults_dir = data_dir.join("defaults");
    if !defaults_dir.is_dir() {
        return Ok((0, 0));
    }

    // Open index to unregister deleted files
    let index = DedupIndex::open(data_dir).ok();

    let mut deleted_count = 0usize;
    let mut bytes_freed = 0u64;

    fn walk_and_clean(
        dir: &std::path::Path,
        base_dir: &std::path::Path,
        used: &std::collections::HashSet<String>,
        index: Option<&DedupIndex>,
        deleted: &mut usize,
        freed: &mut u64,
    ) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_and_clean(&path, base_dir, used, index, deleted, freed);
                // Remove empty directories
                let _ = fs::remove_dir(&path);
            } else if path.is_file() {
                let relative = match path.strip_prefix(base_dir) {
                    Ok(r) => normalize_path(&r.to_string_lossy()),
                    Err(_) => continue,
                };
                if !used.contains(&relative) {
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    if fs::remove_file(&path).is_ok() {
                        // Unregister from persistent index
                        if let Some(idx) = index {
                            let _ = idx.unregister(&relative);
                        }
                        *deleted += 1;
                        *freed += size;
                    }
                }
            }
        }
    }

    walk_and_clean(&defaults_dir, data_dir, &used_defaults, index.as_ref(), &mut deleted_count, &mut bytes_freed);

    Ok((deleted_count, bytes_freed))
}

/// List all case directories under `data_dir/case/` with parseable numeric IDs.
pub fn list_case_dirs(data_dir: &Path) -> Result<Vec<(u32, std::path::PathBuf)>, String> {
    let cases_dir = data_dir.join("case");
    if !cases_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let entries = fs::read_dir(&cases_dir)
        .map_err(|e| format!("Failed to read case directory: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if let Ok(id) = name.parse::<u32>() {
                result.push((id, path));
            }
        }
    }
    result.sort_by_key(|(id, _)| *id);
    Ok(result)
}
