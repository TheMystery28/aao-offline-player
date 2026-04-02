use std::fs;
use std::io;
use std::path::Path;

use crate::downloader::manifest::{read_manifest, write_manifest};
use crate::error::AppError;

// Cross-module calls
use super::*;
use super::shared::read_zip_text;

/// Download plugin assets to a target directory.
/// Silent on failure — logs errors but never returns Err.
/// Returns count of successfully downloaded assets.
pub async fn download_plugin_assets(
    client: &reqwest::Client,
    assets: &[(String, String)],
    dest_dir: &Path,
) -> usize {
    if assets.is_empty() {
        return 0;
    }
    let _ = fs::create_dir_all(dest_dir);
    let mut count = 0;
    for (filename, url) in assets {
        let dest = dest_dir.join(filename);
        match client.get(url.as_str()).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.bytes().await {
                        Ok(bytes) => {
                            if fs::write(&dest, &bytes).is_ok() {
                                eprintln!("[PLUGIN_ASSETS] Downloaded: {} → {}", url, dest.display());
                                count += 1;
                            } else {
                                eprintln!("[PLUGIN_ASSETS] Failed to write {}: I/O error", dest.display());
                            }
                        }
                        Err(e) => eprintln!("[PLUGIN_ASSETS] Failed to read response for {}: {}", url, e),
                    }
                } else {
                    eprintln!("[PLUGIN_ASSETS] Failed to download {}: HTTP {}", url, resp.status());
                }
            }
            Err(e) => eprintln!("[PLUGIN_ASSETS] Failed to download {}: {}", url, e),
        }
    }
    count
}

/// Import a plugin from a .aaoplug ZIP file.
/// Extracts to the global `plugins/` folder and sets scope based on origin.
///
/// The .aaoplug format:
/// ```text
/// manifest.json        Plugin metadata + optional external asset URLs
/// *.js                 Plugin code files
/// assets/              Pre-bundled assets (flat folder)
/// case_config.json     Optional config overrides
/// ```
pub async fn import_aaoplug(
    zip_path: &Path,
    target_case_ids: &[u32],
    engine_dir: &Path,
    client: &reqwest::Client,
    origin: &str,
) -> Result<Vec<u32>, AppError> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open .aaoplug file: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid .aaoplug file: {}", e))?;

    // Validate: manifest.json must exist
    let manifest_text = read_zip_text(&mut archive, "manifest.json")
        .map_err(|_| AppError::Other("Invalid .aaoplug: missing manifest.json".to_string()))?;

    // Parse manifest for external assets
    let plugin_manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .unwrap_or(serde_json::Value::Null);

    let plugins_dir = engine_dir.join("plugins");
    fs::create_dir_all(&plugins_dir)
        .map_err(|e| format!("Failed to create plugins directory: {}", e))?;

    // Extract ZIP entries to global plugins/ (skip manifest.json to avoid overwriting global manifest)
    let mut script_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

        let entry_name = entry.name().to_string();

        // Skip the aaoplug manifest — we already read it and don't want to overwrite global manifest
        if entry_name == "manifest.json" { continue; }
        // Skip case_config.json here — handled separately below
        if entry_name == "case_config.json" { continue; }

        if entry.is_dir() {
            let dir_path = plugins_dir.join(&entry_name);
            let _ = fs::create_dir_all(&dir_path);
            continue;
        }

        // Track JS scripts (not assets)
        if entry_name.ends_with(".js") && !entry_name.contains('/') {
            script_names.push(entry_name.clone());
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

            for ext in externals {
                let url = ext.get("url").and_then(|u| u.as_str()).unwrap_or("");
                let path = ext.get("path").and_then(|p| p.as_str()).unwrap_or("");
                if url.is_empty() || path.is_empty() { continue; }

                let dest = plugins_dir.join(path);
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).ok();
                }

                match client.get(url).send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if let Ok(bytes) = resp.bytes().await {
                                if let Err(e) = fs::write(&dest, &bytes) {
                                    eprintln!("[PLUGINS] Failed to write {}: {}", dest.display(), e);
                                }
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

    // Update global manifest for each script
    for script_name in &script_names {
        // Read JS source for descriptors
        let code = fs::read_to_string(plugins_dir.join(script_name)).unwrap_or_default();
        let descriptors = extract_plugin_descriptors(&code);
        upsert_plugin_manifest(engine_dir, script_name, origin, target_case_ids, descriptors)?;
    }

    // For case-targeted imports, handle case_config.json
    if !target_case_ids.is_empty() && plugins_dir.join("case_config.json").exists() {
        // Copy case_config to each target case
        let config_text = fs::read_to_string(plugins_dir.join("case_config.json")).ok();
        for &case_id in target_case_ids {
            let case_dir = engine_dir.join("case").join(case_id.to_string());
            if case_dir.exists() {
                if let Some(ref text) = config_text {
                    if let Err(e) = fs::write(case_dir.join("case_config.json"), text) {
                        eprintln!("[PLUGINS] Failed to write case_config.json: {}", e);
                    }
                    if let Ok(mut manifest) = read_manifest(&case_dir) {
                        manifest.has_case_config = true;
                        if let Err(e) = write_manifest(&manifest, &case_dir) {
                            eprintln!("[PLUGINS] Failed to write manifest: {}", e);
                        }
                    }
                }
            }
        }
        // Remove from global plugins dir (it's case-specific)
        let _ = fs::remove_file(plugins_dir.join("case_config.json"));
    }

    Ok(target_case_ids.to_vec())
}

