use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use crate::downloader::manifest::{CaseManifest, read_manifest, write_manifest};
use crate::downloader::dedup::DedupIndex;

// Cross-module calls (merge_plugin_param_overrides, etc.)
use super::*;
use super::shared::{ImportResult, read_zip_text};

/// Result of writing + dedup-checking a single ZIP entry.
enum ExtractResult {
    /// File written to disk and registered in index.
    Written,
    /// File was a duplicate — deleted from disk, returns (index_key, existing_path, size).
    Deduped { index_key: String, existing_path: String, size: u64 },
    /// Skipped (JSON metadata or directory).
    Skipped,
}

/// Write a ZIP entry to disk, then check the dedup index.
/// If a duplicate exists, delete the just-written file and return Deduped.
/// If new, register in index and return Written.
/// JSON files and directories return Skipped.
fn extract_and_dedup(
    dest_path: &std::path::Path,
    entry_name: &str,
    index_key: &str,
    dedup_index: Option<&DedupIndex>,
    engine_dir: &std::path::Path,
) -> ExtractResult {
    if entry_name.ends_with(".json") {
        return ExtractResult::Skipped;
    }
    // Reject VFS pointer files from ZIP imports (defensive — shouldn't happen with fixed exporter)
    if let Some(target) = crate::downloader::vfs::read_vfs_pointer(dest_path) {
        eprintln!(
            "[IMPORT WARN] Rejected VFS pointer in ZIP: {} -> {}. Asset may be missing.",
            dest_path.display(), target
        );
        let _ = fs::remove_file(dest_path);
        return ExtractResult::Skipped;
    }
    if let Some(idx) = dedup_index {
        if let Ok(hash) = crate::downloader::dedup::hash_file(dest_path) {
            let size = dest_path.metadata().map(|m| m.len()).unwrap_or(0);
            if let Some(existing) = crate::downloader::dedup::check_and_promote(
                engine_dir, hash, idx, None,
            ) {
                if existing != index_key {
                    let _ = fs::remove_file(dest_path);
                    return ExtractResult::Deduped {
                        index_key: index_key.to_string(),
                        existing_path: existing,
                        size,
                    };
                }
            }
            let _ = idx.register(index_key, size, hash);
        }
    }
    ExtractResult::Written
}

