use std::fs;
use std::path::Path;

use crate::downloader::manifest::read_manifest;
use crate::error::AppError;

// Cross-module calls (extract_plugin_descriptors, etc.)
use super::*;

/// List all plugins from {data_dir}/plugins/manifest.json.
pub fn list_global_plugins(engine_dir: &Path) -> Result<serde_json::Value, AppError> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(serde_json::json!({ "scripts": [], "plugins": {} }));
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read global plugin manifest: {}", e))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Other(format!("Failed to parse global plugin manifest: {}", e)))
}

/// Toggle a plugin's enabled/disabled state for a specific scope.
///
/// scope_type: "case", "sequence", "collection", or "global"
/// For "global": sets scope.all = enabled
/// For others: adds/removes from enabled_for, enabled_for_sequences, or enabled_for_collections
pub fn toggle_plugin_for_scope(
    filename: &str,
    scope_type: &str,
    scope_key: &str,
    enabled: bool,
    engine_dir: &Path,
) -> Result<(), AppError> {
    super::shared::with_global_manifest(engine_dir, |val| {
        let plugins = val.get_mut("plugins")
            .and_then(|p| p.as_object_mut())
            .ok_or_else(|| AppError::Other("No plugins in manifest".to_string()))?;

        let entry = plugins.entry(filename.to_string())
            .or_insert(serde_json::json!({
                "scope": { "all": false, "enabled_for": [], "disabled_for": [] },
                "params": {},
                "origin": "global"
            }));

        let scope = entry.get_mut("scope")
            .and_then(|s| s.as_object_mut())
            .ok_or_else(|| AppError::Other("No scope in plugin entry".to_string()))?;

        match scope_type {
            "global" => {
                scope.insert("all".to_string(), serde_json::json!(enabled));
            }
            "case" => {
                let field = if enabled { "enabled_for" } else { "disabled_for" };
                let anti_field = if enabled { "disabled_for" } else { "enabled_for" };
                let case_val: serde_json::Value = match scope_key.parse::<u64>() {
                    Ok(id) => serde_json::json!(id),
                    Err(_) => return Err(format!("Invalid case ID: {}", scope_key).into()),
                };

                // Add to the target field
                let arr = scope.entry(field.to_string())
                    .or_insert(serde_json::json!([]));
                if let Some(a) = arr.as_array_mut() {
                    if !a.contains(&case_val) {
                        a.push(case_val.clone());
                    }
                }

                // Remove from the anti-field
                if let Some(anti) = scope.get_mut(anti_field).and_then(|a| a.as_array_mut()) {
                    anti.retain(|v| *v != case_val);
                }
            }
            "sequence" => {
                let field = "enabled_for_sequences";
                let seq_val = serde_json::json!(scope_key);
                let arr = scope.entry(field.to_string())
                    .or_insert(serde_json::json!([]));
                if let Some(a) = arr.as_array_mut() {
                    if enabled {
                        if !a.contains(&seq_val) { a.push(seq_val); }
                    } else {
                        a.retain(|v| *v != seq_val);
                    }
                }
            }
            "collection" => {
                let field = "enabled_for_collections";
                let col_val = serde_json::json!(scope_key);
                let arr = scope.entry(field.to_string())
                    .or_insert(serde_json::json!([]));
                if let Some(a) = arr.as_array_mut() {
                    if enabled {
                        if !a.contains(&col_val) { a.push(col_val); }
                    } else {
                        a.retain(|v| *v != col_val);
                    }
                }
            }
            _ => return Err(format!("Invalid scope_type: {}", scope_type).into()),
        }

        Ok(())
    })?;

    // Check auto-scope promotion after scope change
    check_auto_promote(filename, engine_dir);
    // Remove redundant case/sequence entries covered by broader scopes
    super::plugins_case::consolidate_scopes(filename, engine_dir);

    Ok(())
}