/// Attach raw plugin JS code with scoped activation.
/// Stores in global `plugins/` folder. Origin determines default scope.
pub async fn attach_plugin_code(
    code: &str,
    filename: &str,
    target_case_ids: &[u32],
    engine_dir: &Path,
    client: &reqwest::Client,
    origin: &str,
) -> Result<Vec<u32>, AppError> {
    let plugins_dir = engine_dir.join("plugins");
    fs::create_dir_all(&plugins_dir)
        .map_err(|e| format!("Failed to create plugins dir: {}", e))?;

    // Resolve asset filename collisions before writing anything
    let raw_assets = parse_plugin_assets(code);
    let (final_code, final_assets) = resolve_asset_collisions(code, &raw_assets, filename, &plugins_dir);

    // Write the (possibly rewritten) JS file to global plugins/
    let dest = plugins_dir.join(filename);
    fs::write(&dest, &final_code)
        .map_err(|e| format!("Failed to write plugin file: {}", e))?;

    // Download @assets declared in the plugin code
    if !final_assets.is_empty() {
        let assets_dir = plugins_dir.join("assets");
        download_plugin_assets(client, &final_assets, &assets_dir).await;
    }

    // Extract descriptors from the final code (in case @assets block was rewritten)
    let descriptors = extract_plugin_descriptors(&final_code);

    // Update global manifest with scope
    upsert_plugin_manifest(engine_dir, filename, origin, target_case_ids, descriptors)?;

    Ok(target_case_ids.to_vec())
}

/// List plugins active for a given case by reading the global manifest.
/// Returns plugins whose scope includes this case_id.
pub fn list_plugins(case_id: u32, engine_dir: &Path) -> Result<serde_json::Value, AppError> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(serde_json::json!({ "scripts": [], "disabled": [] }));
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read plugin manifest: {}", e))?;
    let val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse plugin manifest: {}", e))?;

    // Filter scripts to those whose scope includes this case
    let mut active_scripts: Vec<String> = Vec::new();
    let mut disabled_scripts: Vec<String> = Vec::new();

    if let Some(scripts) = val.get("scripts").and_then(|s| s.as_array()) {
        for s in scripts {
            if let Some(name) = s.as_str() {
                if is_plugin_active_for_case(&val, name, case_id, engine_dir) {
                    active_scripts.push(name.to_string());
                } else {
                    disabled_scripts.push(name.to_string());
                }
            }
        }
    }

    Ok(serde_json::json!({
        "scripts": active_scripts,
        "disabled": disabled_scripts
    }))
}

