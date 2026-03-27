//! Import cases from existing aaoffline downloads.
//!
//! Supports importing from the aaoffline format:
//!   source_dir/
//!   ├── index.html      (contains trial_information + initial_trial_data as inline JS)
//!   └── assets/          (all case assets with hash-suffixed filenames)
//!
//! The import:
//! 1. Parses trial_information and initial_trial_data from the inlined JS
//! 2. Rewrites asset paths from "assets/..." to "case/{id}/assets/..."
//! 3. Copies the assets/ directory
//! 4. Generates manifest.json, trial_info.json, trial_data.json

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use regex::Regex;
use serde_json::Value;

use serde::{Serialize, Deserialize};

use crate::downloader::manifest::{AssetSummary, CaseManifest, write_manifest, read_manifest};

/// Result of importing a .aaocase ZIP file.
/// Contains the manifest and optionally any game saves that were included.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub manifest: CaseManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saves: Option<Value>,
    /// Number of default assets referenced in the manifest but missing from disk.
    /// Non-zero means the .aaocase was exported without defaults (old format).
    #[serde(default)]
    pub missing_defaults: usize,
    /// For batch imports: all manifests imported (empty for single imports).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_manifests: Vec<CaseManifest>,
    /// For batch imports: errors for individual cases that failed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_errors: Vec<String>,
}

/// Metadata extracted from trial_information.
struct ImportedCaseInfo {
    id: u32,
    title: String,
    author: String,
    language: String,
    format: String,
    last_edit_date: u64,
    sequence: Option<Value>,
}

/// Import a case from an aaoffline download directory.
///
/// `source_dir` must contain `index.html` and optionally `assets/`.
/// The case is installed into `engine_dir/case/{case_id}/`.
pub fn import_aaoffline(
    source_dir: &Path,
    engine_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<CaseManifest, String> {
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

    // 3. Copy assets and rewrite paths
    let source_assets = source_dir.join("assets");
    let dest_assets = case_dir.join("assets");
    let mut asset_map: HashMap<String, String> = HashMap::new();
    let mut total_size: u64 = 0;
    let mut asset_count: usize = 0;

    // Maps original "assets/{name}" → sanitized "assets/{safe_name}" for URL rewriting
    let mut rename_map: HashMap<String, String> = HashMap::new();

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
            let dest_path = dest_assets.join(&safe_filename);

            // Copy file with sanitized name
            match fs::copy(&src_path, &dest_path) {
                Ok(bytes) => {
                    total_size += bytes;
                    asset_count += 1;
                    let old_ref = format!("assets/{}", filename_str);
                    let new_ref = format!("assets/{}", safe_filename);
                    asset_map.insert(old_ref.clone(), new_ref.clone());
                    if old_ref != new_ref {
                        rename_map.insert(old_ref, new_ref);
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

    // 4. Rewrite asset paths in trial_data:
    //    - "assets/x" → "case/{id}/assets/x" (path prefix)
    //    - Apply rename_map for sanitized filenames (e.g. "assets/a+b.mp3" → "assets/a-b.mp3")
    rewrite_imported_urls(&mut trial_data, case_id, &rename_map);

    // 5. Save trial_info.json
    let info_value = build_trial_info_json(&case_info);
    fs::write(
        case_dir.join("trial_info.json"),
        serde_json::to_string_pretty(&info_value)
            .map_err(|e| format!("Failed to serialize trial_info: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_info.json: {}", e))?;

    // 6. Save trial_data.json
    fs::write(
        case_dir.join("trial_data.json"),
        serde_json::to_string_pretty(&trial_data)
            .map_err(|e| format!("Failed to serialize trial_data: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;

    // 7. Build and save manifest
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
    write_manifest(&manifest, &case_dir)?;

    Ok(manifest)
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

    for (i, case_dir) in case_dirs.iter().enumerate() {
        let folder_name = case_dir.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if let Some(cb) = &on_case_progress {
            cb(i + 1, total_cases, &folder_name);
        }

        match import_aaoffline(case_dir, engine_dir, on_asset_progress) {
            Ok(manifest) => {
                imported_ids.push(manifest.case_id);
                batch_manifests.push(manifest);
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
    })
}

/// Export a case as a .aaocase ZIP file.
///
/// Packs `manifest.json`, `trial_info.json`, `trial_data.json`, and `assets/*`
/// from `engine_dir/case/{case_id}/` into a ZIP at `dest_path`.
///
/// If `on_progress` is provided, it is called with (completed, total) after each file.
pub fn export_aaocase(
    case_id: u32,
    engine_dir: &Path,
    dest_path: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
    saves: Option<&Value>,
    include_plugins: bool,
) -> Result<u64, String> {
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id));
    }

    let manifest_path = case_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(format!("Case {} has no manifest.json", case_id));
    }

    let file = fs::File::create(dest_path)
        .map_err(|e| format!("Failed to create ZIP file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Count total files for progress reporting
    let json_files: Vec<&str> = ["manifest.json", "trial_info.json", "trial_data.json"]
        .iter()
        .copied()
        .filter(|name| case_dir.join(name).exists())
        .collect();
    let assets_dir = case_dir.join("assets");
    let asset_files: Vec<_> = if assets_dir.is_dir() {
        fs::read_dir(&assets_dir)
            .map_err(|e| format!("Failed to read assets directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect()
    } else {
        Vec::new()
    };

    // Collect default asset paths from manifest's asset_map.
    // The download pipeline now records ALL defaults (including cached/skipped ones).
    let default_files: Vec<String> = {
        let manifest_data = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("Failed to read manifest: {}", e))?;
        let manifest: CaseManifest = serde_json::from_str(&manifest_data)
            .map_err(|e| format!("Failed to parse manifest: {}", e))?;
        manifest.asset_map.values()
            .filter(|p| p.starts_with("defaults/"))
            .filter(|p| engine_dir.join(p).is_file())
            .cloned()
            .collect()
    };

    let total = json_files.len() + asset_files.len() + default_files.len();
    let mut completed: usize = 0;

    // Add JSON metadata files
    for name in &json_files {
        let path = case_dir.join(name);
        let data = fs::read(&path)
            .map_err(|e| format!("Failed to read {}: {}", name, e))?;
        zip.start_file(*name, options)
            .map_err(|e| format!("Failed to add {} to ZIP: {}", name, e))?;
        io::Write::write_all(&mut zip, &data)
            .map_err(|e| format!("Failed to write {} to ZIP: {}", name, e))?;
        completed += 1;
        if let Some(cb) = &on_progress {
            cb(completed, total);
        }
    }

    // Add case-specific asset files
    for entry in &asset_files {
        let path = entry.path();
        let filename = entry.file_name();
        let zip_path = format!("assets/{}", filename.to_string_lossy());
        let data = fs::read(&path)
            .map_err(|e| format!("Failed to read asset {}: {}", zip_path, e))?;
        zip.start_file(&zip_path, options)
            .map_err(|e| format!("Failed to add {} to ZIP: {}", zip_path, e))?;
        io::Write::write_all(&mut zip, &data)
            .map_err(|e| format!("Failed to write {} to ZIP: {}", zip_path, e))?;
        completed += 1;
        if let Some(cb) = &on_progress {
            cb(completed, total);
        }
    }

    // Add shared default assets (sprites, backgrounds, music, sounds, voices)
    for default_path in &default_files {
        let full_path = engine_dir.join(default_path);
        let data = fs::read(&full_path)
            .map_err(|e| format!("Failed to read default asset {}: {}", default_path, e))?;
        zip.start_file(default_path.as_str(), options)
            .map_err(|e| format!("Failed to add {} to ZIP: {}", default_path, e))?;
        io::Write::write_all(&mut zip, &data)
            .map_err(|e| format!("Failed to write {} to ZIP: {}", default_path, e))?;
        completed += 1;
        if let Some(cb) = &on_progress {
            cb(completed, total);
        }
    }

    // Add plugins directory if present and requested
    let plugins_dir = case_dir.join("plugins");
    if include_plugins && plugins_dir.is_dir() {
        fn add_dir_to_zip(
            zip: &mut zip::ZipWriter<fs::File>,
            dir: &Path,
            prefix: &str,
            options: zip::write::SimpleFileOptions,
        ) -> Result<(), String> {
            for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {}", prefix, e))? {
                let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
                let path = entry.path();
                let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
                if path.is_dir() {
                    add_dir_to_zip(zip, &path, &name, options)?;
                } else if path.is_file() {
                    let data = fs::read(&path)
                        .map_err(|e| format!("Failed to read {}: {}", name, e))?;
                    zip.start_file(&name, options)
                        .map_err(|e| format!("Failed to add {} to ZIP: {}", name, e))?;
                    io::Write::write_all(zip, &data)
                        .map_err(|e| format!("Failed to write {} to ZIP: {}", name, e))?;
                }
            }
            Ok(())
        }
        let _ = add_dir_to_zip(&mut zip, &plugins_dir, "plugins", options);
    }

    // Add case_config.json if present and plugins included
    let case_config_path = case_dir.join("case_config.json");
    if include_plugins && case_config_path.is_file() {
        let data = fs::read(&case_config_path)
            .map_err(|e| format!("Failed to read case_config.json: {}", e))?;
        zip.start_file("case_config.json", options)
            .map_err(|e| format!("Failed to add case_config.json to ZIP: {}", e))?;
        io::Write::write_all(&mut zip, &data)
            .map_err(|e| format!("Failed to write case_config.json to ZIP: {}", e))?;
    }

    // Add saves.json if provided
    if let Some(saves_data) = saves {
        let saves_bytes = serde_json::to_string_pretty(saves_data)
            .map_err(|e| format!("Failed to serialize saves: {}", e))?;
        zip.start_file("saves.json", options)
            .map_err(|e| format!("Failed to add saves.json to ZIP: {}", e))?;
        io::Write::write_all(&mut zip, saves_bytes.as_bytes())
            .map_err(|e| format!("Failed to write saves.json to ZIP: {}", e))?;
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize ZIP: {}", e))?;

    // Return file size
    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get ZIP file size: {}", e))?;
    Ok(meta.len())
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

    // Post-import dedup: run for each case now that defaults/ are all extracted
    for &case_id in &case_ids {
        let _ = crate::downloader::dedup::dedup_case_assets(case_id, engine_dir);
    }
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

    // Post-import dedup for each case now that defaults/ are all extracted
    for &case_id in &case_ids {
        let _ = crate::downloader::dedup::dedup_case_assets(case_id, engine_dir);
    }

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

/// Export a collection as a .aaocase ZIP file.
///
/// ZIP format:
/// ```text
/// collection.json
/// {case_id}/manifest.json
/// {case_id}/trial_info.json
/// {case_id}/trial_data.json
/// {case_id}/assets/...
/// defaults/...
/// saves.json (optional)
/// ```
///
/// `collection.json` contains the collection metadata (title, items, created_date).
/// Each case referenced in the collection is included in the ZIP.
pub fn export_collection(
    collection: &crate::collections::Collection,
    engine_dir: &Path,
    dest_path: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
    saves: Option<&Value>,
    include_plugins: bool,
) -> Result<u64, String> {
    // Gather ALL case IDs from collection items (both standalone cases and sequence members).
    // For sequence items, scan the case/ directory to find cases whose manifest has a matching
    // sequence title.
    let mut case_ids: Vec<u32> = Vec::new();
    let cases_dir = engine_dir.join("case");
    for item in &collection.items {
        match item {
            crate::collections::CollectionItem::Case { case_id } => {
                if !case_ids.contains(case_id) {
                    case_ids.push(*case_id);
                }
            }
            crate::collections::CollectionItem::Sequence { title } => {
                // Find all cases with this sequence title
                if let Ok(entries) = fs::read_dir(&cases_dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let manifest_path = entry.path().join("manifest.json");
                        if let Ok(data) = fs::read_to_string(&manifest_path) {
                            if let Ok(manifest) = serde_json::from_str::<CaseManifest>(&data) {
                                if let Some(seq) = &manifest.sequence {
                                    if let Some(seq_title) = seq.get("title").and_then(|t| t.as_str()) {
                                        if seq_title == title && !case_ids.contains(&manifest.case_id) {
                                            case_ids.push(manifest.case_id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let file = fs::File::create(dest_path)
        .map_err(|e| format!("Failed to create ZIP file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Count total files for progress
    let mut total: usize = 1; // collection.json
    for &case_id in &case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() {
            continue;
        }
        for name in &["manifest.json", "trial_info.json", "trial_data.json"] {
            if case_dir.join(name).exists() {
                total += 1;
            }
        }
        let assets_dir = case_dir.join("assets");
        if assets_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&assets_dir) {
                total += entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_file())
                    .count();
            }
        }
    }

    let mut completed: usize = 0;

    // Write collection.json
    let coll_json = serde_json::to_string_pretty(collection)
        .map_err(|e| format!("Failed to serialize collection: {}", e))?;
    zip.start_file("collection.json", options)
        .map_err(|e| format!("Failed to add collection.json: {}", e))?;
    io::Write::write_all(&mut zip, coll_json.as_bytes())
        .map_err(|e| format!("Failed to write collection.json: {}", e))?;
    completed += 1;
    if let Some(cb) = &on_progress {
        cb(completed, total);
    }

    // Write each case's files
    for &case_id in &case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() {
            return Err(format!("Case {} not found", case_id));
        }

        let prefix = format!("{}/", case_id);

        // JSON metadata files
        for name in &["manifest.json", "trial_info.json", "trial_data.json"] {
            let path = case_dir.join(name);
            if !path.exists() {
                continue;
            }
            let data = fs::read(&path)
                .map_err(|e| format!("Failed to read {}/{}: {}", case_id, name, e))?;
            zip.start_file(format!("{}{}", prefix, name), options)
                .map_err(|e| format!("Failed to add {}{}: {}", prefix, name, e))?;
            io::Write::write_all(&mut zip, &data)
                .map_err(|e| format!("Failed to write {}{}: {}", prefix, name, e))?;
            completed += 1;
            if let Some(cb) = &on_progress {
                cb(completed, total);
            }
        }

        // Asset files
        let assets_dir = case_dir.join("assets");
        if assets_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&assets_dir) {
                for entry in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_file()) {
                    let path = entry.path();
                    let filename = entry.file_name();
                    let zip_path = format!("{}assets/{}", prefix, filename.to_string_lossy());
                    let data = fs::read(&path)
                        .map_err(|e| format!("Failed to read asset {}: {}", zip_path, e))?;
                    zip.start_file(&zip_path, options)
                        .map_err(|e| format!("Failed to add {}: {}", zip_path, e))?;
                    io::Write::write_all(&mut zip, &data)
                        .map_err(|e| format!("Failed to write {}: {}", zip_path, e))?;
                    completed += 1;
                    if let Some(cb) = &on_progress {
                        cb(completed, total);
                    }
                }
            }
        }
    }

    // Collect shared default assets from all cases' manifests (deduplicated)
    let mut seen_defaults: std::collections::HashSet<String> = std::collections::HashSet::new();
    for &case_id in &case_ids {
        let manifest_path = engine_dir
            .join("case")
            .join(case_id.to_string())
            .join("manifest.json");
        if let Ok(data) = fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<CaseManifest>(&data) {
                for path in manifest.asset_map.values() {
                    if path.starts_with("defaults/") && engine_dir.join(path).is_file() {
                        seen_defaults.insert(path.clone());
                    }
                }
            }
        }
    }
    for default_path in &seen_defaults {
        let full_path = engine_dir.join(default_path);
        if let Ok(data) = fs::read(&full_path) {
            let _ = zip.start_file(default_path.as_str(), options);
            let _ = io::Write::write_all(&mut zip, &data);
            completed += 1;
            if let Some(cb) = &on_progress {
                cb(completed, total + seen_defaults.len());
            }
        }
    }

    // Add saves.json if provided
    if let Some(saves_data) = saves {
        let saves_bytes = serde_json::to_string_pretty(saves_data)
            .map_err(|e| format!("Failed to serialize saves: {}", e))?;
        zip.start_file("saves.json", options)
            .map_err(|e| format!("Failed to add saves.json to ZIP: {}", e))?;
        io::Write::write_all(&mut zip, saves_bytes.as_bytes())
            .map_err(|e| format!("Failed to write saves.json to ZIP: {}", e))?;
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize ZIP: {}", e))?;

    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get ZIP file size: {}", e))?;
    Ok(meta.len())
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

    // Post-import dedup: remove case assets identical to shared defaults
    let (dedup_count, _) = crate::downloader::dedup::dedup_case_assets(case_id, engine_dir)
        .unwrap_or((0, 0));
    if dedup_count > 0 {
        manifest = read_manifest(&case_dir)?;
    }

    Ok(manifest)
}

/// Import a plugin from a .aaoplug ZIP file into one or more existing cases.
///
/// The .aaoplug format:
/// ```text
/// manifest.json        Plugin metadata + optional external asset URLs
/// *.js                 Plugin code files
/// assets/              Pre-bundled assets (flat folder)
/// case_config.json     Optional config overrides
/// ```
pub fn import_aaoplug(
    zip_path: &Path,
    target_case_ids: &[u32],
    engine_dir: &Path,
) -> Result<Vec<u32>, String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open .aaoplug file: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid .aaoplug file: {}", e))?;

    // Validate: manifest.json must exist
    let manifest_text = read_zip_text(&mut archive, "manifest.json")
        .map_err(|_| "Invalid .aaoplug: missing manifest.json".to_string())?;

    // Parse manifest for external assets
    let plugin_manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .unwrap_or(serde_json::Value::Null);

    let mut imported_cases = Vec::new();

    for &case_id in target_case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() {
            eprintln!("[IMPORT_PLUGIN] Case {} does not exist, skipping", case_id);
            continue;
        }

        let plugins_dir = case_dir.join("plugins");
        fs::create_dir_all(&plugins_dir)
            .map_err(|e| format!("Failed to create plugins directory for case {}: {}", case_id, e))?;

        // Extract all ZIP entries to plugins/
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

            let entry_name = entry.name().to_string();
            if entry.is_dir() {
                let dir_path = plugins_dir.join(&entry_name);
                let _ = fs::create_dir_all(&dir_path);
                continue;
            }

            let dest_path = plugins_dir.join(&entry_name);
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory for {}: {}", entry_name, e))?;
            }

            let mut outfile = fs::File::create(&dest_path)
                .map_err(|e| format!("Failed to create {}: {}", entry_name, e))?;
            io::copy(&mut entry, &mut outfile)
                .map_err(|e| format!("Failed to write {}: {}", entry_name, e))?;
        }

        // Download external assets if declared in manifest
        if let Some(assets) = plugin_manifest.get("assets") {
            if let Some(externals) = assets.get("external").and_then(|e| e.as_array()) {
                let assets_dir = plugins_dir.join("assets");
                fs::create_dir_all(&assets_dir).ok();

                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .build()
                    .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

                for ext in externals {
                    let url = ext.get("url").and_then(|u| u.as_str()).unwrap_or("");
                    let path = ext.get("path").and_then(|p| p.as_str()).unwrap_or("");
                    if url.is_empty() || path.is_empty() { continue; }

                    let dest = plugins_dir.join(path);
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent).ok();
                    }

                    match client.get(url).send() {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                if let Ok(bytes) = resp.bytes() {
                                    let _ = fs::write(&dest, &bytes);
                                    eprintln!("[IMPORT_PLUGIN] Downloaded external asset: {} → {}", url, dest.display());
                                }
                            } else {
                                eprintln!("[IMPORT_PLUGIN] Failed to download {}: HTTP {}", url, resp.status());
                            }
                        }
                        Err(e) => {
                            eprintln!("[IMPORT_PLUGIN] Failed to download {}: {}", url, e);
                        }
                    }
                }
            }
        }

        // Update case manifest
        let manifest_path = case_dir.join("manifest.json");
        if manifest_path.exists() {
            if let Ok(mut manifest) = read_manifest(&case_dir) {
                manifest.has_plugins = true;
                if plugins_dir.join("case_config.json").exists() {
                    manifest.has_case_config = true;
                }
                let _ = write_manifest(&manifest, &case_dir);
            }
        }

        imported_cases.push(case_id);
    }

    Ok(imported_cases)
}

/// Attach raw plugin JS code to one or more existing cases.
pub fn attach_plugin_code(
    code: &str,
    filename: &str,
    target_case_ids: &[u32],
    engine_dir: &Path,
) -> Result<Vec<u32>, String> {
    let mut attached_cases = Vec::new();

    for &case_id in target_case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() { continue; }

        let plugins_dir = case_dir.join("plugins");
        fs::create_dir_all(&plugins_dir)
            .map_err(|e| format!("Failed to create plugins dir: {}", e))?;

        // Write the JS file
        let dest = plugins_dir.join(filename);
        fs::write(&dest, code)
            .map_err(|e| format!("Failed to write plugin file: {}", e))?;

        // Create/update plugins manifest
        let manifest_file = plugins_dir.join("manifest.json");
        let mut scripts: Vec<String> = Vec::new();
        if manifest_file.exists() {
            if let Ok(text) = fs::read_to_string(&manifest_file) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = val.get("scripts").and_then(|s| s.as_array()) {
                        for s in arr {
                            if let Some(name) = s.as_str() {
                                scripts.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        if !scripts.contains(&filename.to_string()) {
            scripts.push(filename.to_string());
        }
        let manifest_json = serde_json::json!({ "scripts": scripts });
        fs::write(&manifest_file, serde_json::to_string_pretty(&manifest_json).unwrap())
            .map_err(|e| format!("Failed to write plugin manifest: {}", e))?;

        // Update case manifest
        if let Ok(mut case_manifest) = read_manifest(&case_dir) {
            case_manifest.has_plugins = true;
            let _ = write_manifest(&case_manifest, &case_dir);
        }

        attached_cases.push(case_id);
    }

    Ok(attached_cases)
}

/// List plugins installed for a given case.
/// Returns the parsed contents of `case/{id}/plugins/manifest.json`,
/// or `{ "scripts": [] }` if no plugins directory exists.
pub fn list_plugins(case_id: u32, engine_dir: &Path) -> Result<serde_json::Value, String> {
    let manifest_path = engine_dir
        .join("case")
        .join(case_id.to_string())
        .join("plugins")
        .join("manifest.json");
    if !manifest_path.exists() {
        return Ok(serde_json::json!({ "scripts": [] }));
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read plugin manifest: {}", e))?;
    serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse plugin manifest: {}", e))
}

/// Remove a plugin from a case by filename.
/// Deletes the JS file, updates plugins/manifest.json, and if no scripts remain,
/// sets `has_plugins = false` on the case manifest.
pub fn remove_plugin(case_id: u32, filename: &str, engine_dir: &Path) -> Result<(), String> {
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} does not exist", case_id));
    }

    let plugins_dir = case_dir.join("plugins");
    let plugin_file = plugins_dir.join(filename);
    if plugin_file.exists() {
        fs::remove_file(&plugin_file)
            .map_err(|e| format!("Failed to delete plugin file: {}", e))?;
    }

    let manifest_path = plugins_dir.join("manifest.json");
    let mut scripts_empty = true;
    if manifest_path.exists() {
        let text = fs::read_to_string(&manifest_path).unwrap_or_default();
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(arr) = val.get_mut("scripts").and_then(|s| s.as_array_mut()) {
                arr.retain(|s| s.as_str() != Some(filename));
                scripts_empty = arr.is_empty();
            }
            let _ = fs::write(
                &manifest_path,
                serde_json::to_string_pretty(&val).unwrap(),
            );
        }
    }

    if scripts_empty {
        if let Ok(mut case_manifest) = read_manifest(&case_dir) {
            case_manifest.has_plugins = false;
            let _ = write_manifest(&case_manifest, &case_dir);
        }
    }

    // Clean plugin params from case_config.json
    let config_path = case_dir.join("case_config.json");
    if config_path.exists() {
        if let Ok(text) = fs::read_to_string(&config_path) {
            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&text) {
                let plugin_name = filename.trim_end_matches(".js");
                if let Some(plugins) = config.get_mut("plugins").and_then(|p| p.as_object_mut()) {
                    plugins.remove(plugin_name);
                }
                let _ = fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap());
            }
        }
    }

    // Delete resolved_plugins.json (regenerated on next play)
    let _ = fs::remove_file(case_dir.join("resolved_plugins.json"));

    Ok(())
}

/// Toggle a plugin's enabled/disabled state in the manifest.
/// When `enabled` is false, the filename is added to the `disabled` array.
/// When `enabled` is true, the filename is removed from `disabled`.
pub fn toggle_plugin(case_id: u32, filename: &str, enabled: bool, engine_dir: &Path) -> Result<(), String> {
    let plugins_dir = engine_dir.join("case").join(case_id.to_string()).join("plugins");
    let manifest_path = plugins_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(format!("No plugin manifest for case {}", case_id));
    }

    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read plugin manifest: {}", e))?;
    let mut val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse plugin manifest: {}", e))?;

    // Ensure disabled array exists
    if val.get("disabled").is_none() {
        val.as_object_mut().unwrap().insert("disabled".to_string(), serde_json::json!([]));
    }

    let disabled = val.get_mut("disabled").unwrap().as_array_mut().unwrap();

    if enabled {
        disabled.retain(|s| s.as_str() != Some(filename));
    } else {
        if !disabled.iter().any(|s| s.as_str() == Some(filename)) {
            disabled.push(serde_json::Value::String(filename.to_string()));
        }
    }

    fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap())
        .map_err(|e| format!("Failed to write plugin manifest: {}", e))?;

    Ok(())
}

/// List global plugins from {data_dir}/plugins/manifest.json.
pub fn list_global_plugins(engine_dir: &Path) -> Result<serde_json::Value, String> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(serde_json::json!({ "scripts": [], "disabled": [] }));
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read global plugin manifest: {}", e))?;
    serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse global plugin manifest: {}", e))
}

/// Attach raw plugin JS code as a global plugin.
pub fn attach_global_plugin_code(code: &str, filename: &str, engine_dir: &Path) -> Result<(), String> {
    let plugins_dir = engine_dir.join("plugins");
    fs::create_dir_all(&plugins_dir)
        .map_err(|e| format!("Failed to create global plugins dir: {}", e))?;

    let dest = plugins_dir.join(filename);
    fs::write(&dest, code)
        .map_err(|e| format!("Failed to write global plugin file: {}", e))?;

    let manifest_file = plugins_dir.join("manifest.json");
    let mut scripts: Vec<String> = Vec::new();
    let mut disabled: Vec<String> = Vec::new();
    if manifest_file.exists() {
        if let Ok(text) = fs::read_to_string(&manifest_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(arr) = val.get("scripts").and_then(|s| s.as_array()) {
                    for s in arr {
                        if let Some(name) = s.as_str() { scripts.push(name.to_string()); }
                    }
                }
                if let Some(arr) = val.get("disabled").and_then(|s| s.as_array()) {
                    for s in arr {
                        if let Some(name) = s.as_str() { disabled.push(name.to_string()); }
                    }
                }
            }
        }
    }
    if !scripts.contains(&filename.to_string()) {
        scripts.push(filename.to_string());
    }
    let manifest_json = serde_json::json!({ "scripts": scripts, "disabled": disabled });
    fs::write(&manifest_file, serde_json::to_string_pretty(&manifest_json).unwrap())
        .map_err(|e| format!("Failed to write global plugin manifest: {}", e))?;

    Ok(())
}

