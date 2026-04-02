use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use crate::downloader::manifest::CaseManifest;
use crate::downloader::paths::normalize_path;
use crate::error::AppError;

/// Scan defaults/ for VFS pointers whose targets are in the given set.
/// Adds the pointer paths to the set so they get exported as real files.
/// This ensures deduped sprites (e.g., charsStill/ pointing to chars/) are included.
fn collect_vfs_pointers_for_export(engine_dir: &Path, defaults: &mut std::collections::HashSet<String>) {
    let defaults_dir = engine_dir.join("defaults");
    if !defaults_dir.is_dir() {
        return;
    }
    let targets: std::collections::HashSet<String> = defaults.clone();
    fn walk(dir: &Path, base: &Path, targets: &std::collections::HashSet<String>, out: &mut Vec<String>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, base, targets, out);
                } else if let Some(target) = crate::downloader::vfs::read_vfs_pointer(&path) {
                    let target_normalized = normalize_path(&target);
                    if targets.contains(&target_normalized) {
                        if let Ok(rel) = path.strip_prefix(base) {
                            out.push(normalize_path(&rel.to_string_lossy()));
                        }
                    }
                }
            }
        }
    }
    let mut pointer_paths = Vec::new();
    walk(&defaults_dir, engine_dir, &targets, &mut pointer_paths);
    for p in pointer_paths {
        defaults.insert(p);
    }
}