/// Remove a plugin's scope for a given case. If no scopes remain, delete the plugin entirely.
pub fn remove_plugin(case_id: u32, filename: &str, engine_dir: &Path) -> Result<(), AppError> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(());
    }

    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let mut val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;

    // Remove case_id from the plugin's enabled_for
    if let Some(plugin) = val.get_mut("plugins").and_then(|p| p.get_mut(filename)) {
        if let Some(scope) = plugin.get_mut("scope") {
            if let Some(enabled) = scope.get_mut("enabled_for").and_then(|e| e.as_array_mut()) {
                enabled.retain(|v| v.as_u64() != Some(case_id as u64));
            }
        }
    }

    // Check if any scopes remain
    let should_delete = if let Some(plugin) = val.get("plugins").and_then(|p| p.get(filename)) {
        !has_any_scope(plugin)
    } else {
        true
    };

    if should_delete {
        // Remove from scripts list
        if let Some(arr) = val.get_mut("scripts").and_then(|s| s.as_array_mut()) {
            arr.retain(|s| s.as_str() != Some(filename));
        }
        // Remove plugin config
        if let Some(plugins) = val.get_mut("plugins").and_then(|p| p.as_object_mut()) {
            plugins.remove(filename);
        }
        // Delete the plugin's declared assets, then the JS file itself
        let plugins_dir = engine_dir.join("plugins");
        delete_plugin_assets(filename, &plugins_dir);
        let _ = fs::remove_file(plugins_dir.join(filename));
    }

    let json = serde_json::to_string_pretty(&val)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;
    fs::write(&manifest_path, json)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;

    // Clean plugin params from case_config.json
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    let config_path = case_dir.join("case_config.json");
    if config_path.exists() {
        if let Ok(text) = fs::read_to_string(&config_path) {
            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&text) {
                let plugin_name = filename.trim_end_matches(".js");
                if let Some(plugins) = config.get_mut("plugins").and_then(|p| p.as_object_mut()) {
                    plugins.remove(plugin_name);
                }
                match serde_json::to_string_pretty(&config) {
                    Ok(json) => {
                        if let Err(e) = fs::write(&config_path, json) {
                            eprintln!("[PLUGINS] Failed to write {}: {}", config_path.display(), e);
                        }
                    }
                    Err(e) => eprintln!("[PLUGINS] Failed to serialize case_config.json: {}", e),
                }
            }
        }
    }

    // Delete resolved_plugins.json (regenerated on next play)
    let _ = fs::remove_file(case_dir.join("resolved_plugins.json"));

    // Check auto-scope promotion
    check_auto_promote(filename, engine_dir);

    Ok(())
}

/// Toggle a plugin for a specific case (update enabled_for in global manifest).
pub fn toggle_plugin(case_id: u32, filename: &str, enabled: bool, engine_dir: &Path) -> Result<(), AppError> {
    super::shared::with_global_manifest(engine_dir, |val| {
        let plugins = val.get_mut("plugins")
            .and_then(|p| p.as_object_mut())
            .ok_or_else(|| AppError::Other("No plugins in manifest".to_string()))?;

        let entry = plugins.entry(filename.to_string())
            .or_insert(serde_json::json!({
                "scope": { "all": false, "enabled_for": [], "disabled_for": [] },
                "params": {},
                "origin": "case"
            }));

        let scope = entry.get_mut("scope")
            .and_then(|s| s.as_object_mut())
            .ok_or_else(|| AppError::Other("No scope in plugin entry".to_string()))?;

        // Ensure enabled_for array exists
        if scope.get("enabled_for").is_none() {
            scope.insert("enabled_for".to_string(), serde_json::json!([]));
        }

        let enabled_for = scope.get_mut("enabled_for").unwrap().as_array_mut()
            .ok_or_else(|| AppError::Other("enabled_for is not an array".to_string()))?;

        let case_val = serde_json::json!(case_id);
        if enabled {
            if !enabled_for.contains(&case_val) {
                enabled_for.push(case_val);
            }
        } else {
            enabled_for.retain(|v| *v != case_val);
        }

        Ok(())
    })?;

    // Check auto-scope promotion
    check_auto_promote(filename, engine_dir);

    Ok(())
}

// ============================================================
// Internal helpers
// ============================================================