/// Import a case from a .aaocase ZIP file.
///
/// Supports three formats:
/// - **Single-case** (legacy): `manifest.json`, `trial_data.json`, `trial_info.json`, `assets/`
/// - **Multi-case** (sequence): `sequence.json` + `{case_id}/manifest.json`, `{case_id}/...` per case
/// - **Collection**: `collection.json` + `{case_id}/manifest.json`, `{case_id}/...` per case
///
/// Returns an `ImportResult` containing the manifest and optionally any game saves.
pub fn import_aaocase_zip(zip_path: &Path, engine_dir: &Path, on_progress: Option<&dyn Fn(usize, usize)>) -> Result<ImportResult, String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid ZIP file: {}", e))?;

    let total_entries = archive.len();
    if let Some(cb) = &on_progress { cb(0, total_entries); }

    // Read saves.json if present (before consuming archive for case extraction)
    let saves = match read_zip_text(&mut archive, "saves.json") {
        Ok(text) => {
            eprintln!("[IMPORT] Found saves.json ({} bytes)", text.len());
            match serde_json::from_str::<Value>(&text) {
                Ok(val) => Some(val),
                Err(e) => {
                    eprintln!("[IMPORT] Failed to parse saves.json: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("[IMPORT] No saves.json in ZIP: {}", e);
            None
        }
    };

    // Check for collection format: presence of collection.json
    if let Ok(coll_json) = read_zip_text(&mut archive, "collection.json") {
        let output = import_collection_zip(&mut archive, &coll_json, engine_dir, on_progress)?;

        // Create the collection in the collections store
        if let Some(collection) = output.collection {
            let mut coll_data = crate::collections::load_collections(engine_dir);
            coll_data.collections.push(collection);
            crate::collections::save_collections(engine_dir, &coll_data)?;
        }

        let missing_defaults = output.manifest.asset_map.values()
            .filter(|p| p.starts_with("defaults/") && !engine_dir.join(p).is_file())
            .count();

        return Ok(ImportResult { manifest: output.manifest, saves, missing_defaults, batch_manifests: Vec::new(), batch_errors: Vec::new(), dedup_saved_bytes: output.dedup_saved_bytes });
    }

    // Check for multi-case format: presence of sequence.json
    let output = if let Ok(seq_json) = read_zip_text(&mut archive, "sequence.json") {
        import_multi_case_zip(&mut archive, &seq_json, engine_dir, on_progress)?
    } else {
        // Single-case format (legacy)
        import_single_case_zip(&mut archive, engine_dir, on_progress)?
    };

    // Count missing defaults from manifest's asset_map.
    let missing_defaults = output.manifest.asset_map.values()
        .filter(|p| p.starts_with("defaults/") && !engine_dir.join(p).is_file())
        .count();

    Ok(ImportResult { manifest: output.manifest, saves, missing_defaults, batch_manifests: Vec::new(), batch_errors: Vec::new(), dedup_saved_bytes: output.dedup_saved_bytes })
}

/// Import all cases from a multi-case ZIP with sequence.json.
/// Returns the first case's manifest.
fn import_multi_case_zip(
    archive: &mut zip::ZipArchive<fs::File>,
    seq_json: &str,
    engine_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<super::shared::ImportOutput, String> {
    let seq_value: Value = serde_json::from_str(seq_json)
        .map_err(|e| format!("Failed to parse sequence.json: {}", e))?;

    let case_list = seq_value["list"]
        .as_array()
        .ok_or("sequence.json missing 'list' array")?;

    let case_ids: Vec<u32> = case_list
        .iter()
        .filter_map(|p| p["id"].as_u64().map(|id| id as u32))
        .collect();

    if case_ids.is_empty() {
        return Err("sequence.json has empty list".to_string());
    }

    let mut first_manifest: Option<CaseManifest> = None;
    let total_entries = archive.len();
    let mut progress_count: usize = 0;

    // Open dedup index for inline dedup during extraction
    let dedup_index = crate::downloader::dedup::DedupIndex::open(engine_dir).ok();
    let mut dedup_saved_bytes: u64 = 0;
    // Track deduped assets per case for manifest rewriting: (case_id, old_index_key, new_dedup_path)
    let mut deduped_assets: Vec<(u32, String, String)> = Vec::new();

    for &case_id in &case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());

        // Skip if already exists
        if case_dir.join("manifest.json").exists() {
            if first_manifest.is_none() {
                first_manifest = Some(read_manifest(&case_dir)?);
            }
            continue;
        }

        fs::create_dir_all(&case_dir)
            .map_err(|e| format!("Failed to create case directory: {}", e))?;

        // Extract all files under {case_id}/ prefix
        let prefix = format!("{}/", case_id);
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

            let entry_name = entry.name().to_string();
            if !entry_name.starts_with(&prefix) {
                continue;
            }

            let relative = &entry_name[prefix.len()..];
            if relative.is_empty() {
                continue;
            }

            if entry.is_dir() {
                let _ = fs::create_dir_all(case_dir.join(relative));
                continue;
            }

            let dest_path = case_dir.join(relative);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory for {}: {}", entry_name, e))?;
            }

            let mut outfile = fs::File::create(&dest_path)
                .map_err(|e| format!("Failed to create {}: {}", entry_name, e))?;
            io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Failed to write {}: {}", entry_name, e))?;
            drop(outfile);

            let index_key = crate::downloader::paths::normalize_path(
                &crate::downloader::asset_paths::case_relative(case_id, relative)
            );
            match extract_and_dedup(&dest_path, relative, &index_key, dedup_index.as_ref(), engine_dir) {
                ExtractResult::Deduped { index_key, existing_path, size } => {
                    dedup_saved_bytes += size;
                    deduped_assets.push((case_id, index_key, existing_path));
                    progress_count += 1;
                    if let Some(cb) = &on_progress { cb(progress_count, total_entries); }
                    continue;
                }
                _ => {}
            }

            progress_count += 1;
            if let Some(cb) = &on_progress { cb(progress_count, total_entries); }
        }

        // Read the extracted manifest
        if case_dir.join("manifest.json").exists() {
            let manifest = read_manifest(&case_dir)?;
            if first_manifest.is_none() {
                first_manifest = Some(manifest);
            }
        }
    }

    // Extract shared default assets (defaults/ entries) to engine_dir
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

        let entry_name = entry.name().to_string();
        if !entry_name.starts_with("defaults/") {
            continue;
        }

        let dest_path = engine_dir.join(&entry_name);
        if entry.is_dir() {
            let _ = fs::create_dir_all(&dest_path);
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory for {}: {}", entry_name, e))?;
        }

        let mut outfile = fs::File::create(&dest_path)
            .map_err(|e| format!("Failed to create {}: {}", entry_name, e))?;
        io::copy(&mut entry, &mut outfile)
            .map_err(|e| format!("Failed to write {}: {}", entry_name, e))?;
        drop(outfile);

        // Register defaults in index (no dedup — defaults are canonical)
        let index_key = crate::downloader::paths::normalize_path(&entry_name);
        if let Some(ref idx) = dedup_index {
            if let Ok(hash) = crate::downloader::dedup::hash_file(&dest_path) {
                let size = dest_path.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = idx.register(&index_key, size, hash);
            }
        }

        progress_count += 1;
        if let Some(cb) = &on_progress { cb(progress_count, total_entries); }
    }

    // Rewrite manifests and trial_data for deduped case assets
    for &case_id in &case_ids {
        let case_deduped: Vec<&(u32, String, String)> = deduped_assets.iter()
            .filter(|(cid, _, _)| *cid == case_id)
            .collect();
        if case_deduped.is_empty() {
            continue;
        }
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if let Ok(mut manifest) = read_manifest(&case_dir) {
            for (_, old_key, new_path) in &case_deduped {
                let old_local = old_key.strip_prefix(&crate::downloader::asset_paths::case_prefix(case_id))
                    .unwrap_or(old_key);
                let urls: Vec<String> = manifest.asset_map.iter()
                    .filter(|(_, v)| v.as_str() == old_local)
                    .map(|(k, _)| k.clone()).collect();
                for url in urls {
                    manifest.asset_map.insert(url, new_path.to_string());
                }
            }
            manifest.assets.total_downloaded = manifest.asset_map.len();
            let _ = write_manifest(&manifest, &case_dir);

            let td_path = case_dir.join("trial_data.json");
            if td_path.exists() {
                if let Ok(text) = fs::read_to_string(&td_path) {
                    if let Ok(mut td) = serde_json::from_str::<Value>(&text) {
                        for (cid, old_key, new_path) in &case_deduped {
                            let old_local = old_key.strip_prefix(&crate::downloader::asset_paths::case_prefix(*cid))
                                .unwrap_or(old_key);
                            let old_server = crate::downloader::asset_paths::case_relative(*cid, old_local);
                            crate::downloader::dedup::rewrite_value_recursive(&mut td, &old_server, new_path);
                        }
                        if let Ok(json) = serde_json::to_string_pretty(&td) {
                            let _ = fs::write(&td_path, json);
                        }
                    }
                }
            }

            // Update first_manifest if this was the first case
            if let Some(ref fm) = first_manifest {
                if fm.case_id == case_id {
                    first_manifest = Some(read_manifest(&case_dir)?);
                }
            }
        }
    }

    let manifest = first_manifest.ok_or_else(|| "No cases were imported from the multi-case ZIP".to_string())?;
    Ok(super::shared::ImportOutput { manifest, collection: None, dedup_saved_bytes })
}

