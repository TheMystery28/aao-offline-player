use std::fs;
use std::path::Path;

use serde_json::Value;

use super::helpers::{hash_file, normalize_ext, rewrite_value_recursive};
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

/// Dedup a single case's assets against all indexed files (defaults + other cases).
/// Opens its own DedupIndex. For use from download/import pipelines.
pub fn dedup_case_assets(case_id: u32, data_dir: &Path) -> Result<(usize, u64), String> {
    let case_dir = data_dir.join("case").join(case_id.to_string());
    let assets_dir = case_dir.join("assets");
    if !assets_dir.is_dir() {
        return Ok((0, 0));
    }
    let index = DedupIndex::open(data_dir)?;
    // Scan both defaults and case assets so cross-case lookups work
    index.scan_and_register(data_dir, "defaults")?;
    index.scan_and_register_cases(data_dir)?;
    dedup_case_assets_with_index(case_id, data_dir, &index)
}

/// Dedup a single case's assets using a pre-opened index.
/// Matches against ALL indexed files (defaults + other cases).
/// Cross-case matches are promoted to defaults/shared/ and the other case is rewritten.
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

    // Read manifest (if missing, nothing to dedup — manifest tracks what URLs point where)
    let mut manifest = match read_manifest(&case_dir) {
        Ok(m) => m,
        Err(_) => return Ok((0, 0)),
    };

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
        let asset_filename = match file_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Compute hash for index lookup
        let content_hash = match hash_file(&file_path) {
            Ok(h) => h,
            Err(_) => continue,
        };
        let ext = file_path.extension()
            .and_then(|e| e.to_str())
            .map(|e| normalize_ext(e))
            .unwrap_or_default();

        // Exclude self from matches
        let self_reg_key = format!("case/{}/assets/{}", case_id, asset_filename);
        let match_path = match index.find_by_hash(file_size, &ext, content_hash, Some(&self_reg_key)) {
            Some(p) => p,
            None => continue, // No duplicate found
        };

        // Determine target path: use defaults/ directly, or promote case/ to shared
        let target_path = if match_path.starts_with("defaults/") {
            // Verify the default file actually exists on disk
            if !data_dir.join(&match_path).is_file() {
                continue;
            }
            match_path
        } else {
            // Cross-case match — promote to defaults/shared/
            let source = data_dir.join(&match_path);
            if !source.is_file() {
                continue;
            }
            let shared_path = promote_to_shared(data_dir, &source, content_hash, index)?;
            // Rewrite the other case to point to the shared copy
            rewrite_other_case(data_dir, &match_path, &shared_path, index)?;
            shared_path
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

        // Update manifest
        for url in &urls_to_update {
            manifest.asset_map.insert(url.clone(), target_path.clone());
        }

        // Rewrite references in trial_data.json
        if let Some(ref mut td) = trial_data {
            let old_server_path = format!("case/{}/{}", case_id, old_local_path);
            rewrite_value_recursive(td, &old_server_path, &target_path);
            trial_data_modified = true;
        }

        // Delete this case's duplicate file
        if fs::remove_file(&file_path).is_ok() {
            let _ = index.unregister(&self_reg_key);
            deduped_count += 1;
            bytes_saved += file_size;
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

/// Promote a file to defaults/shared/ using hash-based naming.
/// Idempotent: if destination already exists, skip copy but still return path.
/// Returns the new relative path (e.g., "defaults/shared/a1b2/a1b2c3d4e5f67890.png").
pub(crate) fn promote_to_shared(
    data_dir: &Path,
    source_path: &Path,
    content_hash: u64,
    index: &DedupIndex,
) -> Result<String, String> {
    let ext = source_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let hash_hex = format!("{:016x}", content_hash);
    let subdir = &hash_hex[0..4];
    let shared_relative = if ext.is_empty() {
        format!("defaults/shared/{}/{}", subdir, hash_hex)
    } else {
        format!("defaults/shared/{}/{}.{}", subdir, hash_hex, ext)
    };
    let dest = data_dir.join(&shared_relative);

    if !dest.is_file() {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create shared dir: {}", e))?;
        }
        fs::copy(source_path, &dest)
            .map_err(|e| format!("Failed to promote to shared: {}", e))?;
    }

    // Register (or re-register) the shared path in the index
    let size = dest.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = index.register(&shared_relative, size, content_hash);

    Ok(shared_relative)
}

/// Rewrite another case's manifest and trial_data to point to a promoted shared path.
/// Deletes the old file and unregisters it from the index.
/// Silently skips if the case directory or manifest doesn't exist (case may have been deleted).
pub(crate) fn rewrite_other_case(
    data_dir: &Path,
    old_case_relative: &str,  // "case/{id}/assets/{filename}"
    new_shared_path: &str,    // "defaults/shared/a1b2/a1b2c3d4.png"
    index: &DedupIndex,
) -> Result<(), String> {
    // Parse case ID from the path
    let parts: Vec<&str> = old_case_relative.splitn(4, '/').collect();
    if parts.len() < 4 || parts[0] != "case" {
        return Ok(()); // Not a case path, skip
    }
    let other_case_id: u32 = match parts[1].parse() {
        Ok(id) => id,
        Err(_) => return Ok(()),
    };
    let other_case_dir = data_dir.join("case").join(other_case_id.to_string());
    if !other_case_dir.is_dir() {
        return Ok(()); // Case was deleted
    }
    let old_local_path = format!("assets/{}", parts[3]); // "assets/filename.png"

    // Rewrite manifest
    if let Ok(mut manifest) = read_manifest(&other_case_dir) {
        let urls_to_update: Vec<String> = manifest.asset_map.iter()
            .filter(|(_, v)| **v == old_local_path)
            .map(|(k, _)| k.clone())
            .collect();
        if !urls_to_update.is_empty() {
            for url in &urls_to_update {
                manifest.asset_map.insert(url.clone(), new_shared_path.to_string());
            }
            manifest.assets.total_downloaded = manifest.asset_map.len();
            let _ = write_manifest(&manifest, &other_case_dir);
        }
    }

    // Rewrite trial_data.json
    let td_path = other_case_dir.join("trial_data.json");
    if td_path.exists() {
        if let Ok(text) = fs::read_to_string(&td_path) {
            if let Ok(mut td) = serde_json::from_str::<serde_json::Value>(&text) {
                let old_server_path = format!("case/{}/{}", other_case_id, old_local_path);
                rewrite_value_recursive(&mut td, &old_server_path, new_shared_path);
                if let Ok(json) = serde_json::to_string_pretty(&td) {
                    let _ = fs::write(&td_path, json);
                }
            }
        }
    }

    // Delete the other case's copy of the file
    let other_file = data_dir.join(old_case_relative);
    if other_file.is_file() {
        let _ = fs::remove_file(&other_file);
    }

    // Unregister old path from the index
    let _ = index.unregister(old_case_relative);

    Ok(())
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