/// Insert or update a plugin entry in the global manifest.
pub(super) fn upsert_plugin_manifest(
    engine_dir: &Path,
    filename: &str,
    origin: &str,
    target_case_ids: &[u32],
    descriptors: Option<serde_json::Value>,
) -> Result<(), AppError> {
    let plugins_dir = engine_dir.join("plugins");
    fs::create_dir_all(&plugins_dir).ok();
    let manifest_path = plugins_dir.join("manifest.json");

    let mut manifest = if manifest_path.exists() {
        let text = fs::read_to_string(&manifest_path).unwrap_or_default();
        serde_json::from_str::<serde_json::Value>(&text)
            .unwrap_or(serde_json::json!({"scripts": [], "plugins": {}}))
    } else {
        serde_json::json!({"scripts": [], "plugins": {}})
    };

    // Ensure scripts array
    if manifest.get("scripts").is_none() {
        manifest.as_object_mut().unwrap().insert("scripts".to_string(), serde_json::json!([]));
    }
    if manifest.get("plugins").is_none() {
        manifest.as_object_mut().unwrap().insert("plugins".to_string(), serde_json::json!({}));
    }

    // Add to scripts if not present
    let scripts = manifest.get_mut("scripts").unwrap().as_array_mut().unwrap();
    if !scripts.iter().any(|s| s.as_str() == Some(filename)) {
        scripts.push(serde_json::Value::String(filename.to_string()));
    }

    // Build scope based on origin
    let scope = match origin {
        "global" => serde_json::json!({ "all": false }),
        "case" => {
            let case_ids: Vec<serde_json::Value> = target_case_ids.iter()
                .map(|&id| serde_json::json!(id))
                .collect();
            serde_json::json!({ "all": false, "enabled_for": case_ids })
        }
        "sequence" => {
            // Look up sequence title from first case
            let seq_titles = get_sequence_titles_for_cases(target_case_ids, engine_dir);
            serde_json::json!({ "all": false, "enabled_for_sequences": seq_titles })
        }
        "collection" => {
            // Look up collection IDs from target cases
            let col_ids = get_collection_ids_for_cases(target_case_ids, engine_dir);
            serde_json::json!({ "all": false, "enabled_for_collections": col_ids })
        }
        _ => serde_json::json!({ "all": false }),
    };

    // Get or create plugin entry
    let plugins = manifest.get_mut("plugins").unwrap().as_object_mut().unwrap();
    let entry = plugins.entry(filename.to_string())
        .or_insert(serde_json::json!({}));

    // Merge scope: if plugin already exists, add to its scope rather than replacing
    if let Some(existing_scope) = entry.get("scope") {
        let mut merged = existing_scope.clone();
        // Merge enabled_for arrays
        if let Some(new_cases) = scope.get("enabled_for").and_then(|e| e.as_array()) {
            let arr = merged.as_object_mut().unwrap()
                .entry("enabled_for".to_string())
                .or_insert(serde_json::json!([]));
            if let Some(existing) = arr.as_array_mut() {
                for c in new_cases {
                    if !existing.contains(c) {
                        existing.push(c.clone());
                    }
                }
            }
        }
        if let Some(new_seqs) = scope.get("enabled_for_sequences").and_then(|e| e.as_array()) {
            let arr = merged.as_object_mut().unwrap()
                .entry("enabled_for_sequences".to_string())
                .or_insert(serde_json::json!([]));
            if let Some(existing) = arr.as_array_mut() {
                for s in new_seqs {
                    if !existing.contains(s) {
                        existing.push(s.clone());
                    }
                }
            }
        }
        if let Some(new_cols) = scope.get("enabled_for_collections").and_then(|e| e.as_array()) {
            let arr = merged.as_object_mut().unwrap()
                .entry("enabled_for_collections".to_string())
                .or_insert(serde_json::json!([]));
            if let Some(existing) = arr.as_array_mut() {
                for c in new_cols {
                    if !existing.contains(c) {
                        existing.push(c.clone());
                    }
                }
            }
        }
        entry.as_object_mut().unwrap().insert("scope".to_string(), merged);
    } else {
        entry.as_object_mut().unwrap().insert("scope".to_string(), scope);
    }

    // Set origin (only if not already set)
    if entry.get("origin").is_none() {
        entry.as_object_mut().unwrap().insert("origin".to_string(), serde_json::json!(origin));
    }

    // Set descriptors
    entry.as_object_mut().unwrap().insert(
        "descriptors".to_string(),
        descriptors.unwrap_or(serde_json::Value::Null),
    );

    // Ensure params exists
    if entry.get("params").is_none() {
        entry.as_object_mut().unwrap().insert("params".to_string(), serde_json::json!({}));
    }

    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;
    fs::write(&manifest_path, json)
        .map_err(|e| format!("Failed to write plugin manifest: {}", e))?;

    // Consolidate redundant scope entries after merge
    consolidate_scopes(filename, engine_dir);

    Ok(())
}