pub fn export_aaocase(
    case_id: u32,
    engine_dir: &Path,
    dest_path: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
    saves: Option<&Value>,
    include_plugins: bool,
) -> Result<u64, AppError> {
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id).into());
    }

    let manifest_path = case_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(format!("Case {} has no manifest.json", case_id).into());
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
        let mut defaults: std::collections::HashSet<String> = manifest.asset_map.values()
            .filter(|p| p.starts_with("defaults/"))
            .filter(|p| engine_dir.join(p).is_file())
            .cloned()
            .collect();
        // Also include VFS pointers whose targets are in the set.
        // Dedup may have rewritten manifest entries to the target path,
        // but the engine still needs the pointer path (e.g., charsStill/).
        collect_vfs_pointers_for_export(engine_dir, &mut defaults);
        defaults.into_iter().collect()
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
        let path = crate::downloader::vfs::resolve_path(&path, engine_dir, engine_dir);
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
        let full_path = crate::downloader::vfs::resolve_path(&full_path, engine_dir, engine_dir);
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

    // Add active plugins from the global pool
    let global_plugins_dir = engine_dir.join("plugins");
    if include_plugins && global_plugins_dir.is_dir() {
        let active_scripts = super::saves::get_active_plugin_scripts_for_case(case_id, engine_dir);
        if !active_scripts.is_empty() {
            // Write a plugin manifest listing active scripts
            let manifest = serde_json::json!({ "scripts": active_scripts });
            zip.start_file("plugins/manifest.json", options)
                .map_err(|e| format!("Failed to add plugins/manifest.json: {}", e))?;
            let manifest_json = serde_json::to_string_pretty(&manifest)
                .map_err(|e| format!("Failed to serialize plugins manifest: {}", e))?;
            io::Write::write_all(&mut zip, manifest_json.as_bytes())
                .map_err(|e| format!("Failed to write plugins/manifest.json: {}", e))?;

            // Add each active script
            for script in &active_scripts {
                let src = global_plugins_dir.join(script);
                if src.is_file() {
                    let data = fs::read(&src)
                        .map_err(|e| format!("Failed to read plugin {}: {}", script, e))?;
                    let zip_name = format!("plugins/{}", script);
                    zip.start_file(&zip_name, options)
                        .map_err(|e| format!("Failed to add {}: {}", zip_name, e))?;
                    io::Write::write_all(&mut zip, &data)
                        .map_err(|e| format!("Failed to write {}: {}", zip_name, e))?;
                }
            }

            // Add assets/ directory if it exists
            let assets_dir = global_plugins_dir.join("assets");
            if assets_dir.is_dir() {
                fn add_assets_to_zip(
                    zip: &mut zip::ZipWriter<fs::File>,
                    dir: &Path,
                    prefix: &str,
                    options: zip::write::SimpleFileOptions,
                ) -> Result<(), AppError> {
                    for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {}", prefix, e))? {
                        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
                        let path = entry.path();
                        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
                        if path.is_dir() {
                            add_assets_to_zip(zip, &path, &name, options)?;
                        } else if path.is_file() {
                            let data = fs::read(&path)
                                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
                            zip.start_file(&name, options)
                                .map_err(|e| format!("Failed to add {}: {}", name, e))?;
                            io::Write::write_all(zip, &data)
                                .map_err(|e| format!("Failed to write {}: {}", name, e))?;
                        }
                    }
                    Ok(())
                }
                let _ = add_assets_to_zip(&mut zip, &assets_dir, "plugins/assets", options);
            }
        }
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

    // Export non-default plugin param overrides from global manifest
    if include_plugins {
        let global_manifest_path = engine_dir.join("plugins").join("manifest.json");
        if global_manifest_path.exists() {
            if let Ok(text) = fs::read_to_string(&global_manifest_path) {
                if let Ok(gm) = serde_json::from_str::<serde_json::Value>(&text) {
                    let mut plugin_params = serde_json::Map::new();
                    if let Some(plugins) = gm.get("plugins").and_then(|p| p.as_object()) {
                        for (plugin_name, plugin_cfg) in plugins {
                            if let Some(params) = plugin_cfg.get("params").and_then(|p| p.as_object()) {
                                let mut overrides = serde_json::Map::new();
                                if let Some(by_case) = params.get("by_case").and_then(|bc| bc.as_object()) {
                                    let case_key = case_id.to_string();
                                    if let Some(v) = by_case.get(&case_key) {
                                        let mut o = serde_json::Map::new();
                                        o.insert(case_key, v.clone());
                                        overrides.insert("by_case".to_string(), serde_json::Value::Object(o));
                                    }
                                }
                                // by_sequence: read case manifest for sequence title
                                let seq_title_opt = fs::read_to_string(case_dir.join("manifest.json")).ok()
                                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                                    .and_then(|cm| cm.get("sequence")?.get("title")?.as_str().map(|s| s.to_string()));
                                if let Some(ref seq_title) = seq_title_opt {
                                    if let Some(by_seq) = params.get("by_sequence").and_then(|bs| bs.as_object()) {
                                        if let Some(v) = by_seq.get(seq_title) {
                                            let mut o = serde_json::Map::new();
                                            o.insert(seq_title.clone(), v.clone());
                                            overrides.insert("by_sequence".to_string(), serde_json::Value::Object(o));
                                        }
                                    }
                                }
                                // by_collection: check collection membership
                                let collections_data = crate::collections::load_collections(engine_dir);
                                for col in &collections_data.collections {
                                    let case_in_col = col.items.iter().any(|item| {
                                        match item {
                                            crate::collections::CollectionItem::Case { case_id: cid } => *cid == case_id,
                                            crate::collections::CollectionItem::Sequence { title } => {
                                                seq_title_opt.as_deref() == Some(title.as_str())
                                            }
                                        }
                                    });
                                    if case_in_col {
                                        if let Some(by_col) = params.get("by_collection").and_then(|bc| bc.as_object()) {
                                            if let Some(v) = by_col.get(&col.id) {
                                                let existing = overrides.entry("by_collection".to_string())
                                                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
                                                if let Some(map) = existing.as_object_mut() {
                                                    map.insert(col.id.clone(), v.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                                if !overrides.is_empty() {
                                    plugin_params.insert(plugin_name.clone(), serde_json::Value::Object(overrides));
                                }
                            }
                        }
                    }
                    if !plugin_params.is_empty() {
                        let pp_bytes = serde_json::to_string_pretty(&serde_json::Value::Object(plugin_params))
                            .map_err(|e| format!("Failed to serialize plugin params: {}", e))?;
                        let _ = zip.start_file("plugin_params.json", options);
                        let _ = io::Write::write_all(&mut zip, pp_bytes.as_bytes());
                    }
                }
            }
        }
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize ZIP: {}", e))?;

    // Return file size
    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get ZIP file size: {}", e))?;
    Ok(meta.len())
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
) -> Result<u64, AppError> {
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
            return Err(format!("Case {} not found", case_id).into());
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
                    let path = crate::downloader::vfs::resolve_path(&path, engine_dir, engine_dir);
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
    collect_vfs_pointers_for_export(engine_dir, &mut seen_defaults);
    for default_path in &seen_defaults {
        let full_path = engine_dir.join(default_path);
        let full_path = crate::downloader::vfs::resolve_path(&full_path, engine_dir, engine_dir);
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

    // Export non-default plugin param overrides from global manifest
    if include_plugins {
        let global_manifest_path = engine_dir.join("plugins").join("manifest.json");
        if global_manifest_path.exists() {
            if let Ok(text) = fs::read_to_string(&global_manifest_path) {
                if let Ok(gm) = serde_json::from_str::<serde_json::Value>(&text) {
                    let mut plugin_params = serde_json::Map::new();
                    // Collect sequence titles from collection items
                    let seq_titles: Vec<&str> = collection.items.iter().filter_map(|item| {
                        match item {
                            crate::collections::CollectionItem::Sequence { title } => Some(title.as_str()),
                            _ => None,
                        }
                    }).collect();
                    if let Some(plugins) = gm.get("plugins").and_then(|p| p.as_object()) {
                        for (plugin_name, plugin_cfg) in plugins {
                            if let Some(params) = plugin_cfg.get("params").and_then(|p| p.as_object()) {
                                let mut overrides = serde_json::Map::new();
                                // by_case for each case in the collection
                                if let Some(by_case) = params.get("by_case").and_then(|bc| bc.as_object()) {
                                    let mut case_overrides = serde_json::Map::new();
                                    for &cid in &case_ids {
                                        let key = cid.to_string();
                                        if let Some(v) = by_case.get(&key) {
                                            case_overrides.insert(key, v.clone());
                                        }
                                    }
                                    if !case_overrides.is_empty() {
                                        overrides.insert("by_case".to_string(), serde_json::Value::Object(case_overrides));
                                    }
                                }
                                // by_sequence for each sequence in the collection
                                if let Some(by_seq) = params.get("by_sequence").and_then(|bs| bs.as_object()) {
                                    let mut seq_overrides = serde_json::Map::new();
                                    for &st in &seq_titles {
                                        if let Some(v) = by_seq.get(st) {
                                            seq_overrides.insert(st.to_string(), v.clone());
                                        }
                                    }
                                    if !seq_overrides.is_empty() {
                                        overrides.insert("by_sequence".to_string(), serde_json::Value::Object(seq_overrides));
                                    }
                                }
                                // by_collection for this collection
                                if let Some(by_col) = params.get("by_collection").and_then(|bc| bc.as_object()) {
                                    if let Some(v) = by_col.get(&collection.id) {
                                        let mut o = serde_json::Map::new();
                                        o.insert(collection.id.clone(), v.clone());
                                        overrides.insert("by_collection".to_string(), serde_json::Value::Object(o));
                                    }
                                }
                                if !overrides.is_empty() {
                                    plugin_params.insert(plugin_name.clone(), serde_json::Value::Object(overrides));
                                }
                            }
                        }
                    }
                    if !plugin_params.is_empty() {
                        let pp_bytes = serde_json::to_string_pretty(&serde_json::Value::Object(plugin_params))
                            .map_err(|e| format!("Failed to serialize plugin params: {}", e))?;
                        let _ = zip.start_file("plugin_params.json", options);
                        let _ = io::Write::write_all(&mut zip, pp_bytes.as_bytes());
                    }
                }
            }
        }
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize ZIP: {}", e))?;

    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get ZIP file size: {}", e))?;
    Ok(meta.len())
}

/// Helper: recursively add a directory to a ZIP under a prefix.
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
) -> Result<u64, AppError> {
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
    let seq_str = serde_json::to_string_pretty(&seq_json)
        .map_err(|e| format!("Failed to serialize sequence.json: {}", e))?;
    io::Write::write_all(&mut zip, seq_str.as_bytes())
        .map_err(|e| format!("Failed to write sequence.json: {}", e))?;
    completed += 1;
    if let Some(cb) = &on_progress {
        cb(completed, total);
    }

    // Write each case's files
    for &case_id in case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if !case_dir.exists() {
            return Err(format!("Case {} not found", case_id).into());
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
                    let path = crate::downloader::vfs::resolve_path(&path, engine_dir, engine_dir);
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
    collect_vfs_pointers_for_export(engine_dir, &mut seen_defaults);
    for default_path in &seen_defaults {
        let full_path = engine_dir.join(default_path);
        let full_path = crate::downloader::vfs::resolve_path(&full_path, engine_dir, engine_dir);
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

    // Export non-default plugin param overrides from global manifest
    if include_plugins {
        let global_manifest_path = engine_dir.join("plugins").join("manifest.json");
        if global_manifest_path.exists() {
            if let Ok(text) = fs::read_to_string(&global_manifest_path) {
                if let Ok(gm) = serde_json::from_str::<serde_json::Value>(&text) {
                    let mut plugin_params = serde_json::Map::new();
                    if let Some(plugins) = gm.get("plugins").and_then(|p| p.as_object()) {
                        for (plugin_name, plugin_cfg) in plugins {
                            if let Some(params) = plugin_cfg.get("params").and_then(|p| p.as_object()) {
                                let mut overrides = serde_json::Map::new();
                                // by_case for each case in the sequence
                                if let Some(by_case) = params.get("by_case").and_then(|bc| bc.as_object()) {
                                    let mut case_overrides = serde_json::Map::new();
                                    for &cid in case_ids {
                                        let key = cid.to_string();
                                        if let Some(v) = by_case.get(&key) {
                                            case_overrides.insert(key, v.clone());
                                        }
                                    }
                                    if !case_overrides.is_empty() {
                                        overrides.insert("by_case".to_string(), serde_json::Value::Object(case_overrides));
                                    }
                                }
                                // by_sequence for the sequence title
                                if let Some(by_seq) = params.get("by_sequence").and_then(|bs| bs.as_object()) {
                                    if let Some(v) = by_seq.get(sequence_title) {
                                        let mut o = serde_json::Map::new();
                                        o.insert(sequence_title.to_string(), v.clone());
                                        overrides.insert("by_sequence".to_string(), serde_json::Value::Object(o));
                                    }
                                }
                                if !overrides.is_empty() {
                                    plugin_params.insert(plugin_name.clone(), serde_json::Value::Object(overrides));
                                }
                            }
                        }
                    }
                    if !plugin_params.is_empty() {
                        let pp_bytes = serde_json::to_string_pretty(&serde_json::Value::Object(plugin_params))
                            .map_err(|e| format!("Failed to serialize plugin params: {}", e))?;
                        let _ = zip.start_file("plugin_params.json", options);
                        let _ = io::Write::write_all(&mut zip, pp_bytes.as_bytes());
                    }
                }
            }
        }
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize ZIP: {}", e))?;

    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get ZIP file size: {}", e))?;
    Ok(meta.len())
}