/// Migrate a global plugin manifest from old format to new unified format.
/// Old: { "scripts": [...], "disabled": [...] }
/// New: { "scripts": [...], "plugins": { "file.js": { "scope": {...}, "params": {...}, "origin": "global" } } }
pub fn migrate_global_manifest(engine_dir: &Path) -> Result<(), AppError> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(());
    }

    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read global manifest: {}", e))?;
    let mut val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse global manifest: {}", e))?;

    // If already has plugins key with proper format, just ensure new fields exist
    if val.get("plugins").is_some() {
        let mut changed = false;
        if let Some(plugins) = val.get_mut("plugins").and_then(|p| p.as_object_mut()) {
            for (_name, entry) in plugins.iter_mut() {
                if entry.get("origin").is_none() {
                    entry.as_object_mut().unwrap().insert("origin".to_string(), serde_json::json!("global"));
                    changed = true;
                }
                if let Some(scope) = entry.get_mut("scope").and_then(|s| s.as_object_mut()) {
                    if scope.get("enabled_for_sequences").is_none() {
                        scope.insert("enabled_for_sequences".to_string(), serde_json::json!([]));
                        changed = true;
                    }
                    if scope.get("enabled_for_collections").is_none() {
                        scope.insert("enabled_for_collections".to_string(), serde_json::json!([]));
                        changed = true;
                    }
                    // Clean old fields
                    if scope.remove("case_ids").is_some() { changed = true; }
                    if scope.remove("sequence_titles").is_some() { changed = true; }
                    if scope.remove("collection_ids").is_some() { changed = true; }
                }
            }
        }
        // Remove old disabled array
        if val.get("disabled").is_some() {
            val.as_object_mut().unwrap().remove("disabled");
            changed = true;
        }
        if changed {
            let json = serde_json::to_string_pretty(&val)
                .map_err(|e| format!("Failed to serialize JSON: {}", e))?;
            fs::write(&manifest_path, json)
                .map_err(|e| format!("Failed to write migrated manifest: {}", e))?;
        }
        return Ok(());
    }

    // Old format: build plugins from scripts + disabled
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
                "scope": { "all": !is_disabled, "enabled_for": [], "disabled_for": [], "enabled_for_sequences": [], "enabled_for_collections": [] },
                "params": {},
                "origin": "global"
            }));
        }
    }

    val.as_object_mut().unwrap().insert("plugins".to_string(), serde_json::Value::Object(plugins));
    val.as_object_mut().unwrap().remove("disabled");

    let json = serde_json::to_string_pretty(&val)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;
    fs::write(&manifest_path, json)
        .map_err(|e| format!("Failed to write migrated manifest: {}", e))?;

    Ok(())
}