/// Remove a global plugin.
pub fn remove_global_plugin(filename: &str, engine_dir: &Path) -> Result<(), String> {
    let plugins_dir = engine_dir.join("plugins");
    let plugin_file = plugins_dir.join(filename);
    if plugin_file.exists() {
        fs::remove_file(&plugin_file)
            .map_err(|e| format!("Failed to delete global plugin: {}", e))?;
    }

    let manifest_path = plugins_dir.join("manifest.json");
    if manifest_path.exists() {
        let text = fs::read_to_string(&manifest_path).unwrap_or_default();
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(arr) = val.get_mut("scripts").and_then(|s| s.as_array_mut()) {
                arr.retain(|s| s.as_str() != Some(filename));
            }
            if let Some(arr) = val.get_mut("disabled").and_then(|s| s.as_array_mut()) {
                arr.retain(|s| s.as_str() != Some(filename));
            }
            // Clean plugin params
            let plugin_name = filename.trim_end_matches(".js");
            if let Some(plugins) = val.get_mut("plugins") {
                if let Some(params) = plugins.get_mut("params").and_then(|p| p.as_object_mut()) {
                    params.remove(plugin_name);
                }
            }
            let _ = fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap());
        }
    }
    Ok(())
}

/// Toggle a global plugin's enabled/disabled state.
pub fn toggle_global_plugin(filename: &str, enabled: bool, engine_dir: &Path) -> Result<(), String> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Err("No global plugin manifest".to_string());
    }

    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read global plugin manifest: {}", e))?;
    let mut val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse global plugin manifest: {}", e))?;

    if val.get("disabled").is_none() {
        val.as_object_mut().unwrap().insert("disabled".to_string(), serde_json::json!([]));
    }
    let disabled = val.get_mut("disabled").unwrap().as_array_mut().unwrap();

    if enabled {
        disabled.retain(|s| s.as_str() != Some(filename));
    } else {
        if !disabled.iter().any(|s| s.as_str() == Some(filename)) {
            disabled.push(serde_json::Value::String(filename.to_string()));
        }
    }

    fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap())
        .map_err(|e| format!("Failed to write global plugin manifest: {}", e))?;
    Ok(())
}

/// Migrate a global plugin manifest from old format to new format.
/// Old: { "scripts": [...], "disabled": [...] }
/// New: { "scripts": [...], "plugins": { "file.js": { "scope": {...}, "params": {...} } } }
/// If `plugins` key already exists, does nothing. If manifest doesn't exist, does nothing.
pub fn migrate_global_manifest(engine_dir: &Path) -> Result<(), String> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(());
    }

    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read global manifest: {}", e))?;
    let mut val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse global manifest: {}", e))?;

    // Already migrated?
    if val.get("plugins").is_some() {
        return Ok(());
    }

    let scripts = val.get("scripts")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let disabled: Vec<String> = val.get("disabled")
        .and_then(|d| d.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let mut plugins = serde_json::Map::new();
    for script_val in &scripts {
        if let Some(script_name) = script_val.as_str() {
            let is_disabled = disabled.contains(&script_name.to_string());
            plugins.insert(script_name.to_string(), serde_json::json!({
                "scope": {
                    "all": !is_disabled,
                    "case_ids": [],
                    "sequence_titles": [],
                    "collection_ids": []
                },
                "params": {}
            }));
        }
    }

    val.as_object_mut().unwrap().insert("plugins".to_string(), serde_json::Value::Object(plugins));
    // Remove old disabled array
    val.as_object_mut().unwrap().remove("disabled");

    fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap())
        .map_err(|e| format!("Failed to write migrated manifest: {}", e))?;

    Ok(())
}

/// Resolve which global plugins should load for a given case.
/// Reads global manifest, collections, and case manifest to determine scope matches.
/// Merges params cascade: plugin defaults → global → collection → sequence → case.
/// Writes `case/{id}/resolved_plugins.json`.
pub fn resolve_plugins_for_case(case_id: u32, data_dir: &Path) -> Result<serde_json::Value, String> {
    let global_manifest_path = data_dir.join("plugins").join("manifest.json");

    // Migrate if needed
    migrate_global_manifest(data_dir)?;

    // Read global manifest
    if !global_manifest_path.exists() {
        // No global plugins — write empty resolved file
        let resolved = serde_json::json!({ "active": [], "available": [] });
        let case_dir = data_dir.join("case").join(case_id.to_string());
        if case_dir.exists() {
            let _ = fs::write(case_dir.join("resolved_plugins.json"),
                serde_json::to_string_pretty(&resolved).unwrap());
        }
        return Ok(resolved);
    }

    let manifest_text = fs::read_to_string(&global_manifest_path)
        .map_err(|e| format!("Failed to read global manifest: {}", e))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|e| format!("Failed to parse global manifest: {}", e))?;

    let scripts = manifest.get("scripts")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let plugins_config = manifest.get("plugins")
        .and_then(|p| p.as_object())
        .cloned()
        .unwrap_or_default();

    // Read case manifest for sequence info
    let case_dir = data_dir.join("case").join(case_id.to_string());
    let case_sequence_title: Option<String> = if case_dir.exists() {
        let case_manifest_path = case_dir.join("manifest.json");
        if case_manifest_path.exists() {
            let cm_text = fs::read_to_string(&case_manifest_path).ok();
            cm_text.and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v.get("sequence").and_then(|s| s.get("title")).and_then(|t| t.as_str().map(|s| s.to_string())))
        } else { None }
    } else { None };

    // Read collections to check membership
    let collections_data = crate::collections::load_collections(data_dir);
    let case_collection_ids: Vec<String> = collections_data.collections.iter()
        .filter(|c| {
            c.items.iter().any(|item| {
                match item {
                    crate::collections::CollectionItem::Case { case_id: cid } => *cid == case_id,
                    crate::collections::CollectionItem::Sequence { title } => {
                        // Check if this case's sequence title matches
                        case_sequence_title.as_deref() == Some(title.as_str())
                    }
                }
            })
        })
        .map(|c| c.id.clone())
        .collect();

    let mut active = Vec::new();
    let mut available = Vec::new();

    for script_val in &scripts {
        let script_name = match script_val.as_str() {
            Some(s) => s,
            None => continue,
        };

        let plugin_cfg = plugins_config.get(script_name);
        let scope = plugin_cfg.and_then(|p| p.get("scope"));

        let is_active = match scope {
            Some(s) => {
                let all = s.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
                if all { true }
                else {
                    let case_ids = s.get("case_ids").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    let seq_titles = s.get("sequence_titles").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                    let col_ids = s.get("collection_ids").and_then(|v| v.as_array()).cloned().unwrap_or_default();

                    let case_match = case_ids.iter().any(|id| id.as_u64() == Some(case_id as u64));
                    let seq_match = case_sequence_title.as_ref().map(|st| {
                        seq_titles.iter().any(|t| t.as_str() == Some(st.as_str()))
                    }).unwrap_or(false);
                    let col_match = col_ids.iter().any(|cid| {
                        cid.as_str().map(|s| case_collection_ids.contains(&s.to_string())).unwrap_or(false)
                    });

                    case_match || seq_match || col_match
                }
            }
            None => false,
        };

        if is_active {
            // Resolve params cascade
            let params = resolve_param_cascade(
                plugin_cfg,
                case_id,
                case_sequence_title.as_deref(),
                &case_collection_ids,
            );

            active.push(serde_json::json!({
                "script": script_name,
                "source": format!("plugins/{}", script_name),
                "params": params
            }));
        } else {
            available.push(serde_json::json!({
                "script": script_name,
                "reason": "disabled (no matching scope)"
            }));
        }
    }

    let resolved = serde_json::json!({ "active": active, "available": available });

    // Write resolved file
    if case_dir.exists() {
        fs::write(case_dir.join("resolved_plugins.json"),
            serde_json::to_string_pretty(&resolved).unwrap())
            .map_err(|e| format!("Failed to write resolved_plugins.json: {}", e))?;
    }

    Ok(resolved)
}

/// Resolve cascading params for a single plugin.
/// Merge order: params.default → by_collection → by_sequence → by_case
fn resolve_param_cascade(
    plugin_cfg: Option<&serde_json::Value>,
    case_id: u32,
    sequence_title: Option<&str>,
    collection_ids: &[String],
) -> serde_json::Value {
    let empty_obj = serde_json::json!({});
    let params = plugin_cfg
        .and_then(|p| p.get("params"))
        .unwrap_or(&empty_obj);

    let mut result = serde_json::Map::new();

    // 1. Global defaults
    if let Some(defaults) = params.get("default").and_then(|d| d.as_object()) {
        for (k, v) in defaults {
            result.insert(k.clone(), v.clone());
        }
    }

    // 2. Collection overrides (first matching collection wins for conflicts)
    if let Some(by_col) = params.get("by_collection").and_then(|bc| bc.as_object()) {
        for col_id in collection_ids {
            if let Some(overrides) = by_col.get(col_id).and_then(|o| o.as_object()) {
                for (k, v) in overrides {
                    result.insert(k.clone(), v.clone());
                }
                break; // first matching collection wins
            }
        }
    }

    // 3. Sequence overrides
    if let Some(seq_title) = sequence_title {
        if let Some(by_seq) = params.get("by_sequence").and_then(|bs| bs.as_object()) {
            if let Some(overrides) = by_seq.get(seq_title).and_then(|o| o.as_object()) {
                for (k, v) in overrides {
                    result.insert(k.clone(), v.clone());
                }
            }
        }
    }

    // 4. Case overrides
    let case_key = case_id.to_string();
    if let Some(by_case) = params.get("by_case").and_then(|bc| bc.as_object()) {
        if let Some(overrides) = by_case.get(&case_key).and_then(|o| o.as_object()) {
            for (k, v) in overrides {
                result.insert(k.clone(), v.clone());
            }
        }
    }

    serde_json::Value::Object(result)
}

/// Check if plugin code already exists somewhere (global or any case).
/// Returns list of matches with filename and location.
#[derive(Debug, Clone, Serialize)]
pub struct DuplicateMatch {
    pub filename: String,
    pub location: String,
}