/// Import all cases from a collection ZIP with collection.json.
/// Returns the first case's manifest and the reconstructed Collection object.
fn import_collection_zip(
    archive: &mut zip::ZipArchive<fs::File>,
    coll_json: &str,
    engine_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<super::shared::ImportOutput, String> {
    let coll_value: Value = serde_json::from_str(coll_json)
        .map_err(|e| format!("Failed to parse collection.json: {}", e))?;

    let title = coll_value["title"]
        .as_str()
        .unwrap_or("Imported Collection")
        .to_string();

    let items: Vec<crate::collections::CollectionItem> = match coll_value.get("items") {
        Some(arr) => serde_json::from_value(arr.clone()).unwrap_or_default(),
        None => Vec::new(),
    };

    // Scan the ZIP for all case directories (entries like "{case_id}/manifest.json")
    // to find every case included, regardless of whether they're standalone or in sequences.
    let mut case_ids: Vec<u32> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if let Some(prefix) = name.strip_suffix("/manifest.json") {
                if let Ok(id) = prefix.parse::<u32>() {
                    if !case_ids.contains(&id) {
                        case_ids.push(id);
                    }
                }
            }
        }
    }

    if case_ids.is_empty() {
        return Err("Collection ZIP contains no case data".to_string());
    }

    let mut first_manifest: Option<CaseManifest> = None;
    let total_entries = archive.len();
    let mut progress_count: usize = 0;

    // Open dedup index for inline dedup during extraction
    let dedup_index = crate::downloader::dedup::DedupIndex::open(engine_dir).ok();
    let mut dedup_saved_bytes: u64 = 0;
    let mut deduped_assets: Vec<(u32, String, String)> = Vec::new();

    for &case_id in &case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());

        // Skip if already exists
        if case_dir.join("manifest.json").exists() {
            if first_manifest.is_none() {
                first_manifest = Some(read_manifest(&case_dir)?);
            }
            continue;
        }

        fs::create_dir_all(&case_dir)
            .map_err(|e| format!("Failed to create case directory: {}", e))?;

        // Extract all files under {case_id}/ prefix
        let prefix = format!("{}/", case_id);
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

            let entry_name = entry.name().to_string();
            if !entry_name.starts_with(&prefix) {
                continue;
            }

            let relative = &entry_name[prefix.len()..];
            if relative.is_empty() {
                continue;
            }

            if entry.is_dir() {
                let _ = fs::create_dir_all(case_dir.join(relative));
                continue;
            }

            let dest_path = case_dir.join(relative);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory for {}: {}", entry_name, e))?;
            }

            let mut outfile = fs::File::create(&dest_path)
                .map_err(|e| format!("Failed to create {}: {}", entry_name, e))?;
            io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Failed to write {}: {}", entry_name, e))?;
            drop(outfile);

            let index_key = crate::downloader::paths::normalize_path(
                &crate::downloader::asset_paths::case_relative(case_id, relative)
            );
            match extract_and_dedup(&dest_path, relative, &index_key, dedup_index.as_ref(), engine_dir) {
                ExtractResult::Deduped { index_key, existing_path, size } => {
                    dedup_saved_bytes += size;
                    deduped_assets.push((case_id, index_key, existing_path));
                    progress_count += 1;
                    if let Some(cb) = &on_progress { cb(progress_count, total_entries); }
                    continue;
                }
                _ => {}
            }

            progress_count += 1;
            if let Some(cb) = &on_progress {
                cb(progress_count, total_entries);
            }
        }

        // Read the extracted manifest
        if case_dir.join("manifest.json").exists() {
            let manifest = read_manifest(&case_dir)?;
            if first_manifest.is_none() {
                first_manifest = Some(manifest);
            }
        }
    }

    // Extract shared default assets (defaults/ entries) to engine_dir
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

        let entry_name = entry.name().to_string();
        if !entry_name.starts_with("defaults/") {
            continue;
        }

        let dest_path = engine_dir.join(&entry_name);
        if entry.is_dir() {
            let _ = fs::create_dir_all(&dest_path);
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory for {}: {}", entry_name, e))?;
        }

        let mut outfile = fs::File::create(&dest_path)
            .map_err(|e| format!("Failed to create {}: {}", entry_name, e))?;
        io::copy(&mut entry, &mut outfile)
            .map_err(|e| format!("Failed to write {}: {}", entry_name, e))?;
        drop(outfile);

        let index_key = crate::downloader::paths::normalize_path(&entry_name);
        if let Some(ref idx) = dedup_index {
            if let Ok(hash) = crate::downloader::dedup::hash_file(&dest_path) {
                let size = dest_path.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = idx.register(&index_key, size, hash);
            }
        }

        progress_count += 1;
        if let Some(cb) = &on_progress {
            cb(progress_count, total_entries);
        }
    }

    // Rewrite manifests and trial_data for deduped case assets
    for &case_id in &case_ids {
        let case_deduped: Vec<&(u32, String, String)> = deduped_assets.iter()
            .filter(|(cid, _, _)| *cid == case_id)
            .collect();
        if case_deduped.is_empty() {
            continue;
        }
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if let Ok(mut manifest) = read_manifest(&case_dir) {
            for (_, old_key, new_path) in &case_deduped {
                let old_local = old_key.strip_prefix(&crate::downloader::asset_paths::case_prefix(case_id))
                    .unwrap_or(old_key);
                let urls: Vec<String> = manifest.asset_map.iter()
                    .filter(|(_, v)| v.as_str() == old_local)
                    .map(|(k, _)| k.clone()).collect();
                for url in urls {
                    manifest.asset_map.insert(url, new_path.to_string());
                }
            }
            manifest.assets.total_downloaded = manifest.asset_map.len();
            let _ = write_manifest(&manifest, &case_dir);

            let td_path = case_dir.join("trial_data.json");
            if td_path.exists() {
                if let Ok(text) = fs::read_to_string(&td_path) {
                    if let Ok(mut td) = serde_json::from_str::<Value>(&text) {
                        for (cid, old_key, new_path) in &case_deduped {
                            let old_local = old_key.strip_prefix(&crate::downloader::asset_paths::case_prefix(*cid))
                                .unwrap_or(old_key);
                            let old_server = crate::downloader::asset_paths::case_relative(*cid, old_local);
                            crate::downloader::dedup::rewrite_value_recursive(&mut td, &old_server, new_path);
                        }
                        if let Ok(json) = serde_json::to_string_pretty(&td) {
                            let _ = fs::write(&td_path, json);
                        }
                    }
                }
            }

            if let Some(ref fm) = first_manifest {
                if fm.case_id == case_id {
                    first_manifest = Some(read_manifest(&case_dir)?);
                }
            }
        }
    }

    let manifest = first_manifest
        .ok_or_else(|| "No cases were imported from the collection ZIP".to_string())?;

    // Build the Collection object
    let collection = crate::collections::Collection {
        id: crate::collections::generate_id(),
        title,
        items,
        created_date: crate::collections::now_iso8601(),
    };

    Ok(super::shared::ImportOutput { manifest, collection: Some(collection), dedup_saved_bytes })
}


