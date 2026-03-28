use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use crate::downloader::manifest::{CaseManifest, read_manifest, write_manifest};

// Cross-module calls (merge_plugin_param_overrides, etc.)
use super::*;
use super::shared::{ImportedCaseInfo, ImportResult, read_zip_text};

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
        let (manifest, collection) = import_collection_zip(&mut archive, &coll_json, engine_dir, on_progress)?;

        // Create the collection in the collections store
        let mut coll_data = crate::collections::load_collections(engine_dir);
        coll_data.collections.push(collection);
        crate::collections::save_collections(engine_dir, &coll_data)?;

        let missing_defaults = manifest.asset_map.values()
            .filter(|p| p.starts_with("defaults/") && !engine_dir.join(p).is_file())
            .count();

        return Ok(ImportResult { manifest, saves, missing_defaults, batch_manifests: Vec::new(), batch_errors: Vec::new() });
    }

    // Check for multi-case format: presence of sequence.json
    let manifest = if let Ok(seq_json) = read_zip_text(&mut archive, "sequence.json") {
        import_multi_case_zip(&mut archive, &seq_json, engine_dir, on_progress)?
    } else {
        // Single-case format (legacy)
        import_single_case_zip(&mut archive, engine_dir, on_progress)?
    };

    // Count missing defaults from manifest's asset_map.
    let missing_defaults = manifest.asset_map.values()
        .filter(|p| p.starts_with("defaults/") && !engine_dir.join(p).is_file())
        .count();

    Ok(ImportResult { manifest, saves, missing_defaults, batch_manifests: Vec::new(), batch_errors: Vec::new() })
}

/// Import all cases from a multi-case ZIP with sequence.json.
/// Returns the first case's manifest.
fn import_multi_case_zip(
    archive: &mut zip::ZipArchive<fs::File>,
    seq_json: &str,
    engine_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<CaseManifest, String> {
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

            // Strip the case_id prefix to get relative path
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
    let mut extracted_files: Vec<(String, std::path::PathBuf)> = Vec::new();
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
        extracted_files.push((entry_name, dest_path));
        progress_count += 1;
        if let Some(cb) = &on_progress { cb(progress_count, total_entries); }
    }

    // Register ALL extracted files in the persistent hash index
    // (defaults from above + case assets via scan_and_register_cases)
    if let Ok(index) = crate::downloader::dedup::DedupIndex::open(engine_dir) {
        for (index_key, disk_path) in &extracted_files {
            if let Ok(hash) = crate::downloader::dedup::hash_file(disk_path) {
                let size = disk_path.metadata().map(|m| m.len()).unwrap_or(0);
                let normalized_key = crate::downloader::paths::normalize_path(index_key);
                let _ = index.register(&normalized_key, size, hash);
            }
        }
        // Also register case assets that were extracted earlier
        let _ = index.scan_and_register_cases(engine_dir);
    }

    // Post-import finalization: register + dedup for each case
    crate::downloader::dedup::finalize_batch_import(&case_ids, engine_dir);
    // Re-read first manifest if dedup modified it
    if let Some(ref fm) = first_manifest {
        let case_dir = engine_dir.join("case").join(fm.case_id.to_string());
        if case_dir.join("manifest.json").exists() {
            first_manifest = Some(read_manifest(&case_dir)?);
        }
    }

    first_manifest.ok_or_else(|| "No cases were imported from the multi-case ZIP".to_string())
}

/// Import all cases from a collection ZIP with collection.json.
/// Returns the first case's manifest and the reconstructed Collection object.
fn import_collection_zip(
    archive: &mut zip::ZipArchive<fs::File>,
    coll_json: &str,
    engine_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<(CaseManifest, crate::collections::Collection), String> {
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
    let mut extracted_files: Vec<(String, std::path::PathBuf)> = Vec::new();
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
        extracted_files.push((entry_name, dest_path));
        progress_count += 1;
        if let Some(cb) = &on_progress {
            cb(progress_count, total_entries);
        }
    }

    // Register ALL extracted files in the persistent hash index
    if let Ok(index) = crate::downloader::dedup::DedupIndex::open(engine_dir) {
        for (index_key, disk_path) in &extracted_files {
            if let Ok(hash) = crate::downloader::dedup::hash_file(disk_path) {
                let size = disk_path.metadata().map(|m| m.len()).unwrap_or(0);
                let normalized_key = crate::downloader::paths::normalize_path(index_key);
                let _ = index.register(&normalized_key, size, hash);
            }
        }
        let _ = index.scan_and_register_cases(engine_dir);
    }

    // Post-import finalization: register + dedup for each case
    crate::downloader::dedup::finalize_batch_import(&case_ids, engine_dir);

    let manifest = first_manifest
        .ok_or_else(|| "No cases were imported from the collection ZIP".to_string())?;
    // Re-read if dedup modified it
    let manifest = if engine_dir.join("case").join(manifest.case_id.to_string()).join("manifest.json").exists() {
        read_manifest(&engine_dir.join("case").join(manifest.case_id.to_string()))?
    } else {
        manifest
    };

    // Build the Collection object
    let collection = crate::collections::Collection {
        id: crate::collections::generate_id(),
        title,
        items,
        created_date: crate::collections::now_iso8601(),
    };

    Ok((manifest, collection))
}


/// Import a single case from a legacy .aaocase ZIP.
fn import_single_case_zip(
    archive: &mut zip::ZipArchive<fs::File>,
    engine_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<CaseManifest, String> {
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

    // 2. Extract all files from ZIP
    //    - defaults/* entries go to engine_dir/defaults/* (shared across cases)
    //    - everything else goes to case_dir/ (case-specific)
    let total = archive.len();
    let mut extracted_files: Vec<(String, std::path::PathBuf)> = Vec::new(); // (index_key, disk_path)
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

        // Track for index registration
        let index_key = if is_default {
            entry_name.clone()
        } else {
            format!("case/{}/{}", case_id, entry_name)
        };
        extracted_files.push((index_key, dest_path.clone()));

        if let Some(cb) = &on_progress { cb(i + 1, total); }
    }

    // Register ALL extracted files in the persistent hash index
    if !extracted_files.is_empty() {
        if let Ok(index) = crate::downloader::dedup::DedupIndex::open(engine_dir) {
            for (index_key, disk_path) in &extracted_files {
                if let Ok(hash) = crate::downloader::dedup::hash_file(disk_path) {
                    let size = disk_path.metadata().map(|m| m.len()).unwrap_or(0);
                    let normalized_key = crate::downloader::paths::normalize_path(index_key);
                    let _ = index.register(&normalized_key, size, hash);
                }
            }
        }
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

    // Post-import finalization: register + dedup
    let (dedup_count, _) = crate::downloader::dedup::finalize_case_import(case_id, engine_dir);
    if dedup_count > 0 {
        manifest = read_manifest(&case_dir)?;
    }

    Ok(manifest)
}
