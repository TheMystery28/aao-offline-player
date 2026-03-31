use std::fs;
use std::path::Path;

// Cross-module calls (extract_plugin_descriptors, etc.)
use super::*;

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
pub async fn attach_global_plugin_code(code: &str, filename: &str, engine_dir: &Path, client: &reqwest::Client) -> Result<(), String> {
    let plugins_dir = engine_dir.join("plugins");
    fs::create_dir_all(&plugins_dir)
        .map_err(|e| format!("Failed to create global plugins dir: {}", e))?;

    let dest = plugins_dir.join(filename);
    fs::write(&dest, code)
        .map_err(|e| format!("Failed to write global plugin file: {}", e))?;

    // Download @assets declared in the plugin code
    let assets = super::parse_plugin_assets(code);
    if !assets.is_empty() {
        let assets_dir = plugins_dir.join("assets");
        super::download_plugin_assets(client, &assets, &assets_dir).await;
    }

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
    let is_new = !scripts.contains(&filename.to_string());
    if is_new {
        scripts.push(filename.to_string());
        // New plugins start globally disabled (user must explicitly enable)
        if !disabled.contains(&filename.to_string()) {
            disabled.push(filename.to_string());
        }
    }

    // Extract and store param descriptors from plugin source code
    let descriptors = extract_plugin_descriptors(code);

    // Build manifest preserving existing plugins config
    let mut manifest_val = if manifest_file.exists() {
        fs::read_to_string(&manifest_file).ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    manifest_val["scripts"] = serde_json::json!(scripts);
    manifest_val["disabled"] = serde_json::json!(disabled);

    // Store descriptors under plugins.{filename}.descriptors
    if !manifest_val.get("plugins").map(|p| p.is_object()).unwrap_or(false) {
        manifest_val["plugins"] = serde_json::json!({});
    }
    let plugin_entry = manifest_val["plugins"].get(filename).cloned()
        .unwrap_or(serde_json::json!({"scope": {"all": true}, "params": {}}));
    let mut entry = plugin_entry.as_object().cloned().unwrap_or_default();
    entry.insert("descriptors".to_string(), descriptors.unwrap_or(serde_json::Value::Null));
    manifest_val["plugins"].as_object_mut().unwrap()
        .insert(filename.to_string(), serde_json::Value::Object(entry));

    fs::write(&manifest_file, serde_json::to_string_pretty(&manifest_val).unwrap())
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
    with_global_manifest(engine_dir, |val| {
        if val.get("disabled").is_none() {
            val.as_object_mut().unwrap().insert("disabled".to_string(), serde_json::json!([]));
        }
        let disabled = val.get_mut("disabled").unwrap().as_array_mut().unwrap();
        toggle_in_string_array(disabled, filename, !enabled);
        Ok(())
    })
}

/// Toggle a plugin's enabled/disabled state for a specific scope.
///
/// Uses bidirectional override logic:
/// - If globally enabled: `enabled=false` adds to `disabled_for`, `enabled=true` removes from it
/// - If globally disabled: `enabled=true` adds to `enabled_for`, `enabled=false` removes from it
pub fn toggle_plugin_for_scope(
    filename: &str,
    scope_type: &str,
    scope_key: &str,
    enabled: bool,
    engine_dir: &Path,
) -> Result<(), String> {
    // Validate scope_type and build scope value before entering closure
    let field_name = match scope_type {
        "case" => "cases",
        "sequence" => "sequences",
        "collection" => "collections",
        _ => return Err(format!("Invalid scope_type: {}", scope_type)),
    };
    let scope_val: serde_json::Value = if scope_type == "case" {
        match scope_key.parse::<u64>() {
            Ok(id) => serde_json::json!(id),
            Err(_) => return Err(format!("Invalid case ID: {}", scope_key)),
        }
    } else {
        serde_json::json!(scope_key)
    };

    with_global_manifest(engine_dir, |val| {
        let globally_disabled = val.get("disabled")
            .and_then(|d| d.as_array())
            .map(|arr| arr.iter().any(|s| s.as_str() == Some(filename)))
            .unwrap_or(false);

        let plugins = val.get_mut("plugins")
            .and_then(|p| p.as_object_mut())
            .ok_or_else(|| "No plugins config in manifest".to_string())?;
        let entry = plugins.entry(filename.to_string())
            .or_insert(serde_json::json!({"scope": {"all": true}, "params": {}}));

        let override_field = if globally_disabled { "enabled_for" } else { "disabled_for" };
        let should_add = if globally_disabled { enabled } else { !enabled };

        if entry.get(override_field).is_none() {
            entry.as_object_mut().unwrap().insert(
                override_field.to_string(),
                serde_json::json!({"cases": [], "sequences": [], "collections": []}),
            );
        }
        let arr = entry[override_field]
            .get_mut(field_name)
            .and_then(|a| a.as_array_mut())
            .ok_or_else(|| format!("Missing {}.{}", override_field, field_name))?;

        if should_add {
            if !arr.iter().any(|v| *v == scope_val) {
                arr.push(scope_val);
            }
        } else {
            arr.retain(|v| *v != scope_val);
        }
        Ok(())
    })
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

    // If already has plugins, just ensure all scopes are {all: true}
    if val.get("plugins").is_some() {
        let mut changed = false;
        if let Some(plugins) = val.get_mut("plugins").and_then(|p| p.as_object_mut()) {
            for (_name, entry) in plugins.iter_mut() {
                if let Some(scope) = entry.get_mut("scope") {
                    let is_all = scope.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
                    if !is_all {
                        *scope = serde_json::json!({"all": true});
                        changed = true;
                    } else {
                        // Remove old fields if present
                        if let Some(obj) = scope.as_object_mut() {
                            if obj.remove("case_ids").is_some() { changed = true; }
                            if obj.remove("sequence_titles").is_some() { changed = true; }
                            if obj.remove("collection_ids").is_some() { changed = true; }
                        }
                    }
                } else {
                    entry.as_object_mut().unwrap().insert("scope".to_string(), serde_json::json!({"all": true}));
                    changed = true;
                }
            }
        }
        if changed {
            fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap())
                .map_err(|e| format!("Failed to write migrated manifest: {}", e))?;
        }
        return Ok(());
    }

    let scripts = val.get("scripts")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    let mut plugins = serde_json::Map::new();
    for script_val in &scripts {
        if let Some(script_name) = script_val.as_str() {
            plugins.insert(script_name.to_string(), serde_json::json!({
                "scope": { "all": true },
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

    // Read global disabled list
    let global_disabled: Vec<String> = manifest.get("disabled")
        .and_then(|d| d.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let mut active = Vec::new();
    let mut available = Vec::new();

    for script_val in &scripts {
        let script_name = match script_val.as_str() {
            Some(s) => s,
            None => continue,
        };

        let plugin_cfg = plugins_config.get(script_name);
        let globally_disabled = global_disabled.contains(&script_name.to_string());

        // Scope check — simplified: scope.all controls on/off, fine-grained via disabled_for/enabled_for
        let scope = plugin_cfg.and_then(|p| p.get("scope"));
        let scope_matches = match scope {
            Some(s) => s.get("all").and_then(|v| v.as_bool()).unwrap_or(true),
            None => true,
        };

        // Helper: check if a scope override list matches this case
        let check_scope_override = |override_obj: Option<&serde_json::Value>| -> bool {
            let obj = match override_obj {
                Some(o) => o,
                None => return false,
            };
            let case_match = obj.get("cases")
                .and_then(|c| c.as_array())
                .map(|arr| arr.iter().any(|id| id.as_u64() == Some(case_id as u64)))
                .unwrap_or(false);
            let seq_match = case_sequence_title.as_ref().map(|st| {
                obj.get("sequences")
                    .and_then(|s| s.as_array())
                    .map(|arr| arr.iter().any(|t| t.as_str() == Some(st.as_str())))
                    .unwrap_or(false)
            }).unwrap_or(false);
            let col_match = obj.get("collections")
                .and_then(|c| c.as_array())
                .map(|arr| arr.iter().any(|cid| {
                    cid.as_str().map(|s| case_collection_ids.contains(&s.to_string())).unwrap_or(false)
                }))
                .unwrap_or(false);
            case_match || seq_match || col_match
        };

        // Bidirectional override logic
        let is_active = if globally_disabled {
            // Globally disabled → check enabled_for overrides
            check_scope_override(plugin_cfg.and_then(|p| p.get("enabled_for")))
        } else if scope_matches {
            // Globally enabled + scope matches → check disabled_for overrides
            !check_scope_override(plugin_cfg.and_then(|p| p.get("disabled_for")))
        } else {
            false // Scope doesn't match and not globally disabled
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
            let reason = if globally_disabled {
                "disabled (global)"
            } else if !scope_matches {
                "disabled (no matching scope)"
            } else {
                "disabled (scope override)"
            };
            available.push(serde_json::json!({
                "script": script_name,
                "reason": reason
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
/// Merge plugin param overrides from an imported plugin_params.json into the global manifest.
/// Additive: only sets overrides that don't already exist.
pub(super) fn merge_plugin_param_overrides(overrides: &serde_json::Value, engine_dir: &Path) {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    let mut manifest = if manifest_path.exists() {
        fs::read_to_string(&manifest_path).ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        return; // No global manifest → nothing to merge into
    };

    if let Some(override_plugins) = overrides.as_object() {
        for (plugin_name, override_levels) in override_plugins {
            if let Some(levels) = override_levels.as_object() {
                // Ensure the plugin entry exists
                if !manifest.get("plugins").and_then(|p| p.get(plugin_name)).is_some() {
                    continue; // Only merge into existing plugins
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
                                // Additive: only set if not already present
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

    let _ = fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap());
}
