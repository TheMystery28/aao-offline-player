use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::downloader::dedup::{DedupIndex, check_and_promote, hash_file};
use crate::downloader::manifest::{AssetSummary, CaseManifest, write_manifest};
use crate::utils::format_timestamp;
use super::shared::*;
use super::aaoffline_helpers::*;

/// Import a case from an aaoffline download directory.
///
/// `source_dir` must contain `index.html` and optionally `assets/`.
/// The case is installed into `engine_dir/case/{case_id}/`.
pub fn import_aaoffline(
    source_dir: &Path,
    engine_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<(CaseManifest, u64), String> {
    let index_path = source_dir.join("index.html");
    if !index_path.exists() {
        return Err(format!(
            "No index.html found in {}. Expected an aaoffline download folder.",
            source_dir.display()
        ));
    }

    // Read index.html
    let html = fs::read_to_string(&index_path)
        .map_err(|e| format!("Failed to read index.html: {}", e))?;

    // 1. Extract trial_information
    let case_info = extract_trial_information(&html)?;

    // 2. Extract initial_trial_data
    let mut trial_data = extract_trial_data(&html)?;

    let case_id = case_info.id;
    let case_dir = engine_dir.join("case").join(case_id.to_string());

    // Check if case already exists
    if case_dir.join("manifest.json").exists() {
        return Err(format!(
            "Case {} already exists in your library. Delete it first if you want to reimport.",
            case_id
        ));
    }

    fs::create_dir_all(&case_dir)
        .map_err(|e| format!("Failed to create case directory: {}", e))?;

    // Open dedup index for pre-copy dedup checks
    let dedup_index = DedupIndex::open(engine_dir).ok();

    // 3. Copy assets and rewrite paths
    let source_assets = source_dir.join("assets");
    let dest_assets = case_dir.join("assets");
    let mut asset_map: HashMap<String, String> = HashMap::new();
    let mut total_size: u64 = 0;
    let mut asset_count: usize = 0;

    let mut dedup_saved_bytes: u64 = 0;

    if source_assets.is_dir() {
        fs::create_dir_all(&dest_assets)
            .map_err(|e| format!("Failed to create assets directory: {}", e))?;

        // Collect entries so we can count total for progress
        let file_entries: Vec<_> = fs::read_dir(&source_assets)
            .map_err(|e| format!("Failed to read assets directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();
        let total_files = file_entries.len();

        for entry in &file_entries {
            let src_path = entry.path();
            let filename_str = entry.file_name().to_string_lossy().to_string();
            let safe_filename = sanitize_imported_filename(&filename_str);
            let old_ref = format!("assets/{}", filename_str);

            // Check dedup index before copying — skip if identical content already exists
            if let Some(ref idx) = dedup_index {
                if let Ok(hash) = hash_file(&src_path) {
                    let size = src_path.metadata().map(|m| m.len()).unwrap_or(0);
                    if let Some(existing) = check_and_promote(engine_dir, hash, idx, None) {
                        // Duplicate found — skip copy, use existing path
                        asset_count += 1;
                        total_size += size;
                        dedup_saved_bytes += size;
                        asset_map.insert(old_ref, existing);
                        if let Some(cb) = &on_progress {
                            cb(asset_count, total_files);
                        }
                        continue;
                    }
                }
            }

            let dest_path = dest_assets.join(&safe_filename);

            // Copy file with sanitized name
            match fs::copy(&src_path, &dest_path) {
                Ok(bytes) => {
                    total_size += bytes;
                    asset_count += 1;
                    let new_ref = format!("assets/{}", safe_filename);
                    asset_map.insert(old_ref, new_ref);
                    // Register new file in dedup index
                    if let Some(ref idx) = dedup_index {
                        if let Ok(hash) = hash_file(&dest_path) {
                            let reg_key = format!("case/{}/assets/{}", case_id, safe_filename);
                            let _ = idx.register(&reg_key, bytes, hash);
                        }
                    }
                    if let Some(cb) = &on_progress {
                        cb(asset_count, total_files);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: failed to copy {}: {}",
                        src_path.display(),
                        e
                    );
                }
            }
        }
    }

    // 3b. Extract and copy default sprites from aaoffline's getDefaultSpriteUrl overrides.
    //     The aaoffline downloaders replace getDefaultSpriteUrl() with hardcoded if-statements
    //     mapping (base, sprite_id, status) → assets/{hash}.gif. We parse those mappings and
    //     copy the sprites to defaults/images/chars/{base}/{id}.gif etc. so the unmodified
    //     AAO engine can find them.
    let sprite_mappings = extract_default_sprite_mappings(&html);
    let (default_sprites_copied, default_sprites_bytes) = if !sprite_mappings.is_empty() {
        copy_default_sprites(&sprite_mappings, source_dir, engine_dir)
    } else {
        (0, 0)
    };
    total_size += default_sprites_bytes;

    // 3c. Extract and copy default voice blips from aaoffline's getVoiceUrl overrides.
    let voice_mappings = extract_voice_mappings(&html);
    let (voices_copied, voices_bytes) = if !voice_mappings.is_empty() {
        copy_voice_assets(&voice_mappings, source_dir, engine_dir)
    } else {
        (0, 0)
    };
    total_size += voices_bytes;

    // 3d. Extract and copy default place assets from aaoffline's default_places variable.
    let place_mappings = extract_default_place_mappings(&html);
    let (places_copied, places_bytes) = if !place_mappings.is_empty() {
        copy_place_assets(&place_mappings, source_dir, engine_dir)
    } else {
        (0, 0)
    };
    total_size += places_bytes;

    // 3e. Record copied default assets in the asset_map so they're included in .aaocase exports.
    for m in &sprite_mappings {
        let subdir = match m.status.as_str() {
            "talking" => "chars",
            "still" => "charsStill",
            "startup" => "charsStartup",
            _ => continue,
        };
        let local_path = format!("defaults/images/{}/{}/{}.gif", subdir, m.base, m.sprite_id);
        if engine_dir.join(&local_path).is_file() {
            asset_map.insert(local_path.clone(), local_path);
        }
    }
    for m in &voice_mappings {
        let local_path = format!("defaults/voices/voice_singleblip_{}.{}", m.voice_id, m.ext);
        if engine_dir.join(&local_path).is_file() {
            asset_map.insert(local_path.clone(), local_path);
        }
    }
    for m in &place_mappings {
        if engine_dir.join(&m.dest_path).is_file() {
            asset_map.insert(m.dest_path.clone(), m.dest_path.clone());
        }
    }

    // 4. Save trial_info.json
    let info_value = build_trial_info_json(&case_info);
    fs::write(
        case_dir.join("trial_info.json"),
        serde_json::to_string_pretty(&info_value)
            .map_err(|e| format!("Failed to serialize trial_info: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_info.json: {}", e))?;

    // 5. Build manifest FIRST — it's the single source of truth for asset paths
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let manifest = CaseManifest {
        case_id,
        title: case_info.title,
        author: case_info.author,
        language: case_info.language,
        download_date: format_timestamp(now),
        format: case_info.format,
        sequence: case_info.sequence,
        assets: AssetSummary {
            case_specific: asset_count,
            shared_defaults: default_sprites_copied + voices_copied + places_copied,
            total_downloaded: asset_count + default_sprites_copied + voices_copied + places_copied,
            total_size_bytes: total_size,
        },
        asset_map,
        failed_assets: Vec::new(),
        has_plugins: false,
        has_case_config: false,
    };

    // 6. Rewrite trial_data from manifest — derives all paths from the single source of truth
    crate::downloader::manifest::rewrite_trial_data_from_manifest(&mut trial_data, case_id, &manifest);

    // 7. Save trial_data.json and manifest
    fs::write(
        case_dir.join("trial_data.json"),
        serde_json::to_string_pretty(&trial_data)
            .map_err(|e| format!("Failed to serialize trial_data: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;
    write_manifest(&manifest, &case_dir)?;

    Ok((manifest, dedup_saved_bytes))
}

/// Check if a directory is a parent folder containing aaoffline case subfolders.
/// Returns the list of subdirectories that contain an `index.html` file.
pub fn find_aaoffline_subfolders(parent_dir: &Path) -> Vec<std::path::PathBuf> {
    let mut case_dirs = Vec::new();
    if let Ok(entries) = fs::read_dir(parent_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let sub = entry.path();
            if sub.is_dir() && sub.join("index.html").exists() {
                case_dirs.push(sub);
            }
        }
    }
    // Sort by folder name for consistent import order
    case_dirs.sort();
    case_dirs
}

/// Import all aaoffline cases from a directory that contains case subfolders.
///
/// Handles three layouts produced by aaoffline downloaders:
/// 1. Subfolders only (e.g. `Max Jefht/Episode1_id/index.html`, `Max Jefht/Episode2_id/index.html`)
/// 2. Root + subfolders (e.g. `Beyond the Shadows/index.html` + `Beyond the Shadows/Part2_id/index.html`)
///    The root case is imported first, then subfolders; duplicates (same case ID) are skipped.
/// 3. Root only — handled by the caller via `import_aaoffline()` directly.
///
/// Cases that already exist or fail are recorded in `batch_errors` but don't
/// stop the overall import.
pub fn import_aaoffline_batch(
    parent_dir: &Path,
    engine_dir: &Path,
    on_case_progress: Option<&dyn Fn(usize, usize, &str)>,
    on_asset_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<ImportResult, String> {
    let has_root_case = parent_dir.join("index.html").exists();
    let sub_dirs = find_aaoffline_subfolders(parent_dir);

    if !has_root_case && sub_dirs.is_empty() {
        return Err(format!(
            "No index.html found in {} and no subfolders with index.html found either. \
             Expected an aaoffline download folder or a parent folder containing case subfolders.",
            parent_dir.display()
        ));
    }

    // Build ordered list: root case first (if present), then subfolders
    let mut case_dirs: Vec<std::path::PathBuf> = Vec::new();
    if has_root_case {
        case_dirs.push(parent_dir.to_path_buf());
    }
    case_dirs.extend(sub_dirs);

    let total_cases = case_dirs.len();
    let mut batch_manifests: Vec<CaseManifest> = Vec::new();
    let mut batch_errors: Vec<String> = Vec::new();
    let mut imported_ids: Vec<u32> = Vec::new();
    let mut total_dedup_saved: u64 = 0;

    for (i, case_dir) in case_dirs.iter().enumerate() {
        let folder_name = case_dir.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if let Some(cb) = &on_case_progress {
            cb(i + 1, total_cases, &folder_name);
        }

        match import_aaoffline(case_dir, engine_dir, on_asset_progress) {
            Ok((manifest, dedup_bytes)) => {
                imported_ids.push(manifest.case_id);
                batch_manifests.push(manifest);
                total_dedup_saved += dedup_bytes;
            }
            Err(e) => {
                // "already exists" for a duplicate root/subfolder case is expected, not an error
                if e.contains("already exists") {
                    // Silently skip duplicates within the same batch
                } else {
                    batch_errors.push(format!("{}: {}", folder_name, e));
                }
            }
        }
    }

    if batch_manifests.is_empty() {
        return Err(format!(
            "All {} cases failed to import: {}",
            total_cases,
            batch_errors.join("; ")
        ));
    }

    // After all cases are imported, extract default sprite mappings from ALL index.html
    // files and copy sprites. We search ALL asset directories because the aaoffline
    // downloader shares sprite files across a sequence — a sprite referenced in every
    // case's index.html may only exist in one subfolder's assets/.
    let all_asset_dirs: Vec<std::path::PathBuf> = case_dirs.iter()
        .map(|d| d.join("assets"))
        .filter(|d| d.is_dir())
        .collect();

    // Collect unique sprite mappings from all index.html files
    let mut all_mappings: Vec<DefaultSpriteMapping> = Vec::new();
    for case_dir in &case_dirs {
        let index_path = case_dir.join("index.html");
        if let Ok(html) = fs::read_to_string(&index_path) {
            let mappings = extract_default_sprite_mappings(&html);
            for m in mappings {
                // Deduplicate: only add if we don't already have this (base, id, status)
                let exists = all_mappings.iter().any(|e|
                    e.base == m.base && e.sprite_id == m.sprite_id && e.status == m.status
                );
                if !exists {
                    all_mappings.push(m);
                }
            }
        }
    }

    if !all_mappings.is_empty() {
        copy_default_sprites_from_multiple_dirs(&all_mappings, &all_asset_dirs, engine_dir);
    }

    // Use the first successfully imported manifest as the "primary" result
    let first = batch_manifests[0].clone();
    Ok(ImportResult {
        manifest: first,
        saves: None,
        missing_defaults: 0,
        batch_manifests,
        batch_errors,
        dedup_saved_bytes: total_dedup_saved,
    })
}
