use std::fs;
use std::io;
use std::path::Path;

use crate::downloader::manifest::{read_manifest, write_manifest};

// Cross-module calls (attach_global_plugin_code, etc.)
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

/// Import a plugin from a .aaoplug ZIP file into one or more existing cases.
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

/// Import a .aaoplug ZIP file as a global plugin.
/// Extracts JS files and attaches each via attach_global_plugin_code.
/// Assets and case_config are skipped (case-specific, not relevant globally).
pub async fn import_aaoplug_global(zip_path: &Path, engine_dir: &Path, client: &reqwest::Client) -> Result<Vec<String>, String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open .aaoplug file: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid .aaoplug file: {}", e))?;

    // Read manifest.json for scripts list
    let manifest_text = read_zip_text(&mut archive, "manifest.json")
        .map_err(|_| "Invalid .aaoplug: missing manifest.json".to_string())?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|e| format!("Failed to parse .aaoplug manifest: {}", e))?;

    let scripts: Vec<String> = manifest.get("scripts")
        .and_then(|s| s.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    if scripts.is_empty() {
        return Err("No scripts listed in .aaoplug manifest".to_string());
    }

    let mut attached = Vec::new();
    for script_name in &scripts {
        // Read the JS file from the ZIP
        let code = match read_zip_text(&mut archive, script_name) {
            Ok(c) => c,
            Err(_) => {
                eprintln!("[IMPORT_GLOBAL] Script {} listed in manifest but not found in ZIP, skipping", script_name);
                continue;
            }
        };

        attach_global_plugin_code(&code, script_name, engine_dir, client).await?;
        attached.push(script_name.clone());
    }

    Ok(attached)
}

/// Attach raw plugin JS code to one or more existing cases.
pub async fn attach_plugin_code(
    code: &str,
    filename: &str,
    target_case_ids: &[u32],
    engine_dir: &Path,
    client: &reqwest::Client,
) -> Result<Vec<u32>, String> {
    let mut attached_cases = Vec::new();

    // Parse @assets once (same for all target cases)
    let assets = parse_plugin_assets(code);

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

        // Download @assets declared in the plugin code
        if !assets.is_empty() {
            let assets_dir = plugins_dir.join("assets");
            download_plugin_assets(client, &assets, &assets_dir).await;
        }

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
    toggle_in_string_array(disabled, filename, !enabled);

    fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap())
        .map_err(|e| format!("Failed to write plugin manifest: {}", e))?;

    Ok(())
}