/// Import a single case from a legacy .aaocase ZIP.
fn import_single_case_zip(
    archive: &mut zip::ZipArchive<fs::File>,
    engine_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<super::shared::ImportOutput, String> {
    // 1. Read manifest.json from ZIP to get case_id
    let manifest_json = read_zip_text(archive, "manifest.json")?;
    let zip_manifest: CaseManifest = serde_json::from_str(&manifest_json)
        .map_err(|e| format!("Failed to parse manifest.json from ZIP: {}", e))?;

    let case_id = zip_manifest.case_id;
    let case_dir = engine_dir.join("case").join(case_id.to_string());

    if case_dir.join("manifest.json").exists() {
        return Err(format!(
            "Case {} already exists in your library. Delete it first if you want to reimport.",
            case_id
        ));
    }

    fs::create_dir_all(&case_dir)
        .map_err(|e| format!("Failed to create case directory: {}", e))?;

    // Open dedup index for inline dedup during extraction
    let dedup_index = crate::downloader::dedup::DedupIndex::open(engine_dir).ok();
    let mut dedup_saved_bytes: u64 = 0;

    // 2. Extract all files from ZIP
    //    - defaults/* entries go to engine_dir/defaults/* (shared across cases)
    //    - everything else goes to case_dir/ (case-specific)
    let total = archive.len();
    let mut deduped_assets: Vec<(String, String)> = Vec::new(); // (old_index_key, new_dedup_path)
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

        let entry_name = entry.name().to_string();

        // Route defaults/ entries to engine_dir (not case_dir)
        let is_default = entry_name.starts_with("defaults/");
        let dest_path = if is_default {
            engine_dir.join(&entry_name)
        } else {
            case_dir.join(&entry_name)
        };

        // Skip directories
        if entry.is_dir() {
            let _ = fs::create_dir_all(&dest_path);
            continue;
        }

        // Write file
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory for {}: {}", entry_name, e))?;
        }

        let mut outfile = fs::File::create(&dest_path)
            .map_err(|e| format!("Failed to create {}: {}", entry_name, e))?;
        io::copy(&mut entry, &mut outfile)
            .map_err(|e| format!("Failed to write {}: {}", entry_name, e))?;
        drop(outfile);

        let index_key = if is_default {
            crate::downloader::paths::normalize_path(&entry_name)
        } else {
            crate::downloader::paths::normalize_path(&crate::downloader::asset_paths::case_relative(case_id, &entry_name))
        };
        match extract_and_dedup(&dest_path, &entry_name, &index_key, dedup_index.as_ref(), engine_dir) {
            ExtractResult::Deduped { index_key, existing_path, size } => {
                dedup_saved_bytes += size;
                deduped_assets.push((index_key, existing_path));
                if let Some(cb) = &on_progress { cb(i + 1, total); }
                continue;
            }
            _ => {}
        }

        if let Some(cb) = &on_progress { cb(i + 1, total); }
    }

    // 3. Detect plugins and case_config
    let has_plugins = case_dir.join("plugins").is_dir();
    let has_case_config = case_dir.join("case_config.json").is_file();

    // 4. Read the manifest we just extracted (or use the one from the ZIP)
    let final_manifest_path = case_dir.join("manifest.json");
    let mut manifest = if final_manifest_path.exists() {
        read_manifest(&case_dir)?
    } else {
        zip_manifest
    };
    manifest.has_plugins = has_plugins;
    manifest.has_case_config = has_case_config;
    write_manifest(&manifest, &case_dir)?;

    // Import plugin param overrides from plugin_params.json if present
    let plugin_params_path = case_dir.join("plugin_params.json");
    if plugin_params_path.exists() {
        if let Ok(text) = fs::read_to_string(&plugin_params_path) {
            if let Ok(overrides) = serde_json::from_str::<serde_json::Value>(&text) {
                merge_plugin_param_overrides(&overrides, engine_dir);
            }
        }
        // Remove the import-only file from the case dir
        let _ = fs::remove_file(&plugin_params_path);
    }

    // Rewrite manifest and trial_data for any assets deduped during extraction
    if !deduped_assets.is_empty() {
        for (old_key, new_path) in &deduped_assets {
            let old_local = old_key.strip_prefix(&crate::downloader::asset_paths::case_prefix(case_id))
                .unwrap_or(old_key);
            let urls: Vec<String> = manifest.asset_map.iter()
                .filter(|(_, v)| v.as_str() == old_local)
                .map(|(k, _)| k.clone()).collect();
            for url in urls {
                manifest.asset_map.insert(url, new_path.clone());
            }
        }
        manifest.assets.total_downloaded = manifest.asset_map.len();
        write_manifest(&manifest, &case_dir)?;

        let td_path = case_dir.join("trial_data.json");
        if td_path.exists() {
            if let Ok(text) = fs::read_to_string(&td_path) {
                if let Ok(mut td) = serde_json::from_str::<Value>(&text) {
                    for (old_key, new_path) in &deduped_assets {
                        let old_local = old_key.strip_prefix(&crate::downloader::asset_paths::case_prefix(case_id))
                            .unwrap_or(old_key);
                        let old_server = crate::downloader::asset_paths::case_relative(case_id, old_local);
                        crate::downloader::dedup::rewrite_value_recursive(&mut td, &old_server, new_path);
                    }
                    if let Ok(json) = serde_json::to_string_pretty(&td) {
                        let _ = fs::write(&td_path, json);
                    }
                }
            }
        }
    }

    Ok(super::shared::ImportOutput { manifest, collection: None, dedup_saved_bytes })
}
