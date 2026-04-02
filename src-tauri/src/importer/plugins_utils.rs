use std::fs;
use std::path::Path;

use crate::error::AppError;

// Cross-module calls (remove_plugin, migrate_global_manifest, etc.)
use super::*;
use super::shared::{DuplicateMatch, add_dir_to_zip_recursive};

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
    // 1. Remove single-line comments
    let comment_re = regex::Regex::new(r"//[^\n]*").ok()?;
    let no_line_comments = comment_re.replace_all(raw_js, "");

    // 2. Remove block comments (/* ... */)
    let block_comment_re = regex::Regex::new(r"(?s)/\*.*?\*/").ok()?;
    let no_comments = block_comment_re.replace_all(&no_line_comments, "");

    // 3. Convert single-quoted strings to double-quoted
    let single_re = regex::Regex::new(r"'([^']*)'").ok()?;
    let double_quoted = single_re.replace_all(&no_comments, r#""$1""#);

    // 4. Strip function(...){...} values (replace with null)
    let func_re = regex::Regex::new(r"function\s*\([^)]*\)\s*\{[^}]*\}").ok()?;
    let no_funcs = func_re.replace_all(&double_quoted, "null");

    // 5. Quote unquoted keys (skip already-quoted keys)
    // Rust regex doesn't support lookahead, so we match all keys and check manually
    let key_re = regex::Regex::new(r#"(?m)([{,]\s*)"?(\w+)"?\s*:"#).ok()?;
    let quoted = key_re.replace_all(&no_funcs, r#"$1"$2":"#);

    // 6. Remove trailing commas before } or ]
    let trailing_re = regex::Regex::new(r",\s*([}\]])").ok()?;
    let cleaned = trailing_re.replace_all(&quoted, "$1");

    // Try to parse
    serde_json::from_str(&cleaned).ok()
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

/// Set params for a global plugin at a specific level.
/// level: "default", "by_case", "by_sequence", "by_collection"
/// key: the case_id, sequence_title, or collection_id (ignored for "default")
pub fn set_global_plugin_params(
    filename: &str,
    level: &str,
    key: &str,
    params: &serde_json::Value,
    engine_dir: &Path,
) -> Result<(), AppError> {
    migrate_global_manifest(engine_dir)?;
    with_global_manifest(engine_dir, |val| {
        let plugins = val.get_mut("plugins")
            .and_then(|p| p.as_object_mut())
            .ok_or_else(|| AppError::Other("No plugins config".to_string()))?;
        let entry = plugins.entry(filename.to_string())
            .or_insert(serde_json::json!({ "scope": { "all": false }, "params": {} }));
        if entry.get("params").and_then(|p| p.as_object()).is_none() {
            entry.as_object_mut()
                .ok_or_else(|| AppError::Other("Plugin entry is not an object".to_string()))?
                .insert("params".to_string(), serde_json::json!({}));
        }
        let entry_params = entry.get_mut("params")
            .and_then(|p| p.as_object_mut())
            .ok_or_else(|| AppError::Other("Plugin params is not an object".to_string()))?;

        if level == "default" {
            entry_params.insert("default".to_string(), params.clone());
        } else {
            let level_obj = entry_params.entry(level.to_string())
                .or_insert(serde_json::json!({}));
            level_obj.as_object_mut()
                .ok_or_else(|| AppError::Other("Params level is not an object".to_string()))?
                .insert(key.to_string(), params.clone());
        }
        Ok(())
    })
}

/// Export a case's active plugins as a .aaoplug ZIP file.
/// Reads from the global plugins/ folder, filtered to plugins active for this case.
pub fn export_case_plugins(_case_id: u32, dest_path: &Path, data_dir: &Path) -> Result<u64, AppError> {
    let plugins_dir = data_dir.join("plugins");
    if !plugins_dir.is_dir() {
        return Err("No plugins installed".to_string().into());
    }

    let active = super::saves::get_active_plugin_scripts_for_case(_case_id, data_dir);
    if active.is_empty() {
        return Err("No active plugins for this case".to_string().into());
    }

    let file = fs::File::create(dest_path)
        .map_err(|e| format!("Failed to create .aaoplug file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Write manifest
    let manifest = serde_json::json!({ "scripts": active });
    zip.start_file("manifest.json", options)
        .map_err(|e| format!("Failed to add manifest: {}", e))?;
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;
    std::io::Write::write_all(&mut zip, manifest_json.as_bytes())
        .map_err(|e| format!("Failed to write manifest: {}", e))?;

    // Write each active script
    for script in &active {
        let src = plugins_dir.join(script);
        if src.is_file() {
            let data = fs::read(&src)
                .map_err(|e| format!("Failed to read {}: {}", script, e))?;
            zip.start_file(script.as_str(), options)
                .map_err(|e| format!("Failed to add {}: {}", script, e))?;
            std::io::Write::write_all(&mut zip, &data)
                .map_err(|e| format!("Failed to write {}: {}", script, e))?;
        }
    }

    // Write assets/ if present
    let assets_dir = plugins_dir.join("assets");
    if assets_dir.is_dir() {
        add_dir_to_zip_recursive(&mut zip, &assets_dir, "assets", options)?;
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize .aaoplug ZIP: {}", e))?;

    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get file size: {}", e))?;
    Ok(meta.len())
}

/// Parse `@assets` block from plugin JS code.
/// Looks for `@assets` inside a JSDoc comment (`/** ... */`) and extracts
/// lines matching `filename = url`. Returns Vec<(local_filename, remote_url)>.
pub fn parse_plugin_assets(code: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();

    // Find a JSDoc block (/** ... */) containing @assets
    let mut in_jsdoc = false;
    let mut found_assets = false;
    for line in code.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("/**") {
            in_jsdoc = true;
            // Check if @assets is on the same line as /**
            if trimmed.contains("@assets") {
                found_assets = true;
            }
            if trimmed.ends_with("*/") && trimmed.len() > 3 {
                in_jsdoc = false;
                found_assets = false;
            }
            continue;
        }

        if !in_jsdoc {
            continue;
        }

        if trimmed.contains("*/") {
            in_jsdoc = false;
            found_assets = false;
            continue;
        }

        if trimmed.contains("@assets") {
            found_assets = true;
            continue;
        }

        if !found_assets {
            continue;
        }

        // Strip leading * and whitespace
        let content = trimmed.trim_start_matches('*').trim();
        if content.is_empty() {
            continue;
        }

        // Another @tag ends the @assets section
        if content.starts_with('@') {
            found_assets = false;
            continue;
        }

        // Parse "filename = url"
        if let Some((left, right)) = content.split_once('=') {
            let filename = left.trim().to_string();
            let url = right.trim().to_string();
            if !filename.is_empty() && url.starts_with("http") {
                results.push((filename, url));
            }
        }
    }

    results
}

/// Resolve asset filename collisions when attaching a new plugin.
/// If another installed plugin already owns an asset with the same filename,
/// rename the new plugin's asset and rewrite all references in its code.
/// Returns the (possibly modified) code and assets list.
pub fn resolve_asset_collisions(
    code: &str,
    assets: &[(String, String)],
    plugin_filename: &str,
    plugins_dir: &Path,
) -> (String, Vec<(String, String)>) {
    if assets.is_empty() {
        return (code.to_string(), assets.to_vec());
    }

    let assets_dir = plugins_dir.join("assets");
    let mut new_code = code.to_string();
    let mut new_assets = Vec::new();

    for (asset_name, url) in assets {
        if !assets_dir.join(asset_name).exists() {
            new_assets.push((asset_name.clone(), url.clone()));
            continue;
        }

        // File exists — check if another plugin owns it
        if !is_asset_owned_by_other(asset_name, plugin_filename, plugins_dir) {
            // Same plugin re-attached or orphan file — overwrite is fine
            new_assets.push((asset_name.clone(), url.clone()));
            continue;
        }

        // Collision: generate a unique name and rewrite code
        let renamed = unique_asset_name(asset_name, &assets_dir);
        new_code = new_code.replace(asset_name.as_str(), &renamed);
        new_assets.push((renamed, url.clone()));
    }

    (new_code, new_assets)
}

/// Check if any other installed plugin declares the given asset filename.
fn is_asset_owned_by_other(asset_name: &str, current_plugin: &str, plugins_dir: &Path) -> bool {
    let manifest_path = plugins_dir.join("manifest.json");
    let text = match fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let val: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let scripts = match val["scripts"].as_array() {
        Some(s) => s,
        None => return false,
    };
    for script in scripts {
        let name = match script.as_str() {
            Some(n) => n,
            None => continue,
        };
        if name == current_plugin {
            continue;
        }
        if let Ok(other_code) = fs::read_to_string(plugins_dir.join(name)) {
            if parse_plugin_assets(&other_code).iter().any(|(f, _)| f == asset_name) {
                return true;
            }
        }
    }
    false
}

/// Generate a unique asset filename by appending `_2`, `_3`, etc. before the extension.
fn unique_asset_name(name: &str, assets_dir: &Path) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(pos) => (&name[..pos], &name[pos..]),
        None => (name.as_ref(), ""),
    };
    for i in 2..100 {
        let candidate = format!("{}_{}{}", stem, i, ext);
        if !assets_dir.join(&candidate).exists() {
            return candidate;
        }
    }
    // Fallback — should never happen in practice
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{}_{}{}", stem, ts, ext)
}

/// Delete assets declared in a plugin's `@assets` block.
/// Reads the plugin JS source, parses asset filenames, and removes them from `plugins/assets/`.
pub fn delete_plugin_assets(filename: &str, plugins_dir: &Path) {
    let plugin_file = plugins_dir.join(filename);
    let code = match fs::read_to_string(&plugin_file) {
        Ok(c) => c,
        Err(_) => return, // File already gone or unreadable — nothing to clean
    };
    let assets = parse_plugin_assets(&code);
    if assets.is_empty() {
        return;
    }
    let assets_dir = plugins_dir.join("assets");
    for (asset_filename, _url) in &assets {
        let asset_path = assets_dir.join(asset_filename);
        let _ = fs::remove_file(&asset_path);
    }
}