/// Resolve which plugins should load for a given case.
/// Reads global manifest, collections, and case manifest to determine scope matches.
/// Merges params cascade: plugin defaults → global → collection → sequence → case.
/// Writes `case/{id}/resolved_plugins.json`.
/// Migrate case-local plugins to the global pool.
/// Scans all `case/*/plugins/` directories, copies JS + assets to `plugins/`,
/// updates global manifest with `origin: "case"` and `enabled_for: [case_id]`,
/// then deletes the case-local `plugins/` directories.
/// Idempotent: does nothing if no case-local plugins exist.
/// Error-tolerant: logs and skips individual failures.
/// Returns the count of migrated plugin scripts.
pub fn migrate_case_plugins_to_global(data_dir: &Path) -> Result<usize, AppError> {
    let cases_dir = data_dir.join("case");
    if !cases_dir.is_dir() {
        return Ok(0);
    }

    // Quick check: are there any case/*/plugins/ dirs?
    let mut has_case_plugins = false;
    if let Ok(entries) = fs::read_dir(&cases_dir) {
        for entry in entries.flatten() {
            if entry.path().join("plugins").join("manifest.json").exists() {
                has_case_plugins = true;
                break;
            }
        }
    }
    if !has_case_plugins {
        return Ok(0);
    }

    let global_plugins_dir = data_dir.join("plugins");
    let _ = fs::create_dir_all(&global_plugins_dir);

    let mut migrated_count = 0;

    let entries: Vec<_> = fs::read_dir(&cases_dir)
        .map_err(|e| format!("Failed to read case dir: {}", e))?
        .flatten()
        .collect();

    for entry in entries {
        let case_dir = entry.path();
        if !case_dir.is_dir() { continue; }

        let case_id: u32 = match case_dir.file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.parse().ok())
        {
            Some(id) => id,
            None => continue,
        };

        let local_plugins_dir = case_dir.join("plugins");
        let local_manifest_path = local_plugins_dir.join("manifest.json");
        if !local_manifest_path.exists() { continue; }

        eprintln!("[MIGRATE] Found case-local plugins in case {}", case_id);

        // Read the case-local manifest
        let scripts: Vec<String> = match fs::read_to_string(&local_manifest_path) {
            Ok(text) => {
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(val) => val.get("scripts")
                        .and_then(|s| s.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default(),
                    Err(e) => {
                        eprintln!("[MIGRATE] Failed to parse manifest for case {}: {}", case_id, e);
                        continue;
                    }
                }
            }
            Err(e) => {
                eprintln!("[MIGRATE] Failed to read manifest for case {}: {}", case_id, e);
                continue;
            }
        };

        // Copy each script to global plugins/
        for script in &scripts {
            let src = local_plugins_dir.join(script);
            let dest = global_plugins_dir.join(script);
            if src.is_file() && !dest.exists() {
                if let Err(e) = fs::copy(&src, &dest) {
                    eprintln!("[MIGRATE] Failed to copy {} for case {}: {}", script, case_id, e);
                    continue;
                }
            }

            // Read code for descriptors
            let code = fs::read_to_string(&dest).unwrap_or_default();
            let descriptors = extract_plugin_descriptors(&code);

            // Update global manifest with scope
            if let Err(e) = upsert_plugin_manifest(data_dir, script, "case", &[case_id], descriptors) {
                eprintln!("[MIGRATE] Failed to update manifest for {}: {}", script, e);
                continue;
            }
            migrated_count += 1;
        }

        // Copy assets/ if present
        let local_assets = local_plugins_dir.join("assets");
        if local_assets.is_dir() {
            let global_assets = global_plugins_dir.join("assets");
            let _ = fs::create_dir_all(&global_assets);
            if let Ok(asset_entries) = fs::read_dir(&local_assets) {
                for ae in asset_entries.flatten() {
                    let src = ae.path();
                    if src.is_file() {
                        let dest = global_assets.join(ae.file_name());
                        if !dest.exists() {
                            let _ = fs::copy(&src, &dest);
                        }
                    }
                }
            }
        }

        // Delete the case-local plugins/ directory
        if let Err(e) = fs::remove_dir_all(&local_plugins_dir) {
            eprintln!("[MIGRATE] Failed to delete case/{}/plugins/: {}", case_id, e);
        } else {
            eprintln!("[MIGRATE] Migrated {} scripts from case {} to global", scripts.len(), case_id);
        }

        // Update case manifest: has_plugins = false (now global)
        if let Ok(mut cm) = read_manifest(&case_dir) {
            cm.has_plugins = false;
            let _ = crate::downloader::manifest::write_manifest(&cm, &case_dir);
        }
    }

    if migrated_count > 0 {
        eprintln!("[MIGRATE] Total: migrated {} plugin scripts to global pool", migrated_count);
    }

    Ok(migrated_count)
}

/// Remove a plugin from the global manifest (scripts array + plugins object).
/// Does NOT delete the JS file or assets — the caller handles that.
pub fn remove_global_plugin_from_manifest(filename: &str, engine_dir: &Path) -> Result<(), AppError> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Ok(()); // Nothing to remove
    }
    shared::with_global_manifest(engine_dir, |val| {
        if let Some(arr) = val.get_mut("scripts").and_then(|s| s.as_array_mut()) {
            arr.retain(|s| s.as_str() != Some(filename));
        }
        if let Some(plugins) = val.get_mut("plugins").and_then(|p| p.as_object_mut()) {
            plugins.remove(filename);
        }
        Ok(())
    })
}

