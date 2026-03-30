use std::fs;
use std::path::Path;

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
) -> Result<(), String> {
    migrate_global_manifest(engine_dir)?;
    with_global_manifest(engine_dir, |val| {
        let plugins = val.get_mut("plugins")
            .and_then(|p| p.as_object_mut())
            .ok_or_else(|| "No plugins config".to_string())?;
        let entry = plugins.entry(filename.to_string())
            .or_insert(serde_json::json!({ "scope": { "all": false }, "params": {} }));
        if entry.get("params").and_then(|p| p.as_object()).is_none() {
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
        Ok(())
    })
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
