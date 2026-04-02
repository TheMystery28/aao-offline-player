use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::error::AppError;
use super::helpers::rewrite_value_recursive;
use super::index::DedupIndex;
use super::operations::{dedup_case_assets_with_index, list_case_dirs};
use crate::downloader::manifest::{read_manifest, write_manifest};

/// Global optimization: find assets shared across multiple cases, promote to defaults/shared/.
/// Then run single-case dedup for each case against the full defaults pool.
/// Returns total files deduplicated and bytes saved.
pub fn optimize_all_cases(
    data_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize, &str)>,
) -> Result<(usize, u64), AppError> {
    let case_dirs = list_case_dirs(data_dir)?;
    if case_dirs.is_empty() {
        return Ok((0, 0));
    }

    let total_phases = case_dirs.len() + 1; // index query + dedup per case
    let mut progress = 0usize;

    // Phase 1: Build content map from the persistent index (no file I/O)
    // Ensure index is populated for pre-existing downloads (migration)
    let index = DedupIndex::open(data_dir)?;
    index.scan_and_register(data_dir, "defaults")?;
    index.scan_and_register_cases(data_dir)?;

    type ContentKey = (u64, String, u64);
    let mut content_map: HashMap<ContentKey, Vec<(u32, String)>> = HashMap::new();

    // Query all case asset entries from the index
    let case_assets = index.query_case_assets()?;
    for (case_id, filename, size, ext, hash) in case_assets {
        let key = (size, ext, hash);
        content_map.entry(key).or_default().push((case_id, filename));
    }

    progress += 1;
    if let Some(cb) = &on_progress {
        cb(progress, total_phases, "");
    }

    let mut total_deduped = 0usize;
    let mut total_saved = 0u64;

    // Phase 2: Promote entries with 2+ occurrences to defaults/shared/
    let shared_dir = data_dir.join("defaults").join("shared");
    for ((size, ext, hash), entries) in &content_map {
        if entries.len() < 2 {
            continue;
        }

        // Determine shared path
        let shared_relative = crate::downloader::asset_paths::shared_asset_flat(*hash, ext);
        let shared_full = data_dir.join(&shared_relative);

        // Copy first available source to shared location (if not already there)
        let already_shared = shared_full.is_file();
        if !already_shared {
            let mut copied = false;
            for (case_id, filename) in entries {
                let src = data_dir
                    .join("case")
                    .join(case_id.to_string())
                    .join("assets")
                    .join(filename);
                if src.is_file() {
                    if let Err(_) = fs::create_dir_all(&shared_dir) {
                        continue;
                    }
                    if fs::copy(&src, &shared_full).is_ok() {
                        copied = true;
                        break;
                    }
                }
            }
            if !copied {
                continue; // Could not create shared copy, skip this group
            }

            // Register the new shared asset in the persistent index
            let _ = index.register(&shared_relative, *size, *hash);
        }

        // Track how many copies we delete for this group
        let mut group_deleted = 0u32;

        // Update all cases referencing this content
        for (case_id, filename) in entries {
            let case_dir = data_dir.join("case").join(case_id.to_string());
            let asset_path = case_dir.join("assets").join(filename);
            if !asset_path.is_file() {
                continue; // Already removed by a previous pass
            }

            let old_local_path = format!("assets/{}", filename);

            // Update manifest
            if let Ok(mut manifest) = read_manifest(&case_dir) {
                let urls_to_update: Vec<String> = manifest
                    .asset_map
                    .iter()
                    .filter(|(_, v)| **v == old_local_path)
                    .map(|(k, _)| k.clone())
                    .collect();

                if urls_to_update.is_empty() {
                    continue;
                }

                for url in &urls_to_update {
                    manifest
                        .asset_map
                        .insert(url.clone(), shared_relative.clone());
                }
                manifest.assets.total_downloaded = manifest.asset_map.len();
                let _ = write_manifest(&manifest, &case_dir);
            }

            // Rewrite trial_data
            let trial_data_path = case_dir.join("trial_data.json");
            if trial_data_path.exists() {
                if let Ok(text) = fs::read_to_string(&trial_data_path) {
                    if let Ok(mut td) = serde_json::from_str::<Value>(&text) {
                        let old_server_path =
                            crate::downloader::asset_paths::case_relative(*case_id, &old_local_path);
                        rewrite_value_recursive(&mut td, &old_server_path, &shared_relative);
                        if let Ok(json) = serde_json::to_string_pretty(&td) {
                            let _ = fs::write(&trial_data_path, json);
                        }
                    }
                }
            }

            // Delete the case-specific copy
            if fs::remove_file(&asset_path).is_ok() {
                // Unregister from the persistent index
                let reg_key = crate::downloader::asset_paths::case_asset(*case_id, filename);
                let _ = index.unregister(&reg_key);
                total_deduped += 1;
                group_deleted += 1;
            }
        }

        // Net savings: we deleted N copies but created 1 shared copy (if it didn't already exist).
        // So net bytes saved = (deleted * size) - (size if we created the shared copy).
        if group_deleted > 0 {
            let created_cost = if already_shared { 0 } else { *size };
            total_saved += (group_deleted as u64) * size - created_cost;
        }
    }

    // Phase 3: Run single-case dedup for each case against the full defaults pool
    // (catches assets matching existing defaults that weren't cross-case duplicates)
    // Uses _with_index to reuse the already-open index (avoids double Database::open)
    for (case_id, _) in &case_dirs {
        let (n, b) = dedup_case_assets_with_index(*case_id, data_dir, &index).unwrap_or((0, 0));
        total_deduped += n;
        total_saved += b;

        progress += 1;
        if let Some(cb) = &on_progress {
            let desc = format!("case/{}", case_id);
            cb(progress, total_phases, &desc);
        }
    }

    Ok((total_deduped, total_saved))
}