pub fn resolve_plugins_for_case(case_id: u32, data_dir: &Path) -> Result<serde_json::Value, AppError> {
    let global_manifest_path = data_dir.join("plugins").join("manifest.json");

    // Migrate case-local plugins to global on first resolve
    let _ = migrate_case_plugins_to_global(data_dir);

    // Migrate old global manifest format if needed
    migrate_global_manifest(data_dir)?;

    // Read global manifest
    if !global_manifest_path.exists() {
        let resolved = serde_json::json!({ "active": [], "available": [] });
        let case_dir = data_dir.join("case").join(case_id.to_string());
        if case_dir.exists() {
            match serde_json::to_string_pretty(&resolved) {
                Ok(json) => {
                    if let Err(e) = fs::write(case_dir.join("resolved_plugins.json"), json) {
                        eprintln!("[PLUGINS] Failed to write resolved_plugins.json: {}", e);
                    }
                }
                Err(e) => eprintln!("[PLUGINS] Failed to serialize resolved_plugins.json: {}", e),
            }
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

    let mut active = Vec::new();
    let mut available = Vec::new();

    for script_val in &scripts {
        let script_name = match script_val.as_str() {
            Some(s) => s,
            None => continue,
        };

        let is_active = is_plugin_active_for_case(&manifest, script_name, case_id, data_dir);

        if is_active {
            let plugin_cfg = plugins_config.get(script_name);

            // Read case context for param cascade
            let case_dir = data_dir.join("case").join(case_id.to_string());
            let case_sequence_title: Option<String> = read_manifest(&case_dir).ok()
                .and_then(|m| m.sequence.and_then(|s| s.get("title").and_then(|t| t.as_str().map(|s| s.to_string()))));
            let collections_data = crate::collections::load_collections(data_dir);
            let case_collection_ids: Vec<String> = collections_data.collections.iter()
                .filter(|c| c.items.iter().any(|item| match item {
                    crate::collections::CollectionItem::Case { case_id: cid } => *cid == case_id,
                    crate::collections::CollectionItem::Sequence { title } =>
                        case_sequence_title.as_deref() == Some(title.as_str()),
                }))
                .map(|c| c.id.clone())
                .collect();

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

    let case_dir = data_dir.join("case").join(case_id.to_string());
    if case_dir.exists() {
        let json = serde_json::to_string_pretty(&resolved)
            .map_err(|e| format!("Failed to serialize JSON: {}", e))?;
        fs::write(case_dir.join("resolved_plugins.json"), json)
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

    // 2. Collection overrides
    if let Some(by_col) = params.get("by_collection").and_then(|bc| bc.as_object()) {
        for col_id in collection_ids {
            if let Some(overrides) = by_col.get(col_id).and_then(|o| o.as_object()) {
                for (k, v) in overrides {
                    result.insert(k.clone(), v.clone());
                }
                break;
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

/// Merge plugin param overrides from an imported plugin_params.json into the global manifest.
pub(super) fn merge_plugin_param_overrides(overrides: &serde_json::Value, engine_dir: &Path) {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    let mut manifest = if manifest_path.exists() {
        fs::read_to_string(&manifest_path).ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        return;
    };

    if let Some(override_plugins) = overrides.as_object() {
        for (plugin_name, override_levels) in override_plugins {
            if let Some(levels) = override_levels.as_object() {
                if !manifest.get("plugins").and_then(|p| p.get(plugin_name)).is_some() {
                    continue;
                }
                let params = manifest["plugins"][plugin_name]
                    .get("params")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                let mut params = params.as_object().cloned().unwrap_or_default();

                for (level, level_overrides) in levels {
                    if let Some(lo) = level_overrides.as_object() {
                        let existing = params.entry(level.clone())
                            .or_insert(serde_json::json!({}));
                        if let Some(existing_map) = existing.as_object_mut() {
                            for (key, value) in lo {
                                if !existing_map.contains_key(key) {
                                    existing_map.insert(key.clone(), value.clone());
                                }
                            }
                        }
                    }
                }

                manifest["plugins"][plugin_name]["params"] = serde_json::Value::Object(params);
            }
        }
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