/// Extract param descriptors from plugin JS source code.
/// Looks for `params: { ... }` inside an `EnginePlugins.register({...})` call,
/// converts the JS object literal to JSON, and parses it.
/// Returns None if parsing fails (graceful fallback).
pub fn extract_plugin_descriptors(code: &str) -> Option<serde_json::Value> {
    // Find the params section inside EnginePlugins.register({...})
    let params_re = regex::Regex::new(r"params\s*:\s*\{").ok()?;
    let params_match = params_re.find(code)?;
    let start = params_match.end() - 1; // position of the opening {

    // Extract the balanced brace content
    let bytes = code.as_bytes();
    let mut depth = 0;
    let mut end = start;
    for i in start..bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    if depth != 0 { return None; }

    let raw_js = &code[start..end];

    // Convert JS object literal to valid JSON:
    // 1. Quote unquoted keys (word followed by colon)
    let key_re = regex::Regex::new(r"(?m)([{,]\s*)(\w+)\s*:").ok()?;
    let quoted = key_re.replace_all(raw_js, r#"$1"$2":"#);

    // 2. Remove trailing commas before } or ]
    let trailing_re = regex::Regex::new(r",\s*([}\]])").ok()?;
    let cleaned = trailing_re.replace_all(&quoted, "$1");

    // 3. Remove single-line comments
    let comment_re = regex::Regex::new(r"//[^\n]*").ok()?;
    let no_comments = comment_re.replace_all(&cleaned, "");

    // Try to parse
    serde_json::from_str(&no_comments).ok()
}

pub fn check_plugin_duplicate(code: &str, data_dir: &Path) -> Vec<DuplicateMatch> {
    let trimmed = code.trim();
    let mut matches = Vec::new();

    // Check global plugins
    let global_dir = data_dir.join("plugins");
    if global_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&global_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("js") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if content.trim() == trimmed {
                            matches.push(DuplicateMatch {
                                filename: entry.file_name().to_string_lossy().to_string(),
                                location: "global".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Check all case plugins
    let cases_dir = data_dir.join("case");
    if cases_dir.is_dir() {
        if let Ok(case_entries) = fs::read_dir(&cases_dir) {
            for case_entry in case_entries.flatten() {
                let case_plugins_dir = case_entry.path().join("plugins");
                if case_plugins_dir.is_dir() {
                    if let Ok(plugin_entries) = fs::read_dir(&case_plugins_dir) {
                        for pe in plugin_entries.flatten() {
                            let path = pe.path();
                            if path.extension().and_then(|e| e.to_str()) == Some("js") {
                                if let Ok(content) = fs::read_to_string(&path) {
                                    if content.trim() == trimmed {
                                        let case_name = case_entry.file_name().to_string_lossy().to_string();
                                        matches.push(DuplicateMatch {
                                            filename: pe.file_name().to_string_lossy().to_string(),
                                            location: format!("case {}", case_name),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    matches
}

/// Set the scope for a global plugin in the manifest.
pub fn set_global_plugin_scope(filename: &str, scope: &serde_json::Value, engine_dir: &Path) -> Result<(), String> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Err("No global plugin manifest".to_string());
    }
    migrate_global_manifest(engine_dir)?;

    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let mut val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;

    let plugins = val.get_mut("plugins")
        .and_then(|p| p.as_object_mut())
        .ok_or_else(|| "No plugins config in manifest".to_string())?;

    let entry = plugins.entry(filename.to_string())
        .or_insert(serde_json::json!({ "scope": {}, "params": {} }));
    entry.as_object_mut().unwrap().insert("scope".to_string(), scope.clone());

    fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap())
        .map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}

/// Set params for a global plugin at a specific level.
/// level: "default", "by_case", "by_sequence", "by_collection"
/// key: the case_id, sequence_title, or collection_id (ignored for "default")
pub fn set_global_plugin_params(
    filename: &str,
    level: &str,
    key: &str,
    params: &serde_json::Value,
    engine_dir: &Path,
) -> Result<(), String> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Err("No global plugin manifest".to_string());
    }
    migrate_global_manifest(engine_dir)?;

    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let mut val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;

    let plugins = val.get_mut("plugins")
        .and_then(|p| p.as_object_mut())
        .ok_or_else(|| "No plugins config".to_string())?;

    let entry = plugins.entry(filename.to_string())
        .or_insert(serde_json::json!({ "scope": { "all": false }, "params": {} }));
    let entry_params = entry.get_mut("params")
        .and_then(|p| p.as_object_mut());
    if entry_params.is_none() {
        entry.as_object_mut().unwrap().insert("params".to_string(), serde_json::json!({}));
    }
    let entry_params = entry.get_mut("params").unwrap().as_object_mut().unwrap();

    if level == "default" {
        entry_params.insert("default".to_string(), params.clone());
    } else {
        let level_obj = entry_params.entry(level.to_string())
            .or_insert(serde_json::json!({}));
        level_obj.as_object_mut().unwrap().insert(key.to_string(), params.clone());
    }

    fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap())
        .map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}

/// Export a case's plugins as a .aaoplug ZIP file.
pub fn export_case_plugins(case_id: u32, dest_path: &Path, data_dir: &Path) -> Result<u64, String> {
    let plugins_dir = data_dir.join("case").join(case_id.to_string()).join("plugins");
    if !plugins_dir.is_dir() {
        return Err(format!("Case {} has no plugins", case_id));
    }

    let file = fs::File::create(dest_path)
        .map_err(|e| format!("Failed to create .aaoplug file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    add_dir_to_zip_recursive(&mut zip, &plugins_dir, "", options)?;

    zip.finish()
        .map_err(|e| format!("Failed to finalize .aaoplug ZIP: {}", e))?;

    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get file size: {}", e))?;
    Ok(meta.len())
}

/// Promote a case plugin to global.
pub fn promote_plugin_to_global(
    case_id: u32,
    filename: &str,
    scope: &serde_json::Value,
    engine_dir: &Path,
) -> Result<(), String> {
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    let case_plugin_path = case_dir.join("plugins").join(filename);
    if !case_plugin_path.exists() {
        return Err(format!("Plugin {} not found in case {}", filename, case_id));
    }

    // Copy to global
    let global_dir = engine_dir.join("plugins");
    fs::create_dir_all(&global_dir)
        .map_err(|e| format!("Failed to create global plugins dir: {}", e))?;
    let global_path = global_dir.join(filename);
    fs::copy(&case_plugin_path, &global_path)
        .map_err(|e| format!("Failed to copy plugin to global: {}", e))?;

    // Update global manifest
    migrate_global_manifest(engine_dir)?;
    let manifest_path = global_dir.join("manifest.json");
    let mut manifest: serde_json::Value = if manifest_path.exists() {
        let text = fs::read_to_string(&manifest_path).unwrap_or_default();
        serde_json::from_str(&text).unwrap_or(serde_json::json!({ "scripts": [], "plugins": {} }))
    } else {
        serde_json::json!({ "scripts": [], "plugins": {} })
    };

    // Add to scripts if not already there
    let scripts = manifest.get_mut("scripts").and_then(|s| s.as_array_mut()).unwrap();
    if !scripts.iter().any(|s| s.as_str() == Some(filename)) {
        scripts.push(serde_json::Value::String(filename.to_string()));
    }
    // Add plugin config with scope
    let plugins = manifest.get_mut("plugins").and_then(|p| p.as_object_mut()).unwrap();
    plugins.insert(filename.to_string(), serde_json::json!({
        "scope": scope,
        "params": {}
    }));

    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap())
        .map_err(|e| format!("Failed to write global manifest: {}", e))?;

    // Remove from case manifest
    remove_plugin(case_id, filename, engine_dir)?;

    // Delete the case file
    let _ = fs::remove_file(&case_plugin_path);

    Ok(())
}

/// Result of importing a .aaosave file.
#[derive(Debug, Serialize)]
pub struct ImportSaveResult {
    pub saves: serde_json::Value,
    pub metadata: serde_json::Value,
    pub plugins_installed: Vec<u32>,
}

/// Export saves as a .aaosave ZIP file.
///
/// ZIP format:
/// ```text
/// saves.json           Save data (required)
/// metadata.json        Export metadata (required)
/// plugins/{case_id}/   Per-case plugins (optional)
/// case_config/{id}.json Per-case config (optional)
/// ```
pub fn export_aaosave(
    case_ids: &[u32],
    saves: &serde_json::Value,
    include_plugins: bool,
    dest_path: &Path,
    engine_dir: &Path,
) -> Result<u64, String> {
    let file = fs::File::create(dest_path)
        .map_err(|e| format!("Failed to create .aaosave file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Write saves.json
    let saves_bytes = serde_json::to_string_pretty(saves)
        .map_err(|e| format!("Failed to serialize saves: {}", e))?;
    zip.start_file("saves.json", options)
        .map_err(|e| format!("Failed to add saves.json to ZIP: {}", e))?;
    io::Write::write_all(&mut zip, saves_bytes.as_bytes())
        .map_err(|e| format!("Failed to write saves.json to ZIP: {}", e))?;

    // Build metadata.json
    let mut cases_meta = Vec::new();
    let mut has_plugins = false;
    for &case_id in case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        let title = if let Ok(manifest) = read_manifest(&case_dir) {
            if include_plugins && manifest.has_plugins {
                has_plugins = true;
            }
            manifest.title
        } else {
            format!("Case {}", case_id)
        };

        let save_count = saves
            .get(case_id.to_string())
            .and_then(|v| v.as_object())
            .map(|m| m.len())
            .unwrap_or(0);

        cases_meta.push(serde_json::json!({
            "id": case_id,
            "title": title,
            "save_count": save_count
        }));
    }

    let metadata = serde_json::json!({
        "version": 1,
        "export_date": format_timestamp(std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()),
        "cases": cases_meta,
        "has_plugins": has_plugins
    });

    let metadata_bytes = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    zip.start_file("metadata.json", options)
        .map_err(|e| format!("Failed to add metadata.json to ZIP: {}", e))?;
    io::Write::write_all(&mut zip, metadata_bytes.as_bytes())
        .map_err(|e| format!("Failed to write metadata.json to ZIP: {}", e))?;

    // Add plugins and case_config if requested
    if include_plugins {
        for &case_id in case_ids {
            let case_dir = engine_dir.join("case").join(case_id.to_string());
            let plugins_dir = case_dir.join("plugins");
            if plugins_dir.is_dir() {
                let prefix = format!("plugins/{}", case_id);
                add_dir_to_zip_recursive(&mut zip, &plugins_dir, &prefix, options)?;
            }

            let config_path = case_dir.join("case_config.json");
            if config_path.is_file() {
                let data = fs::read(&config_path)
                    .map_err(|e| format!("Failed to read case_config.json: {}", e))?;
                let zip_name = format!("case_config/{}.json", case_id);
                zip.start_file(&zip_name, options)
                    .map_err(|e| format!("Failed to add {} to ZIP: {}", zip_name, e))?;
                io::Write::write_all(&mut zip, &data)
                    .map_err(|e| format!("Failed to write {} to ZIP: {}", zip_name, e))?;
            }
        }
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize .aaosave ZIP: {}", e))?;

    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get .aaosave file size: {}", e))?;
    Ok(meta.len())
}

/// Import saves from a .aaosave ZIP file.
pub fn import_aaosave(
    zip_path: &Path,
    engine_dir: &Path,
) -> Result<ImportSaveResult, String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open .aaosave file: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid .aaosave file: {}", e))?;

    // Read saves.json (required)
    let saves_text = read_zip_text(&mut archive, "saves.json")
        .map_err(|_| "Invalid .aaosave: missing saves.json".to_string())?;
    let saves: serde_json::Value = serde_json::from_str(&saves_text)
        .map_err(|e| format!("Failed to parse saves.json: {}", e))?;

    // Read metadata.json (required)
    let metadata_text = read_zip_text(&mut archive, "metadata.json")
        .map_err(|_| "Invalid .aaosave: missing metadata.json".to_string())?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata_text)
        .map_err(|e| format!("Failed to parse metadata.json: {}", e))?;

    // Collect plugins/ and case_config/ entries
    let mut plugin_entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut config_entries: Vec<(String, Vec<u8>)> = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;
        let name = entry.name().to_string();
        if entry.is_dir() { continue; }

        if name.starts_with("plugins/") {
            let mut buf = Vec::new();
            io::Read::read_to_end(&mut entry, &mut buf)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
            plugin_entries.push((name, buf));
        } else if name.starts_with("case_config/") {
            let mut buf = Vec::new();
            io::Read::read_to_end(&mut entry, &mut buf)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
            config_entries.push((name, buf));
        }
    }

    // Extract plugins if present
    let mut plugins_installed = Vec::new();
    for (zip_name, data) in &plugin_entries {
        // zip_name = "plugins/{case_id}/manifest.json" etc.
        let parts: Vec<&str> = zip_name.splitn(3, '/').collect();
        if parts.len() < 3 { continue; }
        let case_id_str = parts[1];
        let case_id: u32 = match case_id_str.parse() {
            Ok(id) => id,
            Err(_) => continue,
        };

        let case_dir = engine_dir.join("case").join(case_id_str);
        if !case_dir.exists() { continue; }

        let dest = case_dir.join("plugins").join(parts[2]);
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&dest, data);

        if !plugins_installed.contains(&case_id) {
            plugins_installed.push(case_id);
        }
    }

    // Update case manifests for plugins
    for &case_id in &plugins_installed {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if let Ok(mut manifest) = read_manifest(&case_dir) {
            manifest.has_plugins = true;
            let _ = write_manifest(&manifest, &case_dir);
        }
    }

    // Extract case_config entries
    for (zip_name, data) in &config_entries {
        // zip_name = "case_config/{case_id}.json"
        let filename = zip_name.trim_start_matches("case_config/");
        let case_id_str = filename.trim_end_matches(".json");
        let case_dir = engine_dir.join("case").join(case_id_str);
        if case_dir.exists() {
            let _ = fs::write(case_dir.join("case_config.json"), data);
        }
    }

    Ok(ImportSaveResult {
        saves,
        metadata,
        plugins_installed,
    })
}

/// Helper: recursively add a directory to a ZIP under a prefix.
fn add_dir_to_zip_recursive(
    zip: &mut zip::ZipWriter<fs::File>,
    dir: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {}", prefix, e))? {
        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
        let path = entry.path();
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
        if path.is_dir() {
            add_dir_to_zip_recursive(zip, &path, &name, options)?;
        } else if path.is_file() {
            let data = fs::read(&path)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
            zip.start_file(&name, options)
                .map_err(|e| format!("Failed to add {} to ZIP: {}", name, e))?;
            io::Write::write_all(zip, &data)
                .map_err(|e| format!("Failed to write {} to ZIP: {}", name, e))?;
        }
    }
    Ok(())
}

/// Export multiple cases (a sequence) as a single .aaocase ZIP file.
///
/// ZIP format:
/// ```text
/// sequence.json
/// {case_id}/manifest.json
/// {case_id}/trial_info.json
/// {case_id}/trial_data.json
/// {case_id}/assets/...
/// ```
pub fn export_sequence(
    case_ids: &[u32],
    sequence_title: &str,
    sequence_list: &Value,
    engine_dir: &Path,
    dest_path: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
    saves: Option<&Value>,
    include_plugins: bool,
) -> Result<u64, String> {
    let file = fs::File::create(dest_path)
        .map_err(|e| format!("Failed to create ZIP file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Count total files for progress
    let mut total: usize = 1; // sequence.json
    for &case_id in case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() {
            continue;
        }
        // Count JSON files + assets
        for name in &["manifest.json", "trial_info.json", "trial_data.json"] {
            if case_dir.join(name).exists() {
                total += 1;
            }
        }
        let assets_dir = case_dir.join("assets");
        if assets_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&assets_dir) {
                total += entries.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).count();
            }
        }
    }

    let mut completed: usize = 0;

    // Write sequence.json
    let seq_json = serde_json::json!({
        "title": sequence_title,
        "list": sequence_list
    });
    zip.start_file("sequence.json", options)
        .map_err(|e| format!("Failed to add sequence.json: {}", e))?;
    io::Write::write_all(&mut zip, serde_json::to_string_pretty(&seq_json).unwrap().as_bytes())
        .map_err(|e| format!("Failed to write sequence.json: {}", e))?;
    completed += 1;
    if let Some(cb) = &on_progress {
        cb(completed, total);
    }

    // Write each case's files
    for &case_id in case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() {
            return Err(format!("Case {} not found", case_id));
        }

        let prefix = format!("{}/", case_id);

        // JSON metadata files
        for name in &["manifest.json", "trial_info.json", "trial_data.json"] {
            let path = case_dir.join(name);
            if !path.exists() {
                continue;
            }
            let data = fs::read(&path)
                .map_err(|e| format!("Failed to read {}/{}: {}", case_id, name, e))?;
            zip.start_file(format!("{}{}", prefix, name), options)
                .map_err(|e| format!("Failed to add {}{}: {}", prefix, name, e))?;
            io::Write::write_all(&mut zip, &data)
                .map_err(|e| format!("Failed to write {}{}: {}", prefix, name, e))?;
            completed += 1;
            if let Some(cb) = &on_progress {
                cb(completed, total);
            }
        }

        // Asset files
        let assets_dir = case_dir.join("assets");
        if assets_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&assets_dir) {
                for entry in entries.filter_map(|e| e.ok()).filter(|e| e.path().is_file()) {
                    let path = entry.path();
                    let filename = entry.file_name();
                    let zip_path = format!("{}assets/{}", prefix, filename.to_string_lossy());
                    let data = fs::read(&path)
                        .map_err(|e| format!("Failed to read asset {}: {}", zip_path, e))?;
                    zip.start_file(&zip_path, options)
                        .map_err(|e| format!("Failed to add {}: {}", zip_path, e))?;
                    io::Write::write_all(&mut zip, &data)
                        .map_err(|e| format!("Failed to write {}: {}", zip_path, e))?;
                    completed += 1;
                    if let Some(cb) = &on_progress {
                        cb(completed, total);
                    }
                }
            }
        }
    }

    // Collect shared default assets from all cases' manifests (deduplicated).
    // The download pipeline now records ALL defaults (including cached/skipped ones).
    let mut seen_defaults: std::collections::HashSet<String> = std::collections::HashSet::new();
    for &case_id in case_ids {
        let manifest_path = engine_dir.join("case").join(case_id.to_string()).join("manifest.json");
        if let Ok(data) = fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<CaseManifest>(&data) {
                for path in manifest.asset_map.values() {
                    if path.starts_with("defaults/") && engine_dir.join(path).is_file() {
                        seen_defaults.insert(path.clone());
                    }
                }
            }
        }
    }
    for default_path in &seen_defaults {
        let full_path = engine_dir.join(default_path);
        if let Ok(data) = fs::read(&full_path) {
            let _ = zip.start_file(default_path.as_str(), options);
            let _ = io::Write::write_all(&mut zip, &data);
            completed += 1;
            if let Some(cb) = &on_progress {
                cb(completed, total + seen_defaults.len());
            }
        }
    }

    // Add saves.json if provided
    if let Some(saves_data) = saves {
        let saves_bytes = serde_json::to_string_pretty(saves_data)
            .map_err(|e| format!("Failed to serialize saves: {}", e))?;
        zip.start_file("saves.json", options)
            .map_err(|e| format!("Failed to add saves.json to ZIP: {}", e))?;
        io::Write::write_all(&mut zip, saves_bytes.as_bytes())
            .map_err(|e| format!("Failed to write saves.json to ZIP: {}", e))?;
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize ZIP: {}", e))?;

    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get ZIP file size: {}", e))?;
    Ok(meta.len())
}

/// A single default sprite mapping extracted from the aaoffline `getDefaultSpriteUrl` override.
struct DefaultSpriteMapping {
    base: String,      // e.g. "Phoenix"
    sprite_id: u32,    // e.g. 1
    status: String,    // "talking", "still", or "startup"
    asset_path: String, // e.g. "assets/1-18236344477825908183.gif"
}

/// Parse the overridden `getDefaultSpriteUrl` function from an aaoffline index.html.
///
/// The aaoffline downloaders replace the function body with hardcoded if-statements:
/// ```js
/// if (base === 'Phoenix' && sprite_id === 1 && status === 'talking') return 'assets/1-xxx.gif';
/// ```
fn extract_default_sprite_mappings(html: &str) -> Vec<DefaultSpriteMapping> {
    let re = Regex::new(
        r"if\s*\(base\s*===\s*'([^']+)'\s*&&\s*sprite_id\s*===\s*(\d+)\s*&&\s*status\s*===\s*'([^']+)'\)\s*return\s*'([^']+)'"
    ).unwrap();

    re.captures_iter(html)
        .map(|cap| DefaultSpriteMapping {
            base: cap[1].to_string(),
            sprite_id: cap[2].parse().unwrap_or(0),
            status: cap[3].to_string(),
            asset_path: cap[4].to_string(),
        })
        .collect()
}

/// Copy default sprite assets from the aaoffline `assets/` folder to the engine's `defaults/` tree.
///
/// Maps status → subdirectory:
/// - "talking"  → `defaults/images/chars/{base}/{id}.gif`
/// - "still"    → `defaults/images/charsStill/{base}/{id}.gif`
/// - "startup"  → `defaults/images/charsStartup/{base}/{id}.gif`
fn copy_default_sprites(
    mappings: &[DefaultSpriteMapping],
    source_dir: &Path,
    engine_dir: &Path,
) -> (usize, u64) {
    let mut copied = 0usize;
    let mut bytes = 0u64;

    for m in mappings {
        let subdir = match m.status.as_str() {
            "talking" => "chars",
            "still" => "charsStill",
            "startup" => "charsStartup",
            _ => continue,
        };

        let dest_dir = engine_dir.join("defaults").join("images").join(subdir).join(&m.base);
        let dest_file = dest_dir.join(format!("{}.gif", m.sprite_id));

        if dest_file.exists() {
            continue;
        }

        let src_file = source_dir.join(&m.asset_path);
        if !src_file.exists() {
            continue;
        }

        if fs::create_dir_all(&dest_dir).is_err() {
            continue;
        }

        if let Ok(b) = fs::copy(&src_file, &dest_file) {
            copied += 1;
            bytes += b;
        }
    }

    (copied, bytes)
}

/// Copy default sprites searching across multiple asset directories.
///
/// The aaoffline downloader shares sprite files across a sequence — each case's index.html
/// references the same hash filenames, but a given sprite file may only exist in one
/// subfolder's `assets/` directory. This function tries all provided asset directories
/// to find each sprite file.
fn copy_default_sprites_from_multiple_dirs(
    mappings: &[DefaultSpriteMapping],
    asset_dirs: &[std::path::PathBuf],
    engine_dir: &Path,
) -> (usize, u64) {
    let mut copied = 0usize;
    let mut bytes = 0u64;

    for m in mappings {
        let subdir = match m.status.as_str() {
            "talking" => "chars",
            "still" => "charsStill",
            "startup" => "charsStartup",
            _ => continue,
        };

        let dest_dir = engine_dir.join("defaults").join("images").join(subdir).join(&m.base);
        let dest_file = dest_dir.join(format!("{}.gif", m.sprite_id));

        if dest_file.exists() {
            continue;
        }

        // The asset_path is "assets/{hash}.gif" — strip the "assets/" prefix to get filename
        let filename = m.asset_path.strip_prefix("assets/").unwrap_or(&m.asset_path);

        // Search across all asset directories for this file
        let src_file = asset_dirs.iter()
            .map(|dir| dir.join(filename))
            .find(|p| p.exists());

        let src_file = match src_file {
            Some(f) => f,
            None => continue,
        };

        if let Err(_) = fs::create_dir_all(&dest_dir) {
            continue;
        }

        match fs::copy(&src_file, &dest_file) {
            Ok(b) => {
                copied += 1;
                bytes += b;
            }
            Err(_) => {}
        }
    }

    (copied, bytes)
}

/// A voice blip mapping from the aaoffline `getVoiceUrl` override.
struct VoiceMapping {
    voice_id: u32,    // e.g. 1 (absolute value)
    ext: String,      // "opus", "wav", or "mp3"
    asset_path: String,
}

/// Parse the overridden `getVoiceUrl` from an aaoffline index.html.
/// Format: `if (-voice_id === 1 && ext === 'opus') return 'assets/voice_singleblip_1-xxx.opus';`
fn extract_voice_mappings(html: &str) -> Vec<VoiceMapping> {
    let re = Regex::new(
        r"if\s*\(-voice_id\s*===\s*(\d+)\s*&&\s*ext\s*===\s*'([^']+)'\)\s*return\s*'([^']+)'"
    ).unwrap();

    re.captures_iter(html)
        .map(|cap| VoiceMapping {
            voice_id: cap[1].parse().unwrap_or(0),
            ext: cap[2].to_string(),
            asset_path: cap[3].to_string(),
        })
        .collect()
}

/// Copy voice blip assets to `defaults/voices/`.
fn copy_voice_assets(mappings: &[VoiceMapping], source_dir: &Path, engine_dir: &Path) -> (usize, u64) {
    let mut copied = 0usize;
    let mut bytes = 0u64;
    let dest_dir = engine_dir.join("defaults").join("voices");

    for m in mappings {
        let dest_file = dest_dir.join(format!("voice_singleblip_{}.{}", m.voice_id, m.ext));
        if dest_file.exists() { continue; }

        let src_file = source_dir.join(&m.asset_path);
        if !src_file.exists() { continue; }

        if fs::create_dir_all(&dest_dir).is_err() { continue; }
        if let Ok(b) = fs::copy(&src_file, &dest_file) {
            copied += 1;
            bytes += b;
        }
    }
    (copied, bytes)
}

/// A default place asset mapping from the aaoffline `default_places` variable.
struct PlaceAssetMapping {
    /// Original engine path (e.g. "defaults/images/defaultplaces/backgrounds/aj_courtroom.jpg")
    dest_path: String,
    /// Source path in aaoffline assets (e.g. "assets/aj_courtroom-16637394819900123171.jpg")
    asset_path: String,
}

/// Parse the overridden `default_places` variable from an aaoffline index.html.
/// Extracts image paths that point to `assets/` (downloaded) rather than `Ressources/` (remote).
fn extract_default_place_mappings(html: &str) -> Vec<PlaceAssetMapping> {
    let mut mappings = Vec::new();

    // Find all "image":"assets/..." references in the default_places JSON
    let re = Regex::new(
        r#""image"\s*:\s*"(assets/([^"]+))"#
    ).unwrap();

    for cap in re.captures_iter(html) {
        let asset_path = cap[1].to_string(); // "assets/aj_courtroom-123.jpg"
        let filename_with_hash = &cap[2];     // "aj_courtroom-123.jpg"

        // Determine if this is a background or foreground object based on the filename
        // Background filenames: aj_courtroom, pw_judge, pw_court_still, etc.
        // Foreground filenames: aj_courtroom_benches, pw_courtroom_benches, pw_detention_center (glass), aj_judge_bench
        let is_foreground = filename_with_hash.contains("_benches")
            || filename_with_hash.contains("_glass")
            || filename_with_hash.contains("_bench-")
            || (filename_with_hash.contains("detention_center-") && filename_with_hash.ends_with(".gif"));

        let subdir = if is_foreground { "foreground_objects" } else { "backgrounds" };

        // Extract the base name without the hash: "aj_courtroom-123.jpg" → "aj_courtroom.jpg"
        // The hash is the last `-{digits}.ext` part
        let base_name = strip_aaoffline_hash(filename_with_hash);

        let dest_path = format!("defaults/images/defaultplaces/{}/{}", subdir, base_name);
        mappings.push(PlaceAssetMapping { dest_path, asset_path });
    }

    mappings
}

/// Strip the aaoffline hash suffix from a filename.
/// "aj_courtroom-16637394819900123171.jpg" → "aj_courtroom.jpg"
fn strip_aaoffline_hash(filename: &str) -> String {
    if let Some(dot_pos) = filename.rfind('.') {
        let name_part = &filename[..dot_pos];
        let ext = &filename[dot_pos..];
        if let Some(dash_pos) = name_part.rfind('-') {
            let after_dash = &name_part[dash_pos + 1..];
            if after_dash.chars().all(|c| c.is_ascii_digit()) && !after_dash.is_empty() {
                return format!("{}{}", &name_part[..dash_pos], ext);
            }
        }
        filename.to_string()
    } else {
        filename.to_string()
    }
}

/// Copy default place assets to `defaults/images/defaultplaces/`.
fn copy_place_assets(mappings: &[PlaceAssetMapping], source_dir: &Path, engine_dir: &Path) -> (usize, u64) {
    let mut copied = 0usize;
    let mut bytes = 0u64;

    for m in mappings {
        let dest_file = engine_dir.join(&m.dest_path);
        if dest_file.exists() { continue; }

        let src_file = source_dir.join(&m.asset_path);
        if !src_file.exists() { continue; }

        if let Some(parent) = dest_file.parent() {
            if fs::create_dir_all(parent).is_err() { continue; }
        }
        if let Ok(b) = fs::copy(&src_file, &dest_file) {
            copied += 1;
            bytes += b;
        }
    }
    (copied, bytes)
}

/// Read a text file from inside a ZIP archive.
fn read_zip_text(archive: &mut zip::ZipArchive<fs::File>, name: &str) -> Result<String, String> {
    let mut entry = archive.by_name(name)
        .map_err(|_| format!("ZIP does not contain '{}'. Is this a valid .aaocase file?", name))?;
    let mut contents = String::new();
    io::Read::read_to_string(&mut entry, &mut contents)
        .map_err(|e| format!("Failed to read '{}' from ZIP: {}", name, e))?;
    Ok(contents)
}

/// Extract `var trial_information = {...};` from the HTML.
fn extract_trial_information(html: &str) -> Result<ImportedCaseInfo, String> {
    let re = Regex::new(r"var\s+trial_information\s*=\s*(\{[^;]*\})\s*;")
        .map_err(|e| format!("Regex error: {}", e))?;

    let caps = re
        .captures(html)
        .ok_or("Could not find 'var trial_information = {...}' in index.html. Is this an aaoffline download?")?;

    let json_str = caps.get(1).unwrap().as_str();
    let value: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse trial_information JSON: {}", e))?;

    let id = value["id"]
        .as_u64()
        .ok_or("trial_information missing 'id' field")? as u32;
    let title = value["title"]
        .as_str()
        .unwrap_or("Unknown Title")
        .to_string();
    let author = value["author"]
        .as_str()
        .unwrap_or("Unknown Author")
        .to_string();
    let language = value["language"]
        .as_str()
        .unwrap_or("en")
        .to_string();
    let format = value["format"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let last_edit_date = value["last_edit_date"].as_u64().unwrap_or(0);
    let sequence = value.get("sequence").cloned().filter(|v| !v.is_null());

    Ok(ImportedCaseInfo {
        id,
        title,
        author,
        language,
        format,
        last_edit_date,
        sequence,
    })
}

/// Extract `var initial_trial_data = {...};` from the HTML.
///
/// The trial_data JSON can be very large (several MB) and may contain nested
/// braces, so we use a brace-counting approach instead of a simple regex.
fn extract_trial_data(html: &str) -> Result<Value, String> {
    let marker = "var initial_trial_data = ";
    let start = html
        .find(marker)
        .ok_or("Could not find 'var initial_trial_data = ' in index.html.")?;

    let json_start = start + marker.len();
    let bytes = html.as_bytes();

    // Find the matching closing brace
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;
    let mut end_pos = json_start;

    for (i, &b) in bytes[json_start..].iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match b {
            b'\\' if in_string => {
                escape_next = true;
            }
            b'"' => {
                in_string = !in_string;
            }
            b'{' if !in_string => {
                depth += 1;
            }
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    end_pos = json_start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err("Malformed initial_trial_data: unbalanced braces.".to_string());
    }

    let json_str = &html[json_start..end_pos];
    serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse initial_trial_data JSON: {}", e))
}

/// Rewrite asset paths in trial_data from "assets/..." to "case/{id}/assets/...".
///
/// This walks all string values in the JSON and rewrites any that start with "assets/".
/// Also applies filename renames from the `rename_map` (original → sanitized).
fn rewrite_imported_urls(value: &mut Value, case_id: u32, rename_map: &HashMap<String, String>) {
    match value {
        Value::String(s) => {
            if s.starts_with("assets/") {
                // Apply filename rename if needed (e.g. "assets/a+b.mp3" → "assets/a-b.mp3")
                let after_rename = rename_map.get(s.as_str())
                    .cloned()
                    .unwrap_or_else(|| s.clone());
                *s = format!("case/{}/{}", case_id, after_rename);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                rewrite_imported_urls(item, case_id, rename_map);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                rewrite_imported_urls(v, case_id, rename_map);
            }
        }
        _ => {}
    }
}

/// Sanitize a filename for safe use in URLs and on disk.
///
/// Uses the same character policy as `generate_filename()` in asset_downloader:
/// only alphanumeric, `-`, `_` are allowed in the name part. Everything else → `-`.
/// The extension (after the last `.`) is preserved and lowercased.
fn sanitize_imported_filename(filename: &str) -> String {
    let (name, ext) = match filename.rfind('.') {
        Some(pos) => (&filename[..pos], Some(&filename[pos + 1..])),
        None => (filename, None),
    };

    let sanitized_name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    match ext {
        Some(e) => format!("{}.{}", sanitized_name, e.to_lowercase()),
        None => sanitized_name,
    }
}

/// Build trial_info.json value from extracted case info.
fn build_trial_info_json(info: &ImportedCaseInfo) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("id".into(), Value::Number(info.id.into()));
    map.insert("title".into(), Value::String(info.title.clone()));
    map.insert("author".into(), Value::String(info.author.clone()));
    map.insert("language".into(), Value::String(info.language.clone()));
    map.insert("format".into(), Value::String(info.format.clone()));
    map.insert(
        "last_edit_date".into(),
        Value::Number(info.last_edit_date.into()),
    );
    map.insert(
        "sequence".into(),
        info.sequence.clone().unwrap_or(Value::Null),
    );
    Value::Object(map)
}

use crate::utils::format_timestamp;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extract_trial_information_basic() {
        let html = r#"<html>
<script>
var trial_information = {"author":"TestUser","author_id":123,"can_read":true,"can_write":false,"format":"Def6","id":42,"language":"en","last_edit_date":1611519081,"sequence":null,"title":"Test Case"};
var initial_trial_data = {"frames":[]};
</script>
</html>"#;
        let info = extract_trial_information(html).unwrap();
        assert_eq!(info.id, 42);
        assert_eq!(info.title, "Test Case");
        assert_eq!(info.author, "TestUser");
        assert_eq!(info.language, "en");
        assert_eq!(info.format, "Def6");
        assert!(info.sequence.is_none());
    }

    #[test]
    fn test_extract_trial_data_basic() {
        let html = r#"var initial_trial_data = {"frames":[0,{"id":1}],"profiles":[0]};"#;
        let data = extract_trial_data(html).unwrap();
        assert!(data["frames"].is_array());
        assert_eq!(data["frames"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_extract_trial_data_nested_braces() {
        let html = r#"var initial_trial_data = {"a":{"b":{"c":1}},"d":[{"e":2}]}; var other = 1;"#;
        let data = extract_trial_data(html).unwrap();
        assert_eq!(data["a"]["b"]["c"], 1);
        assert_eq!(data["d"][0]["e"], 2);
    }

    #[test]
    fn test_extract_trial_data_escaped_quotes() {
        let html = r#"var initial_trial_data = {"text":"He said \"hello\""};"#;
        let data = extract_trial_data(html).unwrap();
        assert_eq!(data["text"].as_str().unwrap(), r#"He said "hello""#);
    }

    #[test]
    fn test_rewrite_imported_urls() {
        let mut data = serde_json::json!({
            "profiles": [0, {
                "icon": "assets/icon-abc.png",
                "custom_sprites": [0, {"url": "assets/sprite-def.gif"}]
            }],
            "places": [0, {
                "background": {"image": "assets/bg-xyz.jpg"},
                "name": "Courtroom"
            }],
            "evidence": [0, {"icon": "assets/ev-123.png"}]
        });

        let empty_renames = HashMap::new();
        rewrite_imported_urls(&mut data, 102059, &empty_renames);

        assert_eq!(
            data["profiles"][1]["icon"].as_str().unwrap(),
            "case/102059/assets/icon-abc.png"
        );
        assert_eq!(
            data["profiles"][1]["custom_sprites"][1]["url"].as_str().unwrap(),
            "case/102059/assets/sprite-def.gif"
        );
        assert_eq!(
            data["places"][1]["background"]["image"].as_str().unwrap(),
            "case/102059/assets/bg-xyz.jpg"
        );
        // "Courtroom" should not be rewritten
        assert_eq!(
            data["places"][1]["name"].as_str().unwrap(),
            "Courtroom"
        );
    }

    #[test]
    fn test_rewrite_imported_urls_with_renames() {
        let mut data = serde_json::json!({
            "profiles": [0, {
                "icon": "assets/pioggia+car-123.png",
                "custom_sprites": [0, {"url": "assets/normal-456.gif"}]
            }]
        });

        let mut renames = HashMap::new();
        renames.insert(
            "assets/pioggia+car-123.png".to_string(),
            "assets/pioggia-car-123.png".to_string(),
        );
        rewrite_imported_urls(&mut data, 99999, &renames);

        // Renamed file should use sanitized name
        assert_eq!(
            data["profiles"][1]["icon"].as_str().unwrap(),
            "case/99999/assets/pioggia-car-123.png"
        );
        // Non-renamed file should be unchanged (just prefixed)
        assert_eq!(
            data["profiles"][1]["custom_sprites"][1]["url"].as_str().unwrap(),
            "case/99999/assets/normal-456.gif"
        );
    }

    #[test]
    fn test_import_aaoffline_with_default_sprites() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // HTML with getDefaultSpriteUrl override (like aaoffline downloader produces)
        let html = r#"<html>
<script>
var trial_information = {"author":"Test","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":77777,"language":"en","last_edit_date":1000000,"sequence":null,"title":"Sprite Test"};
var initial_trial_data = {"profiles":[0,{"icon":"assets/icon.png","short_name":"Phoenix","base":"Phoenix","custom_sprites":[]}],"frames":[0],"evidence":[0],"places":[0],"cross_examinations":[0]};
function getDefaultSpriteUrl(base, sprite_id, status)
{
if (base === 'Phoenix' && sprite_id === 1 && status === 'talking') return 'assets/1-aaa.gif';
if (base === 'Phoenix' && sprite_id === 1 && status === 'still') return 'assets/1-bbb.gif';
if (base === 'Phoenix' && sprite_id === 2 && status === 'talking') return 'assets/2-ccc.gif';
return 'data:image/gif;base64,'
}
</script>
</html>"#;
        fs::write(source.path().join("index.html"), html).unwrap();

        let assets_dir = source.path().join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("icon.png"), b"icon").unwrap();
        fs::write(assets_dir.join("1-aaa.gif"), b"talking1").unwrap();
        fs::write(assets_dir.join("1-bbb.gif"), b"still1").unwrap();
        fs::write(assets_dir.join("2-ccc.gif"), b"talking2").unwrap();

        let manifest = import_aaoffline(source.path(), engine.path(), None).unwrap();
        assert_eq!(manifest.case_id, 77777);
        assert_eq!(manifest.assets.shared_defaults, 3, "Should have 3 default sprites");
        assert_eq!(manifest.assets.case_specific, 4); // icon + 3 sprite files in assets/

        // Verify default sprites were copied to the right locations
        assert!(engine.path().join("defaults/images/chars/Phoenix/1.gif").exists(),
            "talking sprite should exist");
        assert!(engine.path().join("defaults/images/charsStill/Phoenix/1.gif").exists(),
            "still sprite should exist");
        assert!(engine.path().join("defaults/images/chars/Phoenix/2.gif").exists(),
            "talking sprite 2 should exist");
    }

    #[test]
    fn test_import_aaoffline_missing_index() {
        let dir = tempfile::tempdir().unwrap();
        let result = import_aaoffline(dir.path(), dir.path(), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No index.html found"));
    }

    #[test]
    fn test_import_aaoffline_full() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Create a minimal index.html
        let html = r#"<html>
<script>
var trial_information = {"author":"Tester","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":99999,"language":"fr","last_edit_date":1000000,"sequence":null,"title":"Import Test"};
var initial_trial_data = {"profiles":[0,{"icon":"assets/icon.png","short_name":"Hero","custom_sprites":[]}],"frames":[0],"evidence":[0],"places":[0],"cross_examinations":[0]};
</script>
</html>"#;
        fs::write(source.path().join("index.html"), html).unwrap();

        // Create assets
        let assets_dir = source.path().join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("icon.png"), "fake png data").unwrap();
        fs::write(assets_dir.join("bg.jpg"), "fake jpg data").unwrap();

        let manifest = import_aaoffline(source.path(), engine.path(), None).unwrap();

        assert_eq!(manifest.case_id, 99999);
        assert_eq!(manifest.title, "Import Test");
        assert_eq!(manifest.author, "Tester");
        assert_eq!(manifest.language, "fr");
        assert_eq!(manifest.assets.total_downloaded, 2);
        assert!(manifest.failed_assets.is_empty());

        // Verify files were created
        let case_dir = engine.path().join("case/99999");
        assert!(case_dir.join("manifest.json").exists());
        assert!(case_dir.join("trial_info.json").exists());
        assert!(case_dir.join("trial_data.json").exists());
        assert!(case_dir.join("assets/icon.png").exists());
        assert!(case_dir.join("assets/bg.jpg").exists());

        // Verify URL rewriting in trial_data
        let data_str = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
        let data: Value = serde_json::from_str(&data_str).unwrap();
        assert_eq!(
            data["profiles"][1]["icon"].as_str().unwrap(),
            "case/99999/assets/icon.png"
        );
    }

    /// Integration test: parse the REAL aaoffline download if present.
    #[test]
    fn test_parse_real_aaoffline_download() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let source_dir = manifest_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Random/Ace Prosecutor Zero 1  A Trial in the Rain_102059");

        if !source_dir.exists() {
            eprintln!("Skipping: real aaoffline download not found at {}", source_dir.display());
            return;
        }

        let html = fs::read_to_string(source_dir.join("index.html")).unwrap();

        // Parse trial_information
        let info = extract_trial_information(&html).unwrap();
        assert_eq!(info.id, 102059);
        assert_eq!(info.title, "Ace Prosecutor Zero 1 | A Trial in the Rain");
        assert_eq!(info.author, "Exedeb");
        assert_eq!(info.language, "en");
        assert_eq!(info.format, "Def6");

        // Parse trial_data (large JSON)
        let data = extract_trial_data(&html).unwrap();
        assert!(data["frames"].is_array());
        assert!(data["profiles"].is_array());
        let frames = data["frames"].as_array().unwrap();
        assert!(frames.len() > 100, "Expected many frames, got {}", frames.len());
        let profiles = data["profiles"].as_array().unwrap();
        assert!(profiles.len() > 5, "Expected several profiles, got {}", profiles.len());

        // Verify asset references exist in the parsed data
        // Profile icons should reference assets/
        let first_profile = &profiles[1]; // skip the 0 sentinel
        let icon = first_profile["icon"].as_str().unwrap_or("");
        assert!(icon.starts_with("assets/"), "Profile icon should start with 'assets/', got: {}", icon);

        // Full import test into a temp dir
        let engine = tempfile::tempdir().unwrap();
        let manifest = import_aaoffline(&source_dir, engine.path(), None).unwrap();
        assert_eq!(manifest.case_id, 102059);
        assert!(manifest.assets.total_downloaded > 300, "Expected 300+ assets, got {}", manifest.assets.total_downloaded);
        assert!(manifest.assets.total_size_bytes > 10_000_000, "Expected 10MB+ of assets");

        // Verify trial_data was rewritten correctly
        let case_dir = engine.path().join("case/102059");
        let data_str = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
        assert!(data_str.contains("case/102059/assets/"), "URLs should be rewritten to case/102059/assets/");
        assert!(!data_str.contains("\"assets/"), "No raw 'assets/' refs should remain");
    }

    #[test]
    fn test_import_aaoffline_duplicate_rejected() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        let html = r#"<html>
<script>
var trial_information = {"author":"A","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":12345,"language":"en","last_edit_date":0,"sequence":null,"title":"Dup Test"};
var initial_trial_data = {"frames":[0]};
</script>
</html>"#;
        fs::write(source.path().join("index.html"), html).unwrap();

        // First import succeeds
        import_aaoffline(source.path(), engine.path(), None).unwrap();

        // Second import should fail (duplicate)
        let result = import_aaoffline(source.path(), engine.path(), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    // --- ZIP import tests ---

    /// Helper: create a .aaocase ZIP in a temp dir, returns the path.
    fn create_test_aaocase(dir: &Path, case_id: u32) -> PathBuf {
        use std::io::Write;

        let zip_path = dir.join(format!("test_{}.aaocase", case_id));
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        // manifest.json
        let manifest = serde_json::json!({
            "case_id": case_id,
            "title": "ZIP Test Case",
            "author": "ZipTester",
            "language": "en",
            "download_date": "2025-01-01T00:00:00Z",
            "format": "Def6",
            "sequence": null,
            "assets": {
                "case_specific": 2,
                "shared_defaults": 0,
                "total_downloaded": 2,
                "total_size_bytes": 100
            },
            "asset_map": {
                "http://example.com/bg.png": "assets/bg.png",
                "http://example.com/music.mp3": "assets/music.mp3"
            },
            "failed_assets": []
        });
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes()).unwrap();

        // trial_info.json
        let info = serde_json::json!({
            "id": case_id,
            "title": "ZIP Test Case",
            "author": "ZipTester",
            "language": "en",
            "format": "Def6",
            "last_edit_date": 0,
            "sequence": null
        });
        zip.start_file("trial_info.json", options).unwrap();
        zip.write_all(serde_json::to_string_pretty(&info).unwrap().as_bytes()).unwrap();

        // trial_data.json
        let data = serde_json::json!({
            "frames": [0, {"id": 1}],
            "profiles": [0],
            "evidence": [0],
            "places": [0]
        });
        zip.start_file("trial_data.json", options).unwrap();
        zip.write_all(serde_json::to_string_pretty(&data).unwrap().as_bytes()).unwrap();

        // assets/
        zip.start_file("assets/bg.png", options).unwrap();
        zip.write_all(b"fake png data").unwrap();

        zip.start_file("assets/music.mp3", options).unwrap();
        zip.write_all(b"fake mp3 data").unwrap();

        zip.finish().unwrap();
        zip_path
    }

    #[test]
    fn test_import_aaocase_zip_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        let zip_path = create_test_aaocase(tmp.path(), 77777);
        let manifest = import_aaocase_zip(&zip_path, engine.path(), None).unwrap().manifest;

        assert_eq!(manifest.case_id, 77777);
        assert_eq!(manifest.title, "ZIP Test Case");
        assert_eq!(manifest.author, "ZipTester");
        assert_eq!(manifest.language, "en");
        assert_eq!(manifest.assets.total_downloaded, 2);

        // Verify files extracted
        let case_dir = engine.path().join("case/77777");
        assert!(case_dir.join("manifest.json").exists());
        assert!(case_dir.join("trial_info.json").exists());
        assert!(case_dir.join("trial_data.json").exists());
        assert!(case_dir.join("assets/bg.png").exists());
        assert!(case_dir.join("assets/music.mp3").exists());

        // Verify asset content
        let bg = fs::read_to_string(case_dir.join("assets/bg.png")).unwrap();
        assert_eq!(bg, "fake png data");
    }

    #[test]
    fn test_import_aaocase_zip_duplicate_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        let zip_path = create_test_aaocase(tmp.path(), 88888);

        // First import succeeds
        import_aaocase_zip(&zip_path, engine.path(), None).unwrap();

        // Second import should fail
        let result = import_aaocase_zip(&zip_path, engine.path(), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    #[test]
    fn test_import_aaocase_zip_invalid_file() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Write a non-ZIP file
        let bad_path = tmp.path().join("bad.aaocase");
        fs::write(&bad_path, "this is not a zip file").unwrap();

        let result = import_aaocase_zip(&bad_path, engine.path(), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid ZIP file"));
    }

    /// Regression: filenames with URL-unsafe characters (like +, #, &) must be
    /// sanitized during import so they can be served over HTTP without issues.
    #[test]
    fn test_import_aaoffline_sanitizes_filenames() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        let html = r#"<html>
<script>
var trial_information = {"author":"A","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":55555,"language":"en","last_edit_date":0,"sequence":null,"title":"Sanitize Test"};
var initial_trial_data = {"frames":[0],"profiles":[0,{"icon":"assets/pioggia+car-123.png","short_name":"Test","custom_sprites":[]}],"evidence":[0],"places":[0],"cross_examinations":[0]};
</script>
</html>"#;
        fs::write(source.path().join("index.html"), html).unwrap();

        let assets_dir = source.path().join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        // Files with problematic characters
        fs::write(assets_dir.join("pioggia+car-123.png"), "img data").unwrap();
        fs::write(assets_dir.join("file#with&special-456.mp3"), "audio data").unwrap();
        fs::write(assets_dir.join("normal-file-789.gif"), "gif data").unwrap();

        let manifest = import_aaoffline(source.path(), engine.path(), None).unwrap();
        let case_dir = engine.path().join("case/55555");

        // Sanitized files should exist (+ → -, # → -, & → -)
        assert!(case_dir.join("assets/pioggia-car-123.png").exists(),
            "File with + should be renamed with - on disk");
        assert!(case_dir.join("assets/file-with-special-456.mp3").exists(),
            "File with # and & should be renamed with - on disk");
        // Normal files should be unchanged
        assert!(case_dir.join("assets/normal-file-789.gif").exists(),
            "Normal file should keep its name");

        // Original unsanitized names should NOT exist
        assert!(!case_dir.join("assets/pioggia+car-123.png").exists(),
            "Original file with + should not exist");

        // trial_data.json should reference sanitized filenames
        let data_str = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
        assert!(data_str.contains("case/55555/assets/pioggia-car-123.png"),
            "trial_data.json should reference the sanitized filename");
        assert!(!data_str.contains("pioggia+car"),
            "trial_data.json should not contain the unsanitized filename");

        // Manifest asset_map should map old → new names
        assert_eq!(manifest.assets.total_downloaded, 3);
    }

    /// Regression: sanitize_imported_filename must handle all URL-unsafe characters.
    #[test]
    fn test_sanitize_imported_filename() {
        assert_eq!(sanitize_imported_filename("pioggia+car-123.mp3"), "pioggia-car-123.mp3");
        assert_eq!(sanitize_imported_filename("file#fragment-456.png"), "file-fragment-456.png");
        assert_eq!(sanitize_imported_filename("a&b=c-789.gif"), "a-b-c-789.gif");
        assert_eq!(sanitize_imported_filename("100%done-111.jpg"), "100-done-111.jpg");
        assert_eq!(sanitize_imported_filename("normal-file_ok-222.mp3"), "normal-file_ok-222.mp3");
        // Already-safe filenames should be unchanged
        assert_eq!(sanitize_imported_filename("safe-name-333.png"), "safe-name-333.png");
    }

    #[test]
    fn test_import_aaocase_zip_missing_manifest() {
        use std::io::Write;

        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Create ZIP without manifest.json
        let zip_path = tmp.path().join("no_manifest.aaocase");
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("trial_data.json", options).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish().unwrap();

        let result = import_aaocase_zip(&zip_path, engine.path(), None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("manifest.json"));
    }

    // --- Export tests ---

    #[test]
    fn test_export_aaocase_basic() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // First, import a case so we have something to export
        let html = r#"<html>
<script>
var trial_information = {"author":"Tester","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":44444,"language":"en","last_edit_date":1000000,"sequence":null,"title":"Export Test"};
var initial_trial_data = {"profiles":[0,{"icon":"assets/icon.png","short_name":"Hero","custom_sprites":[]}],"frames":[0],"evidence":[0],"places":[0],"cross_examinations":[0]};
</script>
</html>"#;
        fs::write(source.path().join("index.html"), html).unwrap();
        let assets_dir = source.path().join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("icon.png"), "fake png data").unwrap();
        fs::write(assets_dir.join("music.mp3"), "fake mp3 data").unwrap();

        import_aaoffline(source.path(), engine.path(), None).unwrap();

        // Now export it
        let export_path = source.path().join("test.aaocase");
        let size = export_aaocase(44444, engine.path(), &export_path, None, None, true).unwrap();
        assert!(size > 0, "ZIP file should have non-zero size");
        assert!(export_path.exists(), "ZIP file should exist on disk");

        // Verify we can reimport the exported file into a fresh engine dir
        let engine2 = tempfile::tempdir().unwrap();
        let manifest = import_aaocase_zip(&export_path, engine2.path(), None).unwrap().manifest;
        assert_eq!(manifest.case_id, 44444);
        assert_eq!(manifest.title, "Export Test");

        let case_dir = engine2.path().join("case/44444");
        assert!(case_dir.join("manifest.json").exists());
        assert!(case_dir.join("trial_data.json").exists());
        assert!(case_dir.join("assets/icon.png").exists());
        assert!(case_dir.join("assets/music.mp3").exists());
    }

    #[test]
    fn test_export_aaocase_missing_case() {
        let engine = tempfile::tempdir().unwrap();
        let export_path = engine.path().join("missing.aaocase");
        let result = export_aaocase(99999, engine.path(), &export_path, None, None, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    /// Regression: single-case ZIP import still works after multi-case support was added.
    #[test]
    fn test_import_single_case_still_works_after_multi_support() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Create a classic single-case ZIP (no sequence.json)
        let zip_path = create_test_aaocase(tmp.path(), 11111);
        let manifest = import_aaocase_zip(&zip_path, engine.path(), None).unwrap().manifest;

        assert_eq!(manifest.case_id, 11111);
        assert_eq!(manifest.title, "ZIP Test Case");
        let case_dir = engine.path().join("case/11111");
        assert!(case_dir.join("manifest.json").exists());
        assert!(case_dir.join("trial_data.json").exists());
        assert!(case_dir.join("assets/bg.png").exists());
    }

    /// Test multi-case sequence export creates valid ZIP structure.
    #[test]
    fn test_export_sequence_creates_valid_zip() {
        let engine = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Set up two cases on disk
        for &case_id in &[69063u32, 69064] {
            let case_dir = engine.path().join("case").join(case_id.to_string());
            fs::create_dir_all(case_dir.join("assets")).unwrap();

            let manifest = CaseManifest {
                case_id,
                title: format!("Part {}", case_id),
                author: "Author".to_string(),
                language: "en".to_string(),
                download_date: "2025-01-01T00:00:00Z".to_string(),
                format: "v6".to_string(),
                sequence: Some(serde_json::json!({
                    "title": "Test Sequence",
                    "list": [{"id": 69063, "title": "Part 1"}, {"id": 69064, "title": "Part 2"}]
                })),
                assets: AssetSummary {
                    case_specific: 1, shared_defaults: 0,
                    total_downloaded: 1, total_size_bytes: 10,
                },
                asset_map: HashMap::new(),
                failed_assets: vec![],
                has_plugins: false,
                has_case_config: false,
            };
            write_manifest(&manifest, &case_dir).unwrap();
            fs::write(case_dir.join("trial_info.json"), r#"{"id":0}"#).unwrap();
            fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
            fs::write(case_dir.join("assets").join("test.png"), "fake").unwrap();
        }

        let seq_list = serde_json::json!([
            {"id": 69063, "title": "Part 1"},
            {"id": 69064, "title": "Part 2"}
        ]);
        let export_path = tmp.path().join("sequence.aaocase");
        let size = export_sequence(
            &[69063, 69064],
            "Test Sequence",
            &seq_list,
            engine.path(),
            &export_path,
            None,
            None,
            true,
        ).unwrap();

        assert!(size > 0);
        assert!(export_path.exists());

        // Verify ZIP structure
        let file = fs::File::open(&export_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut entry_names: Vec<String> = Vec::new();
        for i in 0..archive.len() {
            entry_names.push(archive.by_index(i).unwrap().name().to_string());
        }
        assert!(entry_names.contains(&"sequence.json".to_string()));
        assert!(entry_names.contains(&"69063/manifest.json".to_string()));
        assert!(entry_names.contains(&"69064/manifest.json".to_string()));
        assert!(entry_names.contains(&"69063/trial_data.json".to_string()));
        assert!(entry_names.contains(&"69064/trial_data.json".to_string()));
    }

    /// Test multi-case ZIP import.
    #[test]
    fn test_import_multi_case_zip() {
        let engine_export = tempfile::tempdir().unwrap();
        let engine_import = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Set up two cases
        for &case_id in &[69063u32, 69064] {
            let case_dir = engine_export.path().join("case").join(case_id.to_string());
            fs::create_dir_all(case_dir.join("assets")).unwrap();

            let manifest = CaseManifest {
                case_id,
                title: format!("Part {}", if case_id == 69063 { "Investigation" } else { "Trial" }),
                author: "Author".to_string(),
                language: "en".to_string(),
                download_date: "2025-01-01T00:00:00Z".to_string(),
                format: "v6".to_string(),
                sequence: Some(serde_json::json!({
                    "title": "A Turnabout Called Justice",
                    "list": [{"id": 69063, "title": "Investigation"}, {"id": 69064, "title": "Trial"}]
                })),
                assets: AssetSummary {
                    case_specific: 1, shared_defaults: 0,
                    total_downloaded: 1, total_size_bytes: 12,
                },
                asset_map: HashMap::new(),
                failed_assets: vec![],
                has_plugins: false,
                has_case_config: false,
            };
            write_manifest(&manifest, &case_dir).unwrap();
            fs::write(case_dir.join("trial_info.json"), format!(r#"{{"id":{}}}"#, case_id)).unwrap();
            fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
            fs::write(case_dir.join("assets").join("img.png"), "fake image").unwrap();
        }

        // Export sequence
        let seq_list = serde_json::json!([
            {"id": 69063, "title": "Investigation"},
            {"id": 69064, "title": "Trial"}
        ]);
        let export_path = tmp.path().join("sequence.aaocase");
        export_sequence(
            &[69063, 69064],
            "A Turnabout Called Justice",
            &seq_list,
            engine_export.path(),
            &export_path,
            None,
            None,
            true,
        ).unwrap();

        // Import into fresh engine dir
        let manifest = import_aaocase_zip(&export_path, engine_import.path(), None).unwrap().manifest;
        assert_eq!(manifest.case_id, 69063); // First case's manifest

        // Both cases should be imported
        assert!(engine_import.path().join("case/69063/manifest.json").exists());
        assert!(engine_import.path().join("case/69064/manifest.json").exists());
        assert!(engine_import.path().join("case/69063/assets/img.png").exists());
        assert!(engine_import.path().join("case/69064/assets/img.png").exists());
    }

    /// Test multi-case import skips existing cases.
    #[test]
    fn test_import_multi_case_skips_existing() {
        let engine_export = tempfile::tempdir().unwrap();
        let engine_import = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Set up two cases for export
        for &case_id in &[69063u32, 69064] {
            let case_dir = engine_export.path().join("case").join(case_id.to_string());
            fs::create_dir_all(case_dir.join("assets")).unwrap();
            let manifest = CaseManifest {
                case_id,
                title: format!("Part {}", case_id),
                author: "Author".to_string(),
                language: "en".to_string(),
                download_date: "2025-01-01T00:00:00Z".to_string(),
                format: "v6".to_string(),
                sequence: None,
                assets: AssetSummary {
                    case_specific: 0, shared_defaults: 0,
                    total_downloaded: 0, total_size_bytes: 0,
                },
                asset_map: HashMap::new(),
                failed_assets: vec![],
                has_plugins: false,
                has_case_config: false,
            };
            write_manifest(&manifest, &case_dir).unwrap();
            fs::write(case_dir.join("trial_info.json"), "{}").unwrap();
            fs::write(case_dir.join("trial_data.json"), "{}").unwrap();
        }

        // Export sequence
        let seq_list = serde_json::json!([{"id": 69063, "title": "P1"}, {"id": 69064, "title": "P2"}]);
        let export_path = tmp.path().join("seq.aaocase");
        export_sequence(&[69063, 69064], "Seq", &seq_list, engine_export.path(), &export_path, None, None, true).unwrap();

        // Pre-install case 69063 in import engine
        let pre_case_dir = engine_import.path().join("case/69063");
        fs::create_dir_all(&pre_case_dir).unwrap();
        let pre_manifest = CaseManifest {
            case_id: 69063,
            title: "Already Here".to_string(),
            author: "Pre".to_string(),
            language: "en".to_string(),
            download_date: "2024-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&pre_manifest, &pre_case_dir).unwrap();

        // Import — should skip 69063 and import 69064
        let manifest = import_aaocase_zip(&export_path, engine_import.path(), None).unwrap().manifest;
        // First manifest should be the pre-existing one
        assert_eq!(manifest.case_id, 69063);
        assert_eq!(manifest.title, "Already Here"); // wasn't overwritten

        // 69064 should be imported
        assert!(engine_import.path().join("case/69064/manifest.json").exists());
    }

    #[test]
    fn test_export_roundtrip_preserves_data() {
        // Import from ZIP, export, reimport — data should be identical
        let tmp = tempfile::tempdir().unwrap();
        let engine1 = tempfile::tempdir().unwrap();

        let zip_path = create_test_aaocase(tmp.path(), 66666);
        let manifest1 = import_aaocase_zip(&zip_path, engine1.path(), None).unwrap().manifest;

        // Export
        let export_path = tmp.path().join("roundtrip.aaocase");
        export_aaocase(66666, engine1.path(), &export_path, None, None, true).unwrap();

        // Reimport into fresh dir
        let engine2 = tempfile::tempdir().unwrap();
        let manifest2 = import_aaocase_zip(&export_path, engine2.path(), None).unwrap().manifest;

        assert_eq!(manifest1.case_id, manifest2.case_id);
        assert_eq!(manifest1.title, manifest2.title);
        assert_eq!(manifest1.author, manifest2.author);
        assert_eq!(manifest1.language, manifest2.language);

        // Verify asset contents match
        let case1 = engine1.path().join("case/66666");
        let case2 = engine2.path().join("case/66666");
        let data1 = fs::read_to_string(case1.join("trial_data.json")).unwrap();
        let data2 = fs::read_to_string(case2.join("trial_data.json")).unwrap();
        assert_eq!(data1, data2, "trial_data.json should be identical after roundtrip");
    }

    // --- New tests ---

    /// Exporting a sequence where one case doesn't exist should return an error.
    #[test]
    fn test_export_sequence_missing_case_returns_error() {
        let engine = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Set up only one case
        let case_dir = engine.path().join("case/70001");
        fs::create_dir_all(case_dir.join("assets")).unwrap();
        let manifest = CaseManifest {
            case_id: 70001,
            title: "Existing Part".to_string(),
            author: "Author".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), "{}").unwrap();
        fs::write(case_dir.join("trial_data.json"), "{}").unwrap();

        let seq_list = serde_json::json!([
            {"id": 70001, "title": "Part 1"},
            {"id": 70002, "title": "Part 2"}
        ]);
        let export_path = tmp.path().join("missing_case.aaocase");
        let result = export_sequence(
            &[70001, 70002],
            "Broken Sequence",
            &seq_list,
            engine.path(),
            &export_path,
            None,
            None,
            true,
        );
        assert!(result.is_err(), "Should fail when a case in the sequence doesn't exist");
        assert!(
            result.unwrap_err().contains("not found"),
            "Error should mention case not found"
        );
    }

    /// Export with empty case_ids list should create a valid ZIP containing only sequence.json.
    #[test]
    fn test_export_sequence_empty_list_creates_valid_zip() {
        let engine = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        let seq_list = serde_json::json!([]);
        let export_path = tmp.path().join("empty_seq.aaocase");
        let size = export_sequence(
            &[],
            "Empty Sequence",
            &seq_list,
            engine.path(),
            &export_path,
            None,
            None,
            true,
        ).unwrap();

        assert!(size > 0, "ZIP file should have non-zero size");
        assert!(export_path.exists());

        // Verify ZIP structure — should only have sequence.json
        let file = fs::File::open(&export_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 1, "Should contain only sequence.json");
        assert_eq!(archive.by_index(0).unwrap().name(), "sequence.json");
    }

    /// Importing a multi-case ZIP where sequence.json has an empty list should return an error.
    #[test]
    fn test_import_multi_case_empty_sequence_list() {
        use std::io::Write;

        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Create a ZIP with sequence.json containing empty list
        let zip_path = tmp.path().join("empty_list.aaocase");
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        let seq = serde_json::json!({
            "title": "Empty Sequence",
            "list": []
        });
        zip.start_file("sequence.json", options).unwrap();
        zip.write_all(serde_json::to_string(&seq).unwrap().as_bytes()).unwrap();
        zip.finish().unwrap();

        let result = import_aaocase_zip(&zip_path, engine.path(), None);
        assert!(result.is_err(), "Should fail with empty sequence list");
        assert!(
            result.unwrap_err().contains("empty list"),
            "Error should mention empty list"
        );
    }

    /// Single-case ZIP import preserves failed_assets field in manifest roundtrip.
    #[test]
    fn test_import_single_case_backward_compat_with_failed_assets() {
        use std::io::Write;

        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        let zip_path = tmp.path().join("with_failures.aaocase");
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        // manifest.json with failed_assets
        let manifest = serde_json::json!({
            "case_id": 88001,
            "title": "Failed Assets Test",
            "author": "Tester",
            "language": "en",
            "download_date": "2025-06-01T00:00:00Z",
            "format": "Def6",
            "sequence": null,
            "assets": {
                "case_specific": 1,
                "shared_defaults": 0,
                "total_downloaded": 1,
                "total_size_bytes": 50
            },
            "asset_map": {
                "http://ok.com/bg.png": "assets/bg.png"
            },
            "failed_assets": [
                {
                    "url": "http://dead.com/music.mp3",
                    "asset_type": "music",
                    "local_path": "assets/music-hash.mp3",
                    "error": "HTTP 404"
                }
            ]
        });
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes()).unwrap();

        // trial_info.json
        let info = serde_json::json!({"id": 88001, "title": "Failed Assets Test", "author": "Tester", "language": "en", "format": "Def6", "last_edit_date": 0, "sequence": null});
        zip.start_file("trial_info.json", options).unwrap();
        zip.write_all(serde_json::to_string(&info).unwrap().as_bytes()).unwrap();

        // trial_data.json
        zip.start_file("trial_data.json", options).unwrap();
        zip.write_all(b"{}").unwrap();

        // asset
        zip.start_file("assets/bg.png", options).unwrap();
        zip.write_all(b"fake png").unwrap();

        zip.finish().unwrap();

        let imported = import_aaocase_zip(&zip_path, engine.path(), None).unwrap().manifest;
        assert_eq!(imported.case_id, 88001);
        assert_eq!(imported.failed_assets.len(), 1, "failed_assets should roundtrip");
        assert_eq!(imported.failed_assets[0].url, "http://dead.com/music.mp3");
        assert_eq!(imported.failed_assets[0].error, "HTTP 404");
    }

    /// sanitize_imported_filename should preserve the extension (lowercased).
    #[test]
    fn test_sanitize_imported_filename_preserves_extension_case() {
        // Extension should be lowercased
        assert_eq!(sanitize_imported_filename("image.PNG"), "image.png");
        assert_eq!(sanitize_imported_filename("music.MP3"), "music.mp3");
        assert_eq!(sanitize_imported_filename("sprite.GIF"), "sprite.gif");
        assert_eq!(sanitize_imported_filename("normal.jpg"), "normal.jpg");
        // Mixed case extension
        assert_eq!(sanitize_imported_filename("file.JpG"), "file.jpg");
    }

    /// sanitize_imported_filename with no extension should return sanitized name only.
    #[test]
    fn test_sanitize_imported_filename_no_extension() {
        assert_eq!(sanitize_imported_filename("filename"), "filename");
        assert_eq!(sanitize_imported_filename("file+name"), "file-name");
        assert_eq!(sanitize_imported_filename("a&b#c"), "a-b-c");
        // Name with only invalid characters
        assert_eq!(sanitize_imported_filename("+++"), "---");
    }

    // --- Test collection data validation (Phase E) ---
    // 3 collections × 2 parts each = 6 test cases total.
    // Collection A (99901-99902): Part 1 ends with GameOver(action=1) explicit redirect
    // Collection B (99903-99904): Part 1 ends with GameOver(action=0) → auto-continue
    // Collection C (99905-99906): Part 1 runs out of frames → auto-continue

    /// Helper: path to test data directory.
    fn test_data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("test-data")
    }

    /// All 6 test manifests should be valid with correct sequence metadata.
    #[test]
    fn test_collection_manifests_valid() {
        let base = test_data_dir();
        let collections = [
            ("collection-a", &[99901u32, 99902][..], "Test A - Explicit Redirect"),
            ("collection-b", &[99903u32, 99904][..], "Test B - Auto-Continue (action=0)"),
            ("collection-c", &[99905u32, 99906][..], "Test C - Auto-Continue (no GameOver)"),
        ];

        for (folder, ids, expected_title) in &collections {
            for &id in *ids {
                let case_dir = base.join(folder).join(id.to_string());
                let manifest = read_manifest(&case_dir)
                    .unwrap_or_else(|e| panic!("Failed to read manifest for {}: {}", id, e));
                assert_eq!(manifest.case_id, id);
                assert_eq!(manifest.author, "TestBot");
                assert_eq!(manifest.language, "en");
                assert_eq!(manifest.format, "Def6");
                assert!(manifest.sequence.is_some(), "Case {} should have sequence", id);

                let seq = manifest.sequence.as_ref().unwrap();
                assert_eq!(seq["title"].as_str().unwrap(), *expected_title,
                    "Case {} sequence title mismatch", id);
                let list = seq["list"].as_array().unwrap();
                assert_eq!(list.len(), 2, "Case {} should list 2 parts", id);
            }
        }
    }

    /// Collection A Part 1: last frame has GameOver with action=val=1 (explicit next).
    #[test]
    fn test_collection_a_part1_has_gameover_next() {
        let data_str = fs::read_to_string(
            test_data_dir().join("collection-a/99901/trial_data.json")
        ).unwrap();
        let data: Value = serde_json::from_str(&data_str).unwrap();
        let frames = data["frames"].as_array().unwrap();
        let last = &frames[frames.len() - 1];

        assert_eq!(last["action_name"], "GameOver");
        assert_eq!(last["action_parameters"]["global"]["action"], "val=1");
    }

    /// Collection A Part 2: no GameOver (destination part, just runs out).
    #[test]
    fn test_collection_a_part2_no_gameover() {
        let data_str = fs::read_to_string(
            test_data_dir().join("collection-a/99902/trial_data.json")
        ).unwrap();
        let data: Value = serde_json::from_str(&data_str).unwrap();
        let frames = data["frames"].as_array().unwrap();
        for (i, frame) in frames.iter().enumerate() {
            if !frame.is_object() { continue; }
            assert_ne!(frame["action_name"].as_str().unwrap_or(""), "GameOver",
                "Collection A Part 2 frame {} should NOT have GameOver", i);
        }
    }

    /// Collection B Part 1: last frame has GameOver with action=val=0 (end and do nothing).
    #[test]
    fn test_collection_b_part1_has_gameover_end() {
        let data_str = fs::read_to_string(
            test_data_dir().join("collection-b/99903/trial_data.json")
        ).unwrap();
        let data: Value = serde_json::from_str(&data_str).unwrap();
        let frames = data["frames"].as_array().unwrap();
        let last = &frames[frames.len() - 1];

        assert_eq!(last["action_name"], "GameOver");
        assert_eq!(last["action_parameters"]["global"]["action"], "val=0");
    }

    /// Collection C Part 1: no GameOver at all — just runs out of frames.
    #[test]
    fn test_collection_c_part1_no_gameover() {
        let data_str = fs::read_to_string(
            test_data_dir().join("collection-c/99905/trial_data.json")
        ).unwrap();
        let data: Value = serde_json::from_str(&data_str).unwrap();
        let frames = data["frames"].as_array().unwrap();
        for (i, frame) in frames.iter().enumerate() {
            if !frame.is_object() { continue; }
            assert_ne!(frame["action_name"].as_str().unwrap_or(""), "GameOver",
                "Collection C Part 1 frame {} should NOT have GameOver", i);
        }
    }

    /// Export/import roundtrip for each test collection.
    #[test]
    fn test_export_import_collections_roundtrip() {
        let base = test_data_dir();
        let collections: Vec<(&str, Vec<u32>, &str)> = vec![
            ("collection-a", vec![99901, 99902], "Test Collection A - Explicit Redirect"),
            ("collection-b", vec![99903, 99904], "Test Collection B - Auto-Continue (action=0)"),
            ("collection-c", vec![99905, 99906], "Test Collection C - Auto-Continue (no GameOver)"),
        ];

        for (folder, ids, title) in &collections {
            let engine_export = tempfile::tempdir().unwrap();
            let engine_import = tempfile::tempdir().unwrap();
            let tmp = tempfile::tempdir().unwrap();

            // Copy test data into engine_export
            for &id in ids {
                let src = base.join(folder).join(id.to_string());
                let dst = engine_export.path().join("case").join(id.to_string());
                fs::create_dir_all(&dst).unwrap();
                for name in &["manifest.json", "trial_info.json", "trial_data.json"] {
                    let src_file = src.join(name);
                    if src_file.exists() {
                        fs::copy(&src_file, dst.join(name)).unwrap();
                    }
                }
            }

            // Build sequence list
            let seq_list: Vec<Value> = ids.iter().map(|&id| {
                let m = read_manifest(
                    &engine_export.path().join("case").join(id.to_string())
                ).unwrap();
                serde_json::json!({"id": id, "title": m.title})
            }).collect();

            let export_path = tmp.path().join("test.aaocase");
            let size = export_sequence(
                &ids, title, &Value::Array(seq_list),
                engine_export.path(), &export_path, None, None, true,
            ).unwrap();
            assert!(size > 0, "{} export should have non-zero size", folder);

            // Import
            let manifest = import_aaocase_zip(&export_path, engine_import.path(), None).unwrap().manifest;
            assert_eq!(manifest.case_id, ids[0], "{} first case should match", folder);

            // Verify all parts present
            for &id in ids {
                let case_dir = engine_import.path().join("case").join(id.to_string());
                assert!(case_dir.join("manifest.json").exists(),
                    "{} case {} manifest should exist after import", folder, id);
                assert!(case_dir.join("trial_data.json").exists(),
                    "{} case {} trial_data should exist after import", folder, id);
            }
        }
    }

    // --- Regression tests for saves in export/import (Phase A) ---
    // These verify current behavior BEFORE adding saves support.

    /// Regression: export_aaocase should NOT produce a saves.json entry in the ZIP.
    #[test]
    fn test_export_aaocase_no_saves_json_in_zip() {
        let engine = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Set up a case on disk
        let case_dir = engine.path().join("case/77001");
        fs::create_dir_all(case_dir.join("assets")).unwrap();
        let manifest = CaseManifest {
            case_id: 77001,
            title: "No Saves Test".to_string(),
            author: "Tester".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: AssetSummary { case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 10 },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), r#"{"id":77001}"#).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
        fs::write(case_dir.join("assets/test.png"), "fake").unwrap();

        let export_path = tmp.path().join("no_saves.aaocase");
        export_aaocase(77001, engine.path(), &export_path, None, None, true).unwrap();

        // Verify ZIP does NOT contain saves.json
        let file = fs::File::open(&export_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let entry_names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(!entry_names.contains(&"saves.json".to_string()),
            "Current export should not contain saves.json");
        // Should contain the standard files
        assert!(entry_names.contains(&"manifest.json".to_string()));
        assert!(entry_names.contains(&"trial_data.json".to_string()));
        assert!(entry_names.contains(&"trial_info.json".to_string()));
        assert!(entry_names.contains(&"assets/test.png".to_string()));
    }

    /// Regression: export_sequence should NOT produce saves.json in the ZIP.
    #[test]
    fn test_export_sequence_no_saves_json_in_zip() {
        let engine = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Set up two cases
        for &case_id in &[77002u32, 77003] {
            let case_dir = engine.path().join("case").join(case_id.to_string());
            fs::create_dir_all(case_dir.join("assets")).unwrap();
            let manifest = CaseManifest {
                case_id,
                title: format!("Part {}", case_id),
                author: "Tester".to_string(),
                language: "en".to_string(),
                download_date: "2025-01-01T00:00:00Z".to_string(),
                format: "v6".to_string(),
                sequence: None,
                assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
                asset_map: HashMap::new(),
                failed_assets: vec![],
                has_plugins: false,
                has_case_config: false,
            };
            write_manifest(&manifest, &case_dir).unwrap();
            fs::write(case_dir.join("trial_info.json"), "{}").unwrap();
            fs::write(case_dir.join("trial_data.json"), "{}").unwrap();
        }

        let seq_list = serde_json::json!([{"id": 77002, "title": "P1"}, {"id": 77003, "title": "P2"}]);
        let export_path = tmp.path().join("no_saves_seq.aaocase");
        export_sequence(&[77002, 77003], "No Saves Seq", &seq_list, engine.path(), &export_path, None, None, true).unwrap();

        // Verify ZIP does NOT contain saves.json
        let file = fs::File::open(&export_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let entry_names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(!entry_names.contains(&"saves.json".to_string()),
            "Current sequence export should not contain saves.json");
        assert!(entry_names.contains(&"sequence.json".to_string()));
    }

    /// Regression: import_aaocase_zip without saves.json returns a valid manifest.
    #[test]
    fn test_import_aaocase_without_saves_returns_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        let zip_path = create_test_aaocase(tmp.path(), 77004);
        let manifest = import_aaocase_zip(&zip_path, engine.path(), None).unwrap().manifest;
        assert_eq!(manifest.case_id, 77004);
        assert_eq!(manifest.title, "ZIP Test Case");
        // Verify case files were properly installed
        let case_dir = engine.path().join("case/77004");
        assert!(case_dir.join("manifest.json").exists());
        assert!(case_dir.join("trial_data.json").exists());
    }

    /// Regression: single-case export + import roundtrip preserves all metadata.
    #[test]
    fn test_export_import_roundtrip_metadata_preserved() {
        let engine1 = tempfile::tempdir().unwrap();
        let engine2 = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Set up case with full metadata
        let case_dir = engine1.path().join("case/77005");
        fs::create_dir_all(case_dir.join("assets")).unwrap();
        let original = CaseManifest {
            case_id: 77005,
            title: "Metadata Roundtrip".to_string(),
            author: "AuthorZ".to_string(),
            language: "fr".to_string(),
            download_date: "2025-03-14T12:00:00Z".to_string(),
            format: "Def6".to_string(),
            sequence: Some(serde_json::json!({"title": "Test Seq", "list": [{"id": 77005, "title": "Only"}]})),
            assets: AssetSummary { case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 8 },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&original, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), r#"{"id":77005}"#).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0,{"id":1}]}"#).unwrap();
        fs::write(case_dir.join("assets/bg.png"), "fakebg").unwrap();

        // Export
        let zip_path = tmp.path().join("roundtrip.aaocase");
        export_aaocase(77005, engine1.path(), &zip_path, None, None, true).unwrap();

        // Import into fresh engine
        let imported = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap().manifest;
        assert_eq!(imported.case_id, original.case_id);
        assert_eq!(imported.title, original.title);
        assert_eq!(imported.author, original.author);
        assert_eq!(imported.language, original.language);
        assert_eq!(imported.format, original.format);
        assert!(imported.sequence.is_some());

        // Verify asset preserved
        let case2 = engine2.path().join("case/77005");
        assert_eq!(fs::read_to_string(case2.join("assets/bg.png")).unwrap(), "fakebg");
    }

    /// Regression: sequence export + import roundtrip preserves all case data.
    #[test]
    fn test_export_import_sequence_roundtrip_data_preserved() {
        let engine1 = tempfile::tempdir().unwrap();
        let engine2 = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        for &case_id in &[77006u32, 77007] {
            let case_dir = engine1.path().join("case").join(case_id.to_string());
            fs::create_dir_all(case_dir.join("assets")).unwrap();
            let manifest = CaseManifest {
                case_id,
                title: format!("Seq Part {}", case_id),
                author: "SeqAuthor".to_string(),
                language: "en".to_string(),
                download_date: "2025-01-01T00:00:00Z".to_string(),
                format: "v6".to_string(),
                sequence: Some(serde_json::json!({"title": "Seq Test", "list": [{"id": 77006}, {"id": 77007}]})),
                assets: AssetSummary { case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 5 },
                asset_map: HashMap::new(),
                failed_assets: vec![],
                has_plugins: false,
                has_case_config: false,
            };
            write_manifest(&manifest, &case_dir).unwrap();
            fs::write(case_dir.join("trial_info.json"), format!(r#"{{"id":{}}}"#, case_id)).unwrap();
            fs::write(case_dir.join("trial_data.json"), format!(r#"{{"frames":[0,{{"id":{}}}]}}"#, case_id)).unwrap();
            fs::write(case_dir.join("assets/sprite.png"), format!("data{}", case_id)).unwrap();
        }

        let seq_list = serde_json::json!([{"id": 77006, "title": "P1"}, {"id": 77007, "title": "P2"}]);
        let zip_path = tmp.path().join("seq_roundtrip.aaocase");
        export_sequence(&[77006, 77007], "Seq Test", &seq_list, engine1.path(), &zip_path, None, None, true).unwrap();

        let manifest = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap().manifest;
        assert_eq!(manifest.case_id, 77006);

        // Verify both cases present with correct data
        for &case_id in &[77006u32, 77007] {
            let case_dir = engine2.path().join("case").join(case_id.to_string());
            assert!(case_dir.join("manifest.json").exists());
            let data = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
            assert!(data.contains(&format!("\"id\":{}", case_id)));
            let asset = fs::read_to_string(case_dir.join("assets/sprite.png")).unwrap();
            assert_eq!(asset, format!("data{}", case_id));
        }
    }

    // --- Feature tests for saves in export/import (Phase A) ---

    /// Export a single case with saves included — ZIP should contain saves.json.
    #[test]
    fn test_export_aaocase_with_saves() {
        let engine = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        let case_dir = engine.path().join("case/78001");
        fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id: 78001,
            title: "With Saves".to_string(),
            author: "Tester".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), r#"{"id":78001}"#).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();

        let saves = serde_json::json!({
            "78001": {
                "1710000000000": "{\"frame\":5,\"health\":100}"
            }
        });

        let export_path = tmp.path().join("with_saves.aaocase");
        export_aaocase(78001, engine.path(), &export_path, None, Some(&saves), true).unwrap();

        // Verify ZIP contains saves.json
        let file = fs::File::open(&export_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let entry_names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(entry_names.contains(&"saves.json".to_string()),
            "Export with saves should contain saves.json");
        assert!(entry_names.contains(&"manifest.json".to_string()));

        // Verify saves.json content
        let saves_content = read_zip_text(&mut archive, "saves.json").unwrap();
        let parsed: Value = serde_json::from_str(&saves_content).unwrap();
        assert!(parsed["78001"].is_object());
        assert!(parsed["78001"]["1710000000000"].is_string());
    }

    /// Import a ZIP with saves.json — result should include saves.
    #[test]
    fn test_import_aaocase_with_saves() {
        use std::io::Write;

        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Build a ZIP manually with saves.json
        let zip_path = tmp.path().join("with_saves.aaocase");
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        // manifest.json
        let manifest = serde_json::json!({
            "case_id": 78002,
            "title": "Import Saves Test",
            "author": "Tester",
            "language": "en",
            "download_date": "2025-01-01T00:00:00Z",
            "format": "v6",
            "sequence": null,
            "assets": { "case_specific": 0, "shared_defaults": 0, "total_downloaded": 0, "total_size_bytes": 0 },
            "asset_map": {},
            "failed_assets": []
        });
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(serde_json::to_string(&manifest).unwrap().as_bytes()).unwrap();

        // trial_info.json
        zip.start_file("trial_info.json", options).unwrap();
        zip.write_all(br#"{"id":78002}"#).unwrap();

        // trial_data.json
        zip.start_file("trial_data.json", options).unwrap();
        zip.write_all(br#"{"frames":[0]}"#).unwrap();

        // saves.json
        let saves = serde_json::json!({
            "78002": {
                "1710000000000": "{\"frame\":3}",
                "1710001000000": "{\"frame\":7}"
            }
        });
        zip.start_file("saves.json", options).unwrap();
        zip.write_all(serde_json::to_string(&saves).unwrap().as_bytes()).unwrap();

        zip.finish().unwrap();

        // Import
        let result = import_aaocase_zip(&zip_path, engine.path(), None).unwrap();
        assert_eq!(result.manifest.case_id, 78002);
        assert!(result.saves.is_some(), "Import result should contain saves");

        let imported_saves = result.saves.unwrap();
        assert!(imported_saves["78002"].is_object());
        let case_saves = imported_saves["78002"].as_object().unwrap();
        assert_eq!(case_saves.len(), 2, "Should have 2 save entries");
    }

    /// Import a ZIP without saves.json — saves should be None.
    #[test]
    fn test_import_aaocase_without_saves_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        let zip_path = create_test_aaocase(tmp.path(), 78003);
        let result = import_aaocase_zip(&zip_path, engine.path(), None).unwrap();
        assert_eq!(result.manifest.case_id, 78003);
        assert!(result.saves.is_none(), "Import without saves.json should have saves=None");
    }

    /// Full roundtrip: export with saves → import → saves preserved.
    #[test]
    fn test_export_import_saves_roundtrip() {
        let engine1 = tempfile::tempdir().unwrap();
        let engine2 = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        let case_dir = engine1.path().join("case/78004");
        fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id: 78004,
            title: "Saves Roundtrip".to_string(),
            author: "Tester".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), r#"{"id":78004}"#).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();

        let saves = serde_json::json!({
            "78004": {
                "1710000000000": "{\"health\":80,\"frame\":10}",
                "1710005000000": "{\"health\":120,\"frame\":1}"
            }
        });

        // Export with saves
        let zip_path = tmp.path().join("saves_roundtrip.aaocase");
        export_aaocase(78004, engine1.path(), &zip_path, None, Some(&saves), true).unwrap();

        // Import
        let result = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap();
        assert_eq!(result.manifest.case_id, 78004);
        assert!(result.saves.is_some(), "Saves should survive roundtrip");

        let imported_saves = result.saves.unwrap();
        let case_saves = imported_saves["78004"].as_object().unwrap();
        assert_eq!(case_saves.len(), 2);
        assert!(case_saves.contains_key("1710000000000"));
        assert!(case_saves.contains_key("1710005000000"));
    }

    /// Export a sequence with saves — ZIP should contain saves.json.
    #[test]
    fn test_export_sequence_with_saves() {
        let engine = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        for &case_id in &[78005u32, 78006] {
            let case_dir = engine.path().join("case").join(case_id.to_string());
            fs::create_dir_all(&case_dir).unwrap();
            let manifest = CaseManifest {
                case_id,
                title: format!("Part {}", case_id),
                author: "Tester".to_string(),
                language: "en".to_string(),
                download_date: "2025-01-01T00:00:00Z".to_string(),
                format: "v6".to_string(),
                sequence: None,
                assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
                asset_map: HashMap::new(),
                failed_assets: vec![],
                has_plugins: false,
                has_case_config: false,
            };
            write_manifest(&manifest, &case_dir).unwrap();
            fs::write(case_dir.join("trial_info.json"), format!(r#"{{"id":{}}}"#, case_id)).unwrap();
            fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
        }

        let saves = serde_json::json!({
            "78005": { "1710000000000": "{\"frame\":2}" },
            "78006": { "1710001000000": "{\"frame\":5}" }
        });

        let seq_list = serde_json::json!([{"id": 78005, "title": "P1"}, {"id": 78006, "title": "P2"}]);
        let zip_path = tmp.path().join("seq_saves.aaocase");
        export_sequence(&[78005, 78006], "Seq Saves", &seq_list, engine.path(), &zip_path, None, Some(&saves), true).unwrap();

        // Verify ZIP contains saves.json
        let file = fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let entry_names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(entry_names.contains(&"saves.json".to_string()),
            "Sequence export with saves should contain saves.json");
        assert!(entry_names.contains(&"sequence.json".to_string()));

        // Verify saves content
        let saves_str = read_zip_text(&mut archive, "saves.json").unwrap();
        let parsed: Value = serde_json::from_str(&saves_str).unwrap();
        assert!(parsed["78005"].is_object());
        assert!(parsed["78006"].is_object());
    }

    /// Sequence export+import roundtrip with saves.
    #[test]
    fn test_export_import_sequence_saves_roundtrip() {
        let engine1 = tempfile::tempdir().unwrap();
        let engine2 = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        for &case_id in &[78007u32, 78008] {
            let case_dir = engine1.path().join("case").join(case_id.to_string());
            fs::create_dir_all(&case_dir).unwrap();
            let manifest = CaseManifest {
                case_id,
                title: format!("Seq Saves RT {}", case_id),
                author: "Tester".to_string(),
                language: "en".to_string(),
                download_date: "2025-01-01T00:00:00Z".to_string(),
                format: "v6".to_string(),
                sequence: None,
                assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
                asset_map: HashMap::new(),
                failed_assets: vec![],
                has_plugins: false,
                has_case_config: false,
            };
            write_manifest(&manifest, &case_dir).unwrap();
            fs::write(case_dir.join("trial_info.json"), format!(r#"{{"id":{}}}"#, case_id)).unwrap();
            fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
        }

        let saves = serde_json::json!({
            "78007": { "1710000000000": "{\"frame\":1}" },
            "78008": { "1710002000000": "{\"frame\":3}" }
        });

        let seq_list = serde_json::json!([{"id": 78007, "title": "P1"}, {"id": 78008, "title": "P2"}]);
        let zip_path = tmp.path().join("seq_saves_rt.aaocase");
        export_sequence(&[78007, 78008], "Seq Saves RT", &seq_list, engine1.path(), &zip_path, None, Some(&saves), true).unwrap();

        // Import
        let result = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap();
        assert_eq!(result.manifest.case_id, 78007);
        assert!(result.saves.is_some(), "Sequence saves should survive roundtrip");

        let imported_saves = result.saves.unwrap();
        assert!(imported_saves["78007"].is_object());
        assert!(imported_saves["78008"].is_object());
    }

    /// Export with None saves should not include saves.json (same as regression test, but confirms new API).
    #[test]
    fn test_export_with_none_saves_is_backward_compatible() {
        let engine = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        let case_dir = engine.path().join("case/78009");
        fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id: 78009,
            title: "None Saves Compat".to_string(),
            author: "T".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), r#"{"id":78009}"#).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();

        // Export with None saves (backward compatible)
        let zip_path = tmp.path().join("none_saves.aaocase");
        export_aaocase(78009, engine.path(), &zip_path, None, None, true).unwrap();

        let file = fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let entry_names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(!entry_names.contains(&"saves.json".to_string()));

        // Import should have saves=None
        let engine2 = tempfile::tempdir().unwrap();
        let result = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap();
        assert!(result.saves.is_none());
    }

    #[test]
    fn test_extract_default_sprite_mappings() {
        let html = r#"
function getDefaultSpriteUrl(base, sprite_id, status)
{
if (base === 'Phoenix' && sprite_id === 1 && status === 'talking') return 'assets/1-abc.gif';
if (base === 'Phoenix' && sprite_id === 1 && status === 'still') return 'assets/1-def.gif';
if (base === 'Edgeworth' && sprite_id === 3 && status === 'startup') return 'assets/3-ghi.gif';
return 'data:image/gif;base64,'
}
"#;
        let mappings = extract_default_sprite_mappings(html);
        assert_eq!(mappings.len(), 3);

        assert_eq!(mappings[0].base, "Phoenix");
        assert_eq!(mappings[0].sprite_id, 1);
        assert_eq!(mappings[0].status, "talking");
        assert_eq!(mappings[0].asset_path, "assets/1-abc.gif");

        assert_eq!(mappings[1].base, "Phoenix");
        assert_eq!(mappings[1].status, "still");

        assert_eq!(mappings[2].base, "Edgeworth");
        assert_eq!(mappings[2].sprite_id, 3);
        assert_eq!(mappings[2].status, "startup");
    }

    #[test]
    fn test_copy_default_sprites() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Create fake asset files
        let assets_dir = source.path().join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("1-abc.gif"), b"talking_gif").unwrap();
        fs::write(assets_dir.join("1-def.gif"), b"still_gif").unwrap();
        fs::write(assets_dir.join("3-ghi.gif"), b"startup_gif").unwrap();

        let mappings = vec![
            DefaultSpriteMapping { base: "Phoenix".into(), sprite_id: 1, status: "talking".into(), asset_path: "assets/1-abc.gif".into() },
            DefaultSpriteMapping { base: "Phoenix".into(), sprite_id: 1, status: "still".into(), asset_path: "assets/1-def.gif".into() },
            DefaultSpriteMapping { base: "Edgeworth".into(), sprite_id: 3, status: "startup".into(), asset_path: "assets/3-ghi.gif".into() },
        ];

        let (copied, bytes) = copy_default_sprites(&mappings, source.path(), engine.path());
        assert_eq!(copied, 3);
        assert!(bytes > 0);

        // Verify files were placed correctly
        assert!(engine.path().join("defaults/images/chars/Phoenix/1.gif").exists());
        assert!(engine.path().join("defaults/images/charsStill/Phoenix/1.gif").exists());
        assert!(engine.path().join("defaults/images/charsStartup/Edgeworth/3.gif").exists());

        // Running again should skip existing files
        let (copied2, _) = copy_default_sprites(&mappings, source.path(), engine.path());
        assert_eq!(copied2, 0);
    }

    #[test]
    fn test_import_aaoplug_extracts_to_case() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();

        // Create a fake case directory with minimal manifest
        let case_dir = engine_dir.join("case/99999");
        std::fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id: 99999,
            title: "Test".to_string(),
            author: "Test".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01".to_string(),
            format: "test".to_string(),
            sequence: None,
            assets: crate::downloader::manifest::AssetSummary {
                case_specific: 0,
                shared_defaults: 0,
                total_downloaded: 0,
                total_size_bytes: 0,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        // Create a .aaoplug ZIP in memory
        let plug_path = dir.path().join("test.aaoplug");
        {
            let file = std::fs::File::create(&plug_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();

            zip.start_file("manifest.json", options).unwrap();
            std::io::Write::write_all(&mut zip, b"{\"scripts\":[\"test_plugin.js\"]}").unwrap();

            zip.start_file("test_plugin.js", options).unwrap();
            std::io::Write::write_all(&mut zip, b"console.log('test plugin');").unwrap();

            zip.start_file("assets/test_sound.opus", options).unwrap();
            std::io::Write::write_all(&mut zip, b"fake audio data").unwrap();

            zip.finish().unwrap();
        }

        // Import the plugin
        let result = import_aaoplug(&plug_path, &[99999], engine_dir);
        assert!(result.is_ok(), "import_aaoplug should succeed");
        let imported = result.unwrap();
        assert_eq!(imported, vec![99999]);

        // Verify files were extracted
        assert!(case_dir.join("plugins/manifest.json").exists());
        assert!(case_dir.join("plugins/test_plugin.js").exists());
        assert!(case_dir.join("plugins/assets/test_sound.opus").exists());

        // Verify case manifest updated
        let updated_manifest = read_manifest(&case_dir).unwrap();
        assert!(updated_manifest.has_plugins);
    }

    #[test]
    fn test_import_aaoplug_invalid_zip() {
        let dir = tempfile::tempdir().unwrap();
        let bad_path = dir.path().join("bad.aaoplug");
        std::fs::write(&bad_path, "not a zip").unwrap();
        let result = import_aaoplug(&bad_path, &[1], dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_import_aaoplug_nonexistent_case() {
        let dir = tempfile::tempdir().unwrap();
        let plug_path = dir.path().join("test.aaoplug");
        {
            let file = std::fs::File::create(&plug_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("manifest.json", options).unwrap();
            std::io::Write::write_all(&mut zip, b"{}").unwrap();
            zip.finish().unwrap();
        }
        let result = import_aaoplug(&plug_path, &[99998], dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty(), "Should skip non-existent case");
    }

    #[test]
    fn test_attach_plugin_code() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();

        // Create a fake case
        let case_dir = engine_dir.join("case/88888");
        std::fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id: 88888,
            title: "Test".to_string(),
            author: "Test".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01".to_string(),
            format: "test".to_string(),
            sequence: None,
            assets: crate::downloader::manifest::AssetSummary {
                case_specific: 0,
                shared_defaults: 0,
                total_downloaded: 0,
                total_size_bytes: 0,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        // Attach plugin code
        let result = attach_plugin_code(
            "console.log('hello');",
            "my_plugin.js",
            &[88888],
            engine_dir,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![88888]);

        // Verify file exists
        assert!(case_dir.join("plugins/my_plugin.js").exists());
        let content = std::fs::read_to_string(case_dir.join("plugins/my_plugin.js")).unwrap();
        assert_eq!(content, "console.log('hello');");

        // Verify plugin manifest
        let plugin_manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(case_dir.join("plugins/manifest.json")).unwrap()
        ).unwrap();
        let scripts = plugin_manifest.get("scripts").unwrap().as_array().unwrap();
        assert!(scripts.iter().any(|s| s.as_str() == Some("my_plugin.js")));

        // Verify case manifest updated
        let updated = read_manifest(&case_dir).unwrap();
        assert!(updated.has_plugins);
    }

    #[test]
    fn test_list_plugins_empty_case() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let case_dir = engine_dir.join("case/90001");
        std::fs::create_dir_all(&case_dir).unwrap();

        let result = list_plugins(90001, engine_dir);
        assert!(result.is_ok());
        let val = result.unwrap();
        let scripts = val.get("scripts").unwrap().as_array().unwrap();
        assert!(scripts.is_empty());
    }

    #[test]
    fn test_list_plugins_with_plugins() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let case_dir = engine_dir.join("case/90002");
        std::fs::create_dir_all(&case_dir).unwrap();

        let manifest = CaseManifest {
            case_id: 90002,
            title: "Test".to_string(),
            author: "Test".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01".to_string(),
            format: "test".to_string(),
            sequence: None,
            assets: AssetSummary {
                case_specific: 0,
                shared_defaults: 0,
                total_downloaded: 0,
                total_size_bytes: 0,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        attach_plugin_code("// a", "a.js", &[90002], engine_dir).unwrap();
        attach_plugin_code("// b", "b.js", &[90002], engine_dir).unwrap();

        let result = list_plugins(90002, engine_dir).unwrap();
        let scripts = result.get("scripts").unwrap().as_array().unwrap();
        assert_eq!(scripts.len(), 2);
        let names: Vec<&str> = scripts.iter().map(|s| s.as_str().unwrap()).collect();
        assert!(names.contains(&"a.js"));
        assert!(names.contains(&"b.js"));
    }

    #[test]
    fn test_remove_plugin_updates_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let case_dir = engine_dir.join("case/90003");
        std::fs::create_dir_all(&case_dir).unwrap();

        let manifest = CaseManifest {
            case_id: 90003,
            title: "Test".to_string(),
            author: "Test".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01".to_string(),
            format: "test".to_string(),
            sequence: None,
            assets: AssetSummary {
                case_specific: 0,
                shared_defaults: 0,
                total_downloaded: 0,
                total_size_bytes: 0,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        attach_plugin_code("// x", "x.js", &[90003], engine_dir).unwrap();
        attach_plugin_code("// y", "y.js", &[90003], engine_dir).unwrap();

        remove_plugin(90003, "x.js", engine_dir).unwrap();

        // x.js file should be gone
        assert!(!case_dir.join("plugins/x.js").exists());
        // y.js should still exist
        assert!(case_dir.join("plugins/y.js").exists());

        // Plugin manifest should only list y.js
        let val = list_plugins(90003, engine_dir).unwrap();
        let scripts = val.get("scripts").unwrap().as_array().unwrap();
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].as_str().unwrap(), "y.js");

        // Case still has plugins
        let updated = read_manifest(&case_dir).unwrap();
        assert!(updated.has_plugins);
    }

    #[test]
    fn test_remove_plugin_sets_has_plugins_false() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let case_dir = engine_dir.join("case/90004");
        std::fs::create_dir_all(&case_dir).unwrap();

        let manifest = CaseManifest {
            case_id: 90004,
            title: "Test".to_string(),
            author: "Test".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01".to_string(),
            format: "test".to_string(),
            sequence: None,
            assets: AssetSummary {
                case_specific: 0,
                shared_defaults: 0,
                total_downloaded: 0,
                total_size_bytes: 0,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        attach_plugin_code("// only", "only.js", &[90004], engine_dir).unwrap();
        assert!(read_manifest(&case_dir).unwrap().has_plugins);

        remove_plugin(90004, "only.js", engine_dir).unwrap();

        // No more plugins — has_plugins should be false
        let updated = read_manifest(&case_dir).unwrap();
        assert!(!updated.has_plugins);

        // File gone
        assert!(!case_dir.join("plugins/only.js").exists());
    }

    fn create_test_case_for_save(engine_dir: &Path, case_id: u32) -> std::path::PathBuf {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        std::fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id,
            title: format!("Test Case {}", case_id),
            author: "Tester".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01".to_string(),
            format: "test".to_string(),
            sequence: None,
            assets: AssetSummary {
                case_specific: 0,
                shared_defaults: 0,
                total_downloaded: 0,
                total_size_bytes: 0,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        case_dir
    }

    #[test]
    fn test_export_aaosave_basic() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 50001);

        let saves = serde_json::json!({
            "50001": { "1700000000000": "{\"trial_id\":50001}" }
        });
        let dest = engine_dir.join("test.aaosave");
        let size = export_aaosave(&[50001], &saves, false, &dest, engine_dir).unwrap();
        assert!(size > 0);

        // Verify ZIP contents
        let file = std::fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert!(archive.by_name("saves.json").is_ok());
        assert!(archive.by_name("metadata.json").is_ok());

        let meta_text = read_zip_text(&mut archive, "metadata.json").unwrap();
        let meta: serde_json::Value = serde_json::from_str(&meta_text).unwrap();
        assert_eq!(meta["version"], 1);
        let export_date = meta["export_date"].as_str().unwrap();
        assert!(export_date.contains("T"), "export_date should be ISO-8601: {}", export_date);
        assert!(export_date.ends_with("Z"), "export_date should end with Z: {}", export_date);
        assert_eq!(meta["has_plugins"], false);
        let cases = meta["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0]["id"], 50001);
        assert_eq!(cases[0]["save_count"], 1);
    }

    #[test]
    fn test_export_aaosave_with_plugins() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 50002);
        attach_plugin_code("// test", "test.js", &[50002], engine_dir).unwrap();

        let saves = serde_json::json!({
            "50002": { "1700000000000": "{\"trial_id\":50002}" }
        });
        let dest = engine_dir.join("test_plug.aaosave");
        export_aaosave(&[50002], &saves, true, &dest, engine_dir).unwrap();

        let file = std::fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert!(archive.by_name("plugins/50002/manifest.json").is_ok());
        assert!(archive.by_name("plugins/50002/test.js").is_ok());

        let meta_text = read_zip_text(&mut archive, "metadata.json").unwrap();
        let meta: serde_json::Value = serde_json::from_str(&meta_text).unwrap();
        assert_eq!(meta["has_plugins"], true);
    }

    #[test]
    fn test_import_aaosave_basic() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();

        // Create a .aaosave manually
        let dest = engine_dir.join("import_test.aaosave");
        let file = std::fs::File::create(&dest).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        let saves = serde_json::json!({ "60001": { "999": "{\"trial_id\":60001}" } });
        zip.start_file("saves.json", options).unwrap();
        io::Write::write_all(&mut zip, serde_json::to_string(&saves).unwrap().as_bytes()).unwrap();

        let meta = serde_json::json!({ "version": 1, "cases": [], "has_plugins": false });
        zip.start_file("metadata.json", options).unwrap();
        io::Write::write_all(&mut zip, serde_json::to_string(&meta).unwrap().as_bytes()).unwrap();
        zip.finish().unwrap();

        let result = import_aaosave(&dest, engine_dir).unwrap();
        assert_eq!(result.saves["60001"]["999"], "{\"trial_id\":60001}");
        assert!(result.plugins_installed.is_empty());
    }

    #[test]
    fn test_import_aaosave_with_plugins() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let case_dir = create_test_case_for_save(engine_dir, 60002);

        // Create .aaosave with plugins
        let dest = engine_dir.join("plug_import.aaosave");
        let file = std::fs::File::create(&dest).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();

        let saves = serde_json::json!({ "60002": { "111": "{}" } });
        zip.start_file("saves.json", options).unwrap();
        io::Write::write_all(&mut zip, serde_json::to_string(&saves).unwrap().as_bytes()).unwrap();

        let meta = serde_json::json!({ "version": 1, "cases": [], "has_plugins": true });
        zip.start_file("metadata.json", options).unwrap();
        io::Write::write_all(&mut zip, serde_json::to_string(&meta).unwrap().as_bytes()).unwrap();

        zip.start_file("plugins/60002/manifest.json", options).unwrap();
        io::Write::write_all(&mut zip, b"{\"scripts\":[\"plugin.js\"]}").unwrap();

        zip.start_file("plugins/60002/plugin.js", options).unwrap();
        io::Write::write_all(&mut zip, b"console.log('hi');").unwrap();
        zip.finish().unwrap();

        let result = import_aaosave(&dest, engine_dir).unwrap();
        assert_eq!(result.plugins_installed, vec![60002]);
        assert!(case_dir.join("plugins/manifest.json").exists());
        assert!(case_dir.join("plugins/plugin.js").exists());
        assert!(read_manifest(&case_dir).unwrap().has_plugins);
    }

    #[test]
    fn test_import_aaosave_missing_saves() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("bad.aaosave");
        let file = std::fs::File::create(&dest).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("metadata.json", options).unwrap();
        io::Write::write_all(&mut zip, b"{}").unwrap();
        zip.finish().unwrap();

        let result = import_aaosave(&dest, dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("saves.json"));
    }

    #[test]
    fn test_export_import_aaosave_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 70001);
        attach_plugin_code("// roundtrip", "rt.js", &[70001], engine_dir).unwrap();

        let saves = serde_json::json!({
            "70001": {
                "1000": "{\"trial_id\":70001,\"frame\":5}",
                "2000": "{\"trial_id\":70001,\"frame\":10}"
            }
        });

        let dest = engine_dir.join("roundtrip.aaosave");
        export_aaosave(&[70001], &saves, true, &dest, engine_dir).unwrap();

        // Import into a fresh engine dir with the same case
        let dir2 = tempfile::tempdir().unwrap();
        let engine_dir2 = dir2.path();
        create_test_case_for_save(engine_dir2, 70001);

        let result = import_aaosave(&dest, engine_dir2).unwrap();

        // Saves preserved
        assert_eq!(result.saves["70001"]["1000"], "{\"trial_id\":70001,\"frame\":5}");
        assert_eq!(result.saves["70001"]["2000"], "{\"trial_id\":70001,\"frame\":10}");

        // Plugins installed
        assert_eq!(result.plugins_installed, vec![70001]);
        let case_dir2 = engine_dir2.join("case/70001");
        assert!(case_dir2.join("plugins/rt.js").exists());
        assert!(read_manifest(&case_dir2).unwrap().has_plugins);
    }

    #[test]
    fn test_attach_global_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        attach_global_plugin_code("// global", "global.js", engine_dir).unwrap();
        assert!(engine_dir.join("plugins/global.js").exists());
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
        ).unwrap();
        assert!(manifest["scripts"].as_array().unwrap().iter().any(|s| s.as_str() == Some("global.js")));
    }

    #[test]
    fn test_remove_global_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        attach_global_plugin_code("// global", "global.js", engine_dir).unwrap();
        remove_global_plugin("global.js", engine_dir).unwrap();
        assert!(!engine_dir.join("plugins/global.js").exists());
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
        ).unwrap();
        assert!(manifest["scripts"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_toggle_global_plugin() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        attach_global_plugin_code("// global", "g.js", engine_dir).unwrap();
        toggle_global_plugin("g.js", false, engine_dir).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
        ).unwrap();
        assert!(manifest["disabled"].as_array().unwrap().iter().any(|s| s.as_str() == Some("g.js")));

        toggle_global_plugin("g.js", true, engine_dir).unwrap();
        let manifest2: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
        ).unwrap();
        assert!(manifest2["disabled"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_toggle_plugin_disables() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 80001);
        attach_plugin_code("// test", "test.js", &[80001], engine_dir).unwrap();

        toggle_plugin(80001, "test.js", false, engine_dir).unwrap();

        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(engine_dir.join("case/80001/plugins/manifest.json")).unwrap()
        ).unwrap();
        let disabled = manifest["disabled"].as_array().unwrap();
        assert!(disabled.iter().any(|s| s.as_str() == Some("test.js")));
    }

    #[test]
    fn test_toggle_plugin_enables() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 80002);
        attach_plugin_code("// test", "test.js", &[80002], engine_dir).unwrap();

        // Disable then re-enable
        toggle_plugin(80002, "test.js", false, engine_dir).unwrap();
        toggle_plugin(80002, "test.js", true, engine_dir).unwrap();

        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(engine_dir.join("case/80002/plugins/manifest.json")).unwrap()
        ).unwrap();
        let disabled = manifest.get("disabled").and_then(|d| d.as_array());
        assert!(disabled.is_none() || disabled.unwrap().is_empty());
    }

    // ============================================================
    // Global Plugin System — Phase A Tests
    // ============================================================

    #[test]
    fn test_migrate_old_format_to_new() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("manifest.json"),
            r#"{"scripts":["a.js","b.js"],"disabled":["b.js"]}"#).unwrap();

        migrate_global_manifest(engine_dir).unwrap();

        let text = std::fs::read_to_string(plugins_dir.join("manifest.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(val.get("plugins").is_some());
        let plugins = val["plugins"].as_object().unwrap();
        assert_eq!(plugins["a.js"]["scope"]["all"], true);
        assert_eq!(plugins["b.js"]["scope"]["all"], false);
        assert!(val.get("disabled").is_none()); // old field removed
    }

    #[test]
    fn test_migrate_already_new_stays_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        let original = r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{"default":{"x":1}}}}}"#;
        std::fs::write(plugins_dir.join("manifest.json"), original).unwrap();

        migrate_global_manifest(engine_dir).unwrap();

        let text = std::fs::read_to_string(plugins_dir.join("manifest.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        // Params should still be there (not wiped)
        assert_eq!(val["plugins"]["a.js"]["params"]["default"]["x"], 1);
    }

    #[test]
    fn test_migrate_missing_file_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let result = migrate_global_manifest(dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_scope_all_matches_any_case() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
        std::fs::write(plugins_dir.join("manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{}}}}"#).unwrap();
        // Create a case dir
        let case_dir = engine_dir.join("case/99999");
        std::fs::create_dir_all(&case_dir).unwrap();

        let resolved = resolve_plugins_for_case(99999, engine_dir).unwrap();
        let active = resolved["active"].as_array().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0]["script"], "a.js");
    }

    #[test]
    fn test_scope_case_ids_matching() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
        std::fs::write(plugins_dir.join("manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"case_ids":[12345],"sequence_titles":[],"collection_ids":[]},"params":{}}}}"#).unwrap();
        std::fs::create_dir_all(engine_dir.join("case/12345")).unwrap();

        let resolved = resolve_plugins_for_case(12345, engine_dir).unwrap();
        assert_eq!(resolved["active"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_scope_case_ids_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
        std::fs::write(plugins_dir.join("manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"case_ids":[12345],"sequence_titles":[],"collection_ids":[]},"params":{}}}}"#).unwrap();
        std::fs::create_dir_all(engine_dir.join("case/99999")).unwrap();

        let resolved = resolve_plugins_for_case(99999, engine_dir).unwrap();
        assert_eq!(resolved["active"].as_array().unwrap().len(), 0);
        assert_eq!(resolved["available"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_scope_disabled_everywhere() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
        std::fs::write(plugins_dir.join("manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false},"params":{}}}}"#).unwrap();
        std::fs::create_dir_all(engine_dir.join("case/11111")).unwrap();

        let resolved = resolve_plugins_for_case(11111, engine_dir).unwrap();
        assert_eq!(resolved["active"].as_array().unwrap().len(), 0);
        assert_eq!(resolved["available"].as_array().unwrap().len(), 1);
        assert!(resolved["available"][0]["reason"].as_str().unwrap().contains("no matching scope"));
    }

    #[test]
    fn test_params_defaults_only() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("a.js"), "//").unwrap();
        std::fs::write(plugins_dir.join("manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{"default":{"font":"Arial","size":14}}}}}"#).unwrap();
        std::fs::create_dir_all(engine_dir.join("case/1")).unwrap();

        let resolved = resolve_plugins_for_case(1, engine_dir).unwrap();
        let params = &resolved["active"][0]["params"];
        assert_eq!(params["font"], "Arial");
        assert_eq!(params["size"], 14);
    }

    #[test]
    fn test_params_case_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("a.js"), "//").unwrap();
        std::fs::write(plugins_dir.join("manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{"default":{"font":"Arial","size":14},"by_case":{"42":{"font":"sans-serif","size":10}}}}}}"#).unwrap();
        std::fs::create_dir_all(engine_dir.join("case/42")).unwrap();

        let resolved = resolve_plugins_for_case(42, engine_dir).unwrap();
        let params = &resolved["active"][0]["params"];
        assert_eq!(params["font"], "sans-serif");
        assert_eq!(params["size"], 10);
    }

    #[test]
    fn test_params_partial_override_inherits() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        let plugins_dir = engine_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("a.js"), "//").unwrap();
        std::fs::write(plugins_dir.join("manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{"default":{"font":"Arial","size":14},"by_case":{"42":{"font":"Calibri"}}}}}}"#).unwrap();
        std::fs::create_dir_all(engine_dir.join("case/42")).unwrap();

        let resolved = resolve_plugins_for_case(42, engine_dir).unwrap();
        let params = &resolved["active"][0]["params"];
        assert_eq!(params["font"], "Calibri"); // overridden
        assert_eq!(params["size"], 14); // inherited from default
    }

    #[test]
    fn test_duplicate_found_in_global() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
        std::fs::write(engine_dir.join("plugins/test.js"), "console.log('hello');").unwrap();

        let matches = check_plugin_duplicate("console.log('hello');", engine_dir);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].filename, "test.js");
        assert_eq!(matches[0].location, "global");
    }

    #[test]
    fn test_duplicate_found_in_case() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        std::fs::create_dir_all(engine_dir.join("case/555/plugins")).unwrap();
        std::fs::write(engine_dir.join("case/555/plugins/p.js"), "// dup").unwrap();

        let matches = check_plugin_duplicate("// dup", engine_dir);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].location, "case 555");
    }

    #[test]
    fn test_no_duplicate_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let matches = check_plugin_duplicate("unique code", dir.path());
        assert!(matches.is_empty());
    }

    #[test]
    fn test_duplicate_whitespace_trimmed() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
        std::fs::write(engine_dir.join("plugins/t.js"), "  code  \n").unwrap();

        let matches = check_plugin_duplicate("\n  code  ", engine_dir);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_set_scope_updates_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
        std::fs::write(engine_dir.join("plugins/manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false},"params":{}}}}"#).unwrap();

        let new_scope = serde_json::json!({"all": true, "case_ids": [1,2,3]});
        set_global_plugin_scope("a.js", &new_scope, engine_dir).unwrap();

        let text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(val["plugins"]["a.js"]["scope"]["all"], true);
        assert_eq!(val["plugins"]["a.js"]["scope"]["case_ids"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_set_params_default() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
        std::fs::write(engine_dir.join("plugins/manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{}}}}"#).unwrap();

        set_global_plugin_params("a.js", "default", "", &serde_json::json!({"font":"Arial"}), engine_dir).unwrap();

        let text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(val["plugins"]["a.js"]["params"]["default"]["font"], "Arial");
    }

    #[test]
    fn test_set_params_by_case() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        std::fs::create_dir_all(engine_dir.join("plugins")).unwrap();
        std::fs::write(engine_dir.join("plugins/manifest.json"),
            r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true},"params":{}}}}"#).unwrap();

        set_global_plugin_params("a.js", "by_case", "69063", &serde_json::json!({"font":"Mono"}), engine_dir).unwrap();

        let text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(val["plugins"]["a.js"]["params"]["by_case"]["69063"]["font"], "Mono");
    }

    #[test]
    fn test_promote_copies_file() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 55555);
        attach_plugin_code("// promote me", "prom.js", &[55555], engine_dir).unwrap();

        let scope = serde_json::json!({"all": true});
        promote_plugin_to_global(55555, "prom.js", &scope, engine_dir).unwrap();

        assert!(engine_dir.join("plugins/prom.js").exists());
        let content = std::fs::read_to_string(engine_dir.join("plugins/prom.js")).unwrap();
        assert_eq!(content, "// promote me");
    }

    #[test]
    fn test_promote_updates_global_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 55556);
        attach_plugin_code("// prom2", "p2.js", &[55556], engine_dir).unwrap();

        let scope = serde_json::json!({"all": false, "case_ids": [1, 2]});
        promote_plugin_to_global(55556, "p2.js", &scope, engine_dir).unwrap();

        let text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(val["scripts"].as_array().unwrap().iter().any(|s| s == "p2.js"));
        assert_eq!(val["plugins"]["p2.js"]["scope"]["case_ids"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_promote_removes_from_case() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 55557);
        attach_plugin_code("// prom3", "p3.js", &[55557], engine_dir).unwrap();

        promote_plugin_to_global(55557, "p3.js", &serde_json::json!({"all":true}), engine_dir).unwrap();

        // Case plugin file should be gone
        assert!(!engine_dir.join("case/55557/plugins/p3.js").exists());
    }

    #[test]
    fn test_promote_nonexistent_fails() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();
        create_test_case_for_save(engine_dir, 55558);

        let result = promote_plugin_to_global(55558, "nope.js", &serde_json::json!({"all":true}), engine_dir);
        assert!(result.is_err());
    }

    // --- Batch import tests ---

    fn make_aaoffline_case(dir: &Path, case_id: u32, title: &str) {
        fs::create_dir_all(dir).unwrap();
        let html = format!(
            r#"<html>
<script>
var trial_information = {{"author":"BatchTester","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":{},"language":"en","last_edit_date":1000000,"sequence":null,"title":"{}"}};
var initial_trial_data = {{"profiles":[0,{{"icon":"","short_name":"Hero","custom_sprites":[]}}],"frames":[0],"evidence":[0],"places":[0],"cross_examinations":[0]}};
</script>
</html>"#,
            case_id, title
        );
        fs::write(dir.join("index.html"), html).unwrap();
    }

    #[test]
    fn test_import_aaoffline_batch_multiple_subfolders() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        make_aaoffline_case(&source.path().join("case1"), 70001, "Batch Part 1");
        make_aaoffline_case(&source.path().join("case2"), 70002, "Batch Part 2");
        make_aaoffline_case(&source.path().join("case3"), 70003, "Batch Part 3");

        let result = import_aaoffline_batch(source.path(), engine.path(), None, None).unwrap();
        assert_eq!(result.batch_manifests.len(), 3, "Should import 3 cases");

        let ids: Vec<u32> = result.batch_manifests.iter().map(|m| m.case_id).collect();
        assert!(ids.contains(&70001));
        assert!(ids.contains(&70002));
        assert!(ids.contains(&70003));
    }

    #[test]
    fn test_import_aaoffline_batch_skips_existing() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Import case1 first
        make_aaoffline_case(&source.path().join("case1"), 70010, "Already There");
        import_aaoffline(&source.path().join("case1"), engine.path(), None).unwrap();

        // Now batch import case1 + case2
        make_aaoffline_case(&source.path().join("case2"), 70011, "New Case");
        let result = import_aaoffline_batch(source.path(), engine.path(), None, None).unwrap();

        // case1 silently skipped (already exists), case2 succeeds
        assert_eq!(result.batch_manifests.len(), 1, "Only new case should be in manifests");
        assert_eq!(result.batch_manifests[0].case_id, 70011);
    }

    #[test]
    fn test_import_aaoffline_batch_with_root_and_subfolders() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        // Root case (index.html in parent dir)
        make_aaoffline_case(source.path(), 70020, "Root Case");
        // Subfolder case
        make_aaoffline_case(&source.path().join("sub1"), 70021, "Sub Case");

        let result = import_aaoffline_batch(source.path(), engine.path(), None, None).unwrap();
        assert_eq!(result.batch_manifests.len(), 2, "Root + subfolder should both import");

        let ids: Vec<u32> = result.batch_manifests.iter().map(|m| m.case_id).collect();
        assert!(ids.contains(&70020));
        assert!(ids.contains(&70021));
    }

    #[test]
    fn test_import_aaoffline_batch_empty_folder() {
        let source = tempfile::tempdir().unwrap();
        let engine = tempfile::tempdir().unwrap();

        let result = import_aaoffline_batch(source.path(), engine.path(), None, None);
        assert!(result.is_err(), "Empty folder should return error");
        assert!(result.unwrap_err().contains("No index.html found"));
    }

    #[test]
    fn test_remove_plugin_cleans_config_and_resolved() {
        let engine = tempfile::tempdir().unwrap();
        let case_id = 88001u32;
        let case_dir = engine.path().join("case").join(case_id.to_string());
        let plugins_dir = case_dir.join("plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        // Create plugin file
        fs::write(plugins_dir.join("test_plugin.js"), "// plugin code").unwrap();

        // Create manifest with the plugin
        fs::write(plugins_dir.join("manifest.json"), r#"{"scripts":["test_plugin.js"]}"#).unwrap();

        // Create case_config.json with plugin params
        fs::write(case_dir.join("case_config.json"), r#"{"plugins":{"test_plugin":{"volume":0.5}}}"#).unwrap();

        // Create resolved_plugins.json
        fs::write(case_dir.join("resolved_plugins.json"), r#"{"active":[]}"#).unwrap();

        // Create case manifest
        let manifest = CaseManifest {
            case_id,
            title: "Config Test".into(), author: "A".into(), language: "en".into(),
            download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
            assets: crate::downloader::manifest::AssetSummary {
                case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![], has_plugins: true, has_case_config: true,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        // Remove the plugin
        remove_plugin(case_id, "test_plugin.js", engine.path()).unwrap();

        // Verify plugin file deleted
        assert!(!plugins_dir.join("test_plugin.js").exists());

        // Verify case_config.json no longer has the plugin's params
        let config_text = fs::read_to_string(case_dir.join("case_config.json")).unwrap();
        let config: serde_json::Value = serde_json::from_str(&config_text).unwrap();
        assert!(
            config["plugins"].get("test_plugin").is_none(),
            "Plugin params should be removed from case_config.json"
        );

        // Verify resolved_plugins.json deleted
        assert!(!case_dir.join("resolved_plugins.json").exists());
    }

    // --- extract_plugin_descriptors ---

    #[test]
    fn test_extract_descriptors_basic() {
        let code = r#"
EnginePlugins.register({
    name: "test_plugin",
    params: {
        volume: { type: "number", default: 0.8, min: 0, max: 1, step: 0.1, label: "Volume" },
        enabled: { type: "checkbox", default: true, label: "Enable" }
    },
    init: function(config, events, api) {}
});
"#;
        let result = extract_plugin_descriptors(code);
        assert!(result.is_some(), "Should extract descriptors from basic plugin");
        let desc = result.unwrap();
        assert_eq!(desc["volume"]["type"], "number");
        assert_eq!(desc["volume"]["min"], 0);
        assert_eq!(desc["volume"]["max"], 1);
        assert_eq!(desc["enabled"]["type"], "checkbox");
        assert_eq!(desc["enabled"]["default"], true);
    }

    #[test]
    fn test_extract_descriptors_with_select() {
        let code = r#"
EnginePlugins.register({
    name: "theme_plugin",
    params: {
        theme: { type: "select", default: "dark", options: ["dark", "light", "auto"], label: "Theme" }
    },
    init: function() {}
});
"#;
        let result = extract_plugin_descriptors(code);
        assert!(result.is_some(), "Should extract select descriptors");
        let desc = result.unwrap();
        assert_eq!(desc["theme"]["type"], "select");
        let opts = desc["theme"]["options"].as_array().unwrap();
        assert_eq!(opts.len(), 3);
        assert_eq!(opts[0], "dark");
    }

    #[test]
    fn test_extract_descriptors_no_params() {
        let code = r#"
EnginePlugins.register({
    name: "no_params",
    init: function() {}
});
"#;
        let result = extract_plugin_descriptors(code);
        assert!(result.is_none(), "Plugin without params should return None");
    }

    #[test]
    fn test_extract_descriptors_malformed() {
        let result = extract_plugin_descriptors("this is not valid JS at all");
        assert!(result.is_none(), "Malformed code should return None");

        let result2 = extract_plugin_descriptors("");
        assert!(result2.is_none(), "Empty code should return None");
    }
}