/// Check if a plugin has any remaining scopes (enabled_for, sequences, collections, or all:true).
fn has_any_scope(plugin: &serde_json::Value) -> bool {
    let scope = match plugin.get("scope") {
        Some(s) => s,
        None => return false,
    };

    if scope.get("all").and_then(|v| v.as_bool()).unwrap_or(false) {
        return true;
    }

    let has_cases = scope.get("enabled_for")
        .and_then(|e| e.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let has_sequences = scope.get("enabled_for_sequences")
        .and_then(|e| e.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let has_collections = scope.get("enabled_for_collections")
        .and_then(|e| e.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    has_cases || has_sequences || has_collections
}

/// Check if a specific plugin is active for a given case.
pub(super) fn is_plugin_active_for_case(manifest: &serde_json::Value, filename: &str, case_id: u32, engine_dir: &Path) -> bool {
    let plugin = match manifest.get("plugins").and_then(|p| p.get(filename)) {
        Some(p) => p,
        None => return false,
    };
    let scope = match plugin.get("scope") {
        Some(s) => s,
        None => return false,
    };

    // Check all: true (with disabled_for exclusion)
    if scope.get("all").and_then(|v| v.as_bool()).unwrap_or(false) {
        let disabled_for = scope.get("disabled_for").and_then(|d| d.as_array());
        if let Some(disabled) = disabled_for {
            return !disabled.iter().any(|v| v.as_u64() == Some(case_id as u64));
        }
        return true;
    }

    // Check enabled_for
    let mut active = false;
    if let Some(enabled) = scope.get("enabled_for").and_then(|e| e.as_array()) {
        if enabled.iter().any(|v| v.as_u64() == Some(case_id as u64)) {
            active = true;
        }
    }

    // Check enabled_for_sequences
    if !active {
        if let Some(seqs) = scope.get("enabled_for_sequences").and_then(|e| e.as_array()) {
            if !seqs.is_empty() {
                let case_dir = engine_dir.join("case").join(case_id.to_string());
                if let Ok(case_manifest) = read_manifest(&case_dir) {
                    if let Some(seq) = &case_manifest.sequence {
                        if let Some(title) = seq.get("title").and_then(|t| t.as_str()) {
                            if seqs.iter().any(|s| s.as_str() == Some(title)) {
                                active = true;
                            }
                        }
                    }
                }
            }
        }
    }

    // Check enabled_for_collections
    if !active {
        if let Some(cols) = scope.get("enabled_for_collections").and_then(|e| e.as_array()) {
            if !cols.is_empty() {
                let collections = crate::collections::load_collections(engine_dir);
                for col in &collections.collections {
                    if cols.iter().any(|c| c.as_str() == Some(&col.id)) {
                        let case_dir = engine_dir.join("case").join(case_id.to_string());
                        let case_seq_title = read_manifest(&case_dir).ok()
                            .and_then(|m| m.sequence.and_then(|s| s.get("title").and_then(|t| t.as_str().map(|s| s.to_string()))));

                        for item in &col.items {
                            match item {
                                crate::collections::CollectionItem::Case { case_id: cid } if *cid == case_id => { active = true; break; }
                                crate::collections::CollectionItem::Sequence { title } => {
                                    if case_seq_title.as_deref() == Some(title.as_str()) {
                                        active = true;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    if active { break; }
                }
            }
        }
    }

    // Final exclusion: disabled_for overrides any activation
    if active {
        if let Some(disabled) = scope.get("disabled_for").and_then(|d| d.as_array()) {
            if disabled.iter().any(|v| v.as_u64() == Some(case_id as u64)) {
                return false;
            }
        }
    }

    active
}

/// Get sequence titles for a list of case IDs.
fn get_sequence_titles_for_cases(case_ids: &[u32], engine_dir: &Path) -> Vec<String> {
    let mut titles = Vec::new();
    for &id in case_ids {
        let case_dir = engine_dir.join("case").join(id.to_string());
        if let Ok(manifest) = read_manifest(&case_dir) {
            if let Some(seq) = &manifest.sequence {
                if let Some(title) = seq.get("title").and_then(|t| t.as_str()) {
                    if !titles.contains(&title.to_string()) {
                        titles.push(title.to_string());
                    }
                }
            }
        }
    }
    titles
}

/// Get collection IDs that contain any of the given case IDs.
fn get_collection_ids_for_cases(case_ids: &[u32], engine_dir: &Path) -> Vec<String> {
    let collections = crate::collections::load_collections(engine_dir);
    let mut ids = Vec::new();
    for col in &collections.collections {
        for item in &col.items {
            let matches = match item {
                crate::collections::CollectionItem::Case { case_id } => case_ids.contains(case_id),
                crate::collections::CollectionItem::Sequence { title } => {
                    // Check if any target case belongs to this sequence
                    case_ids.iter().any(|&cid| {
                        let case_dir = engine_dir.join("case").join(cid.to_string());
                        read_manifest(&case_dir).ok()
                            .and_then(|m| m.sequence.and_then(|s| s.get("title").and_then(|t| t.as_str().map(|s| s.to_string()))))
                            .as_deref() == Some(title.as_str())
                    })
                }
            };
            if matches && !ids.contains(&col.id) {
                ids.push(col.id.clone());
                break;
            }
        }
    }
    ids
}

/// Auto-scope promotion: if all cases in a sequence are individually enabled,
/// promote to sequence scope. Same for collections.
pub(super) fn check_auto_promote(filename: &str, engine_dir: &Path) {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() { return; }

    let text = match fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(_) => return,
    };
    let mut manifest: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return,
    };

    let plugin = match manifest.get_mut("plugins").and_then(|p| p.get_mut(filename)) {
        Some(p) => p,
        None => return,
    };

    let enabled_for: Vec<u32> = plugin.get("scope")
        .and_then(|s| s.get("enabled_for"))
        .and_then(|e| e.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|id| id as u32)).collect())
        .unwrap_or_default();

    if enabled_for.len() < 2 { return; } // Need at least 2 cases to promote

    // Check sequences: group enabled cases by sequence
    let mut seq_cases: std::collections::HashMap<String, Vec<u32>> = std::collections::HashMap::new();
    for &cid in &enabled_for {
        let case_dir = engine_dir.join("case").join(cid.to_string());
        if let Ok(m) = read_manifest(&case_dir) {
            if let Some(seq) = &m.sequence {
                if let Some(title) = seq.get("title").and_then(|t| t.as_str()) {
                    if let Some(list) = seq.get("list").and_then(|l| l.as_array()) {
                        let all_ids: Vec<u32> = list.iter()
                            .filter_map(|v| v.get("id").and_then(|id| id.as_u64()).map(|id| id as u32))
                            .collect();
                        seq_cases.entry(title.to_string()).or_default().push(cid);
                        // Store full sequence for comparison
                        if !seq_cases.contains_key(&format!("__full_{}", title)) {
                            seq_cases.insert(format!("__full_{}", title), all_ids);
                        }
                    }
                }
            }
        }
    }

    let mut promoted_cases: Vec<u32> = Vec::new();
    let mut promoted_seqs: Vec<String> = Vec::new();

    for (title, _cases) in &seq_cases {
        if title.starts_with("__full_") { continue; }
        let full_key = format!("__full_{}", title);
        if let Some(full_seq) = seq_cases.get(&full_key) {
            // Check if all sequence cases are in enabled_for
            if full_seq.iter().all(|id| enabled_for.contains(id)) {
                promoted_seqs.push(title.clone());
                promoted_cases.extend(full_seq.iter());
            }
        }
    }

    if !promoted_seqs.is_empty() {
        let scope = plugin.get_mut("scope").unwrap();
        // Add to enabled_for_sequences
        let seqs = scope.as_object_mut().unwrap()
            .entry("enabled_for_sequences".to_string())
            .or_insert(serde_json::json!([]));
        if let Some(arr) = seqs.as_array_mut() {
            for s in &promoted_seqs {
                let val = serde_json::json!(s);
                if !arr.contains(&val) {
                    arr.push(val);
                }
            }
        }
        // Remove promoted cases from enabled_for
        if let Some(enabled) = scope.get_mut("enabled_for").and_then(|e| e.as_array_mut()) {
            enabled.retain(|v| {
                v.as_u64().map(|id| !promoted_cases.contains(&(id as u32))).unwrap_or(true)
            });
        }

        match serde_json::to_string_pretty(&manifest) {
            Ok(json) => {
                if let Err(e) = fs::write(&manifest_path, json) {
                    eprintln!("[PLUGINS] Failed to write manifest: {}", e);
                }
            }
            Err(e) => eprintln!("[PLUGINS] Failed to serialize manifest: {}", e),
        }
    }

    // Collection promotion would follow the same pattern but is less common — skip for now
}

/// Remove redundant scope entries covered by broader scopes.
/// Cases in enabled collections/sequences are removed from enabled_for.
/// Sequences in enabled collections are removed from enabled_for_sequences.
pub(super) fn consolidate_scopes(filename: &str, engine_dir: &Path) {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() { return; }

    let text = match fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(_) => return,
    };
    let mut manifest: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return,
    };

    let plugin = match manifest.get_mut("plugins").and_then(|p| p.get_mut(filename)) {
        Some(p) => p,
        None => return,
    };

    let scope = match plugin.get("scope") {
        Some(s) => s.clone(),
        None => return,
    };

    let enabled_for: Vec<u64> = scope.get("enabled_for")
        .and_then(|e| e.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect())
        .unwrap_or_default();
    let enabled_for_seqs: Vec<String> = scope.get("enabled_for_sequences")
        .and_then(|e| e.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let enabled_for_cols: Vec<String> = scope.get("enabled_for_collections")
        .and_then(|e| e.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    if enabled_for_cols.is_empty() && enabled_for_seqs.is_empty() {
        return; // Nothing to consolidate against
    }

    let mut cases_to_remove: Vec<u64> = Vec::new();
    let mut seqs_to_remove: Vec<String> = Vec::new();

    // 1. Collection consolidation: remove cases and sequences covered by collections
    if !enabled_for_cols.is_empty() {
        let collections = crate::collections::load_collections(engine_dir);
        for col in &collections.collections {
            if !enabled_for_cols.iter().any(|c| c == &col.id) { continue; }
            for item in &col.items {
                match item {
                    crate::collections::CollectionItem::Case { case_id } => {
                        cases_to_remove.push(*case_id as u64);
                    }
                    crate::collections::CollectionItem::Sequence { title } => {
                        seqs_to_remove.push(title.clone());
                        // Also find all case IDs in this sequence
                        for &cid in &enabled_for {
                            let case_dir = engine_dir.join("case").join(cid.to_string());
                            if let Ok(m) = read_manifest(&case_dir) {
                                if let Some(seq) = &m.sequence {
                                    if seq.get("title").and_then(|t| t.as_str()) == Some(title) {
                                        if let Some(list) = seq.get("list").and_then(|l| l.as_array()) {
                                            for v in list {
                                                if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                                                    cases_to_remove.push(id);
                                                }
                                            }
                                        }
                                        break; // Found the sequence info from one case, done
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Sequence consolidation: remove cases covered by sequences
    if !enabled_for_seqs.is_empty() {
        for &cid in &enabled_for {
            let case_dir = engine_dir.join("case").join(cid.to_string());
            if let Ok(m) = read_manifest(&case_dir) {
                if let Some(seq) = &m.sequence {
                    if let Some(title) = seq.get("title").and_then(|t| t.as_str()) {
                        if enabled_for_seqs.iter().any(|s| s == title) {
                            // This case's sequence is enabled — remove the case
                            if let Some(list) = seq.get("list").and_then(|l| l.as_array()) {
                                for v in list {
                                    if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                                        cases_to_remove.push(id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Apply removals
    let mut changed = false;
    let scope_mut = plugin.get_mut("scope").unwrap();

    if !cases_to_remove.is_empty() {
        if let Some(arr) = scope_mut.get_mut("enabled_for").and_then(|e| e.as_array_mut()) {
            let before = arr.len();
            arr.retain(|v| v.as_u64().map(|id| !cases_to_remove.contains(&id)).unwrap_or(true));
            if arr.len() != before { changed = true; }
        }
    }

    if !seqs_to_remove.is_empty() {
        if let Some(arr) = scope_mut.get_mut("enabled_for_sequences").and_then(|e| e.as_array_mut()) {
            let before = arr.len();
            arr.retain(|v| v.as_str().map(|s| !seqs_to_remove.iter().any(|r| r == s)).unwrap_or(true));
            if arr.len() != before { changed = true; }
        }
    }

    if changed {
        match serde_json::to_string_pretty(&manifest) {
            Ok(json) => {
                if let Err(e) = fs::write(&manifest_path, json) {
                    eprintln!("[PLUGINS] Failed to write manifest: {}", e);
                }
            }
            Err(e) => eprintln!("[PLUGINS] Failed to serialize manifest: {}", e),
        }
    }
}
