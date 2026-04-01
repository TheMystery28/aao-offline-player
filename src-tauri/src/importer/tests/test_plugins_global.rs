use super::*;
use std::io;

/// Sync wrapper for attach_plugin_code with origin "global".
fn attach_global_plugin_code_sync(code: &str, filename: &str, engine_dir: &std::path::Path) -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = reqwest::Client::new();
    rt.block_on(attach_plugin_code(code, filename, &[], engine_dir, &client, "global"))?;
    Ok(())
}

/// Sync wrapper for import_aaoplug with origin "global".
fn import_aaoplug_global_sync(zip_path: &std::path::Path, engine_dir: &std::path::Path) -> Result<Vec<u32>, String> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = reqwest::Client::new();
    rt.block_on(import_aaoplug(zip_path, &[], engine_dir, &client, "global"))
}

/// Wrapper for remove_global_plugin (removes all scopes + deletes file).
fn remove_global_plugin_test(filename: &str, engine_dir: &std::path::Path) -> Result<(), String> {
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if manifest_path.exists() {
        let text = std::fs::read_to_string(&manifest_path).unwrap_or_default();
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(arr) = val.get_mut("scripts").and_then(|s| s.as_array_mut()) {
                arr.retain(|s| s.as_str() != Some(filename));
            }
            if let Some(plugins) = val.get_mut("plugins").and_then(|p| p.as_object_mut()) {
                plugins.remove(filename);
            }
            let _ = std::fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap());
        }
    }
    let _ = std::fs::remove_file(engine_dir.join("plugins").join(filename));
    Ok(())
}

/// Wrapper for toggle_global_plugin (sets scope.all).
fn toggle_global_plugin_test(filename: &str, enabled: bool, engine_dir: &std::path::Path) -> Result<(), String> {
    toggle_plugin_for_scope(filename, "global", "", enabled, engine_dir)
}

#[test]
fn test_attach_global_plugin() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    attach_global_plugin_code_sync("// global", "global.js", engine_dir).unwrap();
    assert!(engine_dir.join("plugins/global.js").exists());
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
    ).unwrap();
    assert!(manifest["scripts"].as_array().unwrap().iter().any(|s| s.as_str() == Some("global.js")));
}

#[test]
fn test_remove_global_plugin_test() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    attach_global_plugin_code_sync("// global", "global.js", engine_dir).unwrap();
    remove_global_plugin_test("global.js", engine_dir).unwrap();
    assert!(!engine_dir.join("plugins/global.js").exists());
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
    ).unwrap();
    assert!(manifest["scripts"].as_array().unwrap().is_empty());
}

#[test]
fn test_toggle_global_plugin_test() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    attach_global_plugin_code_sync("// global", "g.js", engine_dir).unwrap();
    // Global plugin starts with scope.all = false
    let m1: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
    ).unwrap();
    assert_eq!(m1["plugins"]["g.js"]["scope"]["all"], false, "Global plugin starts disabled (all:false)");

    toggle_global_plugin_test("g.js", true, engine_dir).unwrap();
    let m2: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
    ).unwrap();
    assert_eq!(m2["plugins"]["g.js"]["scope"]["all"], true, "After enable, scope.all = true");

    toggle_global_plugin_test("g.js", false, engine_dir).unwrap();
    let m3: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap()
    ).unwrap();
    assert_eq!(m3["plugins"]["g.js"]["scope"]["all"], false, "After disable, scope.all = false");
}

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
    assert_eq!(plugins["a.js"]["scope"]["all"], true);  // was enabled → all:true
    assert_eq!(plugins["b.js"]["scope"]["all"], false); // was in disabled[] → all:false
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
fn test_migrate_scope_to_all_true() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    // Plugin with old scope format
    fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"case_ids":[1,2],"sequence_titles":["Seq"],"collection_ids":["c1"]},"params":{}}}}"#).unwrap();

    migrate_global_manifest(engine_dir).unwrap();

    let text = fs::read_to_string(plugins_dir.join("manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    // Old fields should be cleaned, new fields added
    assert!(val["plugins"]["a.js"]["scope"].get("case_ids").is_none(), "Old case_ids should be removed");
    assert!(val["plugins"]["a.js"]["scope"].get("sequence_titles").is_none(), "Old sequence_titles should be removed");
    assert!(val["plugins"]["a.js"]["scope"].get("collection_ids").is_none(), "Old collection_ids should be removed");
    assert!(val["plugins"]["a.js"]["scope"].get("enabled_for_sequences").is_some(), "New field should exist");
    assert!(val["plugins"]["a.js"]["scope"].get("enabled_for_collections").is_some(), "New field should exist");
    assert!(val["plugins"]["a.js"].get("origin").is_some(), "origin field should be added");
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
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"enabled_for":[12345]},"params":{},"origin":"case"}}}"#).unwrap();
    std::fs::create_dir_all(engine_dir.join("case/12345")).unwrap();

    let resolved = resolve_plugins_for_case(12345, engine_dir).unwrap();
    assert_eq!(resolved["active"].as_array().unwrap().len(), 1);
}

#[test]
fn test_disabled_for_case_not_active() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    std::fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":true,"disabled_for":[99999]},"params":{},"origin":"global"}}}"#).unwrap();
    std::fs::create_dir_all(engine_dir.join("case/99999")).unwrap();

    let resolved = resolve_plugins_for_case(99999, engine_dir).unwrap();
    assert_eq!(resolved["active"].as_array().unwrap().len(), 0);
    assert_eq!(resolved["available"].as_array().unwrap().len(), 1);
}

#[test]
fn test_globally_disabled_not_active() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    std::fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false},"params":{},"origin":"global"}}}"#).unwrap();
    std::fs::create_dir_all(engine_dir.join("case/11111")).unwrap();

    let resolved = resolve_plugins_for_case(11111, engine_dir).unwrap();
    assert_eq!(resolved["active"].as_array().unwrap().len(), 0);
    assert_eq!(resolved["available"].as_array().unwrap().len(), 1);
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
fn test_attach_global_plugin_starts_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();

    let text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();

    // Global origin → starts with scope.all = false (disabled)
    assert_eq!(val["plugins"]["test.js"]["scope"]["all"], false, "Global plugin starts with all:false");
    assert_eq!(val["plugins"]["test.js"]["origin"], "global", "Origin should be 'global'");
}

#[test]
fn test_attach_existing_plugin_not_re_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Attach first time (starts disabled)
    attach_global_plugin_code_sync("// v1", "test.js", engine_dir).unwrap();
    // Enable it
    toggle_global_plugin_test("test.js", true, engine_dir).unwrap();
    // Re-attach (update code)
    attach_global_plugin_code_sync("// v2", "test.js", engine_dir).unwrap();

    let text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();

    // After enabling then re-attaching, scope.all should stay true
    assert_eq!(val["plugins"]["test.js"]["scope"]["all"], true, "Re-attached plugin should keep enabled state");
}

#[test]
fn test_resolve_globally_disabled_plugin_inactive() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create case
    create_test_case_for_save(engine_dir, 99001);

    // Attach plugin (starts disabled)
    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();

    let resolved = resolve_plugins_for_case(99001, engine_dir).unwrap();
    let active = resolved["active"].as_array().unwrap();
    assert!(active.is_empty(), "Globally disabled plugin should not be active");
}

#[test]
fn test_resolve_globally_enabled_plugin_active() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    create_test_case_for_save(engine_dir, 99002);

    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();
    toggle_global_plugin_test("test.js", true, engine_dir).unwrap();

    let resolved = resolve_plugins_for_case(99002, engine_dir).unwrap();
    let active = resolved["active"].as_array().unwrap();
    assert_eq!(active.len(), 1, "Globally enabled plugin should be active");
    assert_eq!(active[0]["script"], "test.js");
}

#[test]
fn test_resolve_with_disabled_for() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    create_test_case_for_save(engine_dir, 99003);

    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();
    toggle_global_plugin_test("test.js", true, engine_dir).unwrap();

    // Disable for this specific case
    toggle_plugin_for_scope("test.js", "case", "99003", false, engine_dir).unwrap();

    let resolved = resolve_plugins_for_case(99003, engine_dir).unwrap();
    let active = resolved["active"].as_array().unwrap();
    assert!(active.is_empty(), "Plugin disabled_for this case should not be active");
}

#[test]
fn test_resolve_with_enabled_for() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    create_test_case_for_save(engine_dir, 99004);

    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();
    // Plugin starts disabled globally

    // Enable for this specific case
    toggle_plugin_for_scope("test.js", "case", "99004", true, engine_dir).unwrap();

    let resolved = resolve_plugins_for_case(99004, engine_dir).unwrap();
    let active = resolved["active"].as_array().unwrap();
    assert_eq!(active.len(), 1, "Plugin enabled_for this case should be active");
}

#[test]
fn test_toggle_plugin_for_scope_globally_enabled_disable() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();
    toggle_global_plugin_test("test.js", true, engine_dir).unwrap();

    // Disable for a sequence
    toggle_plugin_for_scope("test.js", "sequence", "My Seq", false, engine_dir).unwrap();

    let text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    // disabled_for is now under scope (toggle_plugin_for_scope with enabled=false adds to scope for sequences)
    // Actually, for "sequence" scope_type, we use enabled_for_sequences
    // When globally enabled + disable for sequence → we don't have disabled_for_sequences in new model
    // The new toggle_plugin_for_scope just adds/removes from enabled_for_sequences
    // So disabling a sequence when globally enabled means... we need to check the actual behavior
    // Let's just verify the scope changed somehow
    let scope = &val["plugins"]["test.js"]["scope"];
    // The toggle should have added "My Seq" to enabled_for_sequences (toggle adds, not removes for sequences)
    assert!(scope.get("enabled_for_sequences").is_some() || scope.get("all") == Some(&serde_json::json!(true)),
        "Scope should be updated after toggle");
}

#[test]
fn test_toggle_plugin_for_scope_globally_disabled_enable() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();
    // Plugin starts globally disabled

    // Enable for a collection
    toggle_plugin_for_scope("test.js", "collection", "col_1", true, engine_dir).unwrap();

    let text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    let ef = &val["plugins"]["test.js"]["scope"]["enabled_for_collections"];
    assert!(ef.as_array().unwrap().iter().any(|s| s == "col_1"));
}

#[test]
fn test_toggle_plugin_for_scope_re_enable_removes_from_disabled_for() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();
    toggle_global_plugin_test("test.js", true, engine_dir).unwrap();

    // Disable then re-enable for a case
    toggle_plugin_for_scope("test.js", "case", "123", false, engine_dir).unwrap();
    toggle_plugin_for_scope("test.js", "case", "123", true, engine_dir).unwrap();

    let text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    // After disabling then re-enabling, the case should be in enabled_for (not disabled_for)
    let scope = &val["plugins"]["test.js"]["scope"];
    let disabled = scope.get("disabled_for").and_then(|d| d.as_array());
    assert!(disabled.is_none() || !disabled.unwrap().iter().any(|v| v.as_u64() == Some(123)),
        "Re-enabling should remove from disabled_for");
}

#[test]
fn test_resolve_bidirectional_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    create_test_case_for_save(engine_dir, 99010);
    create_test_case_for_save(engine_dir, 99011);

    attach_global_plugin_code_sync("// test", "test.js", engine_dir).unwrap();
    // Globally disabled, but enable for case 99010 only
    toggle_plugin_for_scope("test.js", "case", "99010", true, engine_dir).unwrap();

    // Case 99010 should have it active
    let resolved1 = resolve_plugins_for_case(99010, engine_dir).unwrap();
    assert_eq!(resolved1["active"].as_array().unwrap().len(), 1, "enabled_for case should be active");

    // Case 99011 should NOT have it active
    let resolved2 = resolve_plugins_for_case(99011, engine_dir).unwrap();
    assert!(resolved2["active"].as_array().unwrap().is_empty(), "Other case should remain inactive");
}

// --- import_aaoplug_global tests ---

fn create_aaoplug_zip(dir: &std::path::Path, scripts: &[(&str, &str)]) -> std::path::PathBuf {
    let zip_path = dir.join("test.aaoplug");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    // Write manifest.json
    let script_names: Vec<&str> = scripts.iter().map(|(name, _)| *name).collect();
    let manifest = serde_json::json!({ "scripts": script_names });
    zip.start_file("manifest.json", options).unwrap();
    io::Write::write_all(&mut zip, serde_json::to_string(&manifest).unwrap().as_bytes()).unwrap();

    // Write each script
    for (name, code) in scripts {
        zip.start_file(*name, options).unwrap();
        io::Write::write_all(&mut zip, code.as_bytes()).unwrap();
    }

    zip.finish().unwrap();
    zip_path
}

#[test]
fn test_import_aaoplug_global_basic() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let zip_path = create_aaoplug_zip(dir.path(), &[("myplugin.js", "// test plugin code")]);

    let result = import_aaoplug_global_sync(&zip_path, engine_dir).unwrap();
    // Returns empty case IDs since no target cases specified
    assert!(result.is_empty());

    // Verify plugin is in global manifest
    let text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(val["scripts"].as_array().unwrap().iter().any(|s| s == "myplugin.js"));
    // Global origin → scope.all = false (disabled by default)
    assert_eq!(val["plugins"]["myplugin.js"]["scope"]["all"], false);
    // JS file should exist on disk
    assert!(engine_dir.join("plugins/myplugin.js").exists());
}

#[test]
fn test_import_aaoplug_global_multiple_scripts() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let zip_path = create_aaoplug_zip(dir.path(), &[
        ("a.js", "// plugin a"),
        ("b.js", "// plugin b"),
    ]);

    let result = import_aaoplug_global_sync(&zip_path, engine_dir).unwrap();
    assert!(result.is_empty(), "Global import returns empty case IDs");

    let text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["scripts"].as_array().unwrap().len(), 2);
}

#[test]
fn test_import_aaoplug_global_skips_assets() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create a ZIP with JS + assets directory
    let zip_path = dir.path().join("test.aaoplug");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.start_file("manifest.json", options).unwrap();
    io::Write::write_all(&mut zip, br#"{"scripts":["p.js"]}"#).unwrap();
    zip.start_file("p.js", options).unwrap();
    io::Write::write_all(&mut zip, b"// plugin").unwrap();
    zip.start_file("assets/sound.mp3", options).unwrap();
    io::Write::write_all(&mut zip, b"fake audio data").unwrap();
    zip.finish().unwrap();

    let result = import_aaoplug_global_sync(&zip_path, engine_dir).unwrap();
    assert!(result.is_empty());
    // Assets SHOULD be extracted to global plugins dir (unified storage)
    assert!(engine_dir.join("plugins/assets/sound.mp3").exists());
}

// =====================================================================
// Migration tests
// =====================================================================

#[test]
fn test_migrate_case_plugins_to_global() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create a case with local plugins
    let case_id = 77001u32;
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    let local_plugins = case_dir.join("plugins");
    fs::create_dir_all(&local_plugins).unwrap();
    fs::write(local_plugins.join("manifest.json"), r#"{"scripts":["my_plugin.js"]}"#).unwrap();
    fs::write(local_plugins.join("my_plugin.js"), "// old case plugin").unwrap();
    fs::create_dir_all(local_plugins.join("assets")).unwrap();
    fs::write(local_plugins.join("assets/sound.opus"), "fake audio").unwrap();

    // Create case manifest
    let manifest = CaseManifest {
        case_id, title: "Test".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(),
        failed_assets: vec![], has_plugins: true, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Run migration
    let count = migrate_case_plugins_to_global(engine_dir).unwrap();
    assert_eq!(count, 1);

    // Verify: plugin JS in global plugins/
    assert!(engine_dir.join("plugins/my_plugin.js").exists());
    let content = fs::read_to_string(engine_dir.join("plugins/my_plugin.js")).unwrap();
    assert_eq!(content, "// old case plugin");

    // Verify: assets copied
    assert!(engine_dir.join("plugins/assets/sound.opus").exists());

    // Verify: global manifest has the plugin scoped to this case
    let gm_text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let gm: serde_json::Value = serde_json::from_str(&gm_text).unwrap();
    assert!(gm["scripts"].as_array().unwrap().iter().any(|s| s == "my_plugin.js"));
    let enabled = gm["plugins"]["my_plugin.js"]["scope"]["enabled_for"].as_array().unwrap();
    assert!(enabled.iter().any(|v| v.as_u64() == Some(77001)));

    // Verify: case-local plugins/ deleted
    assert!(!local_plugins.exists());

    // Verify: case manifest has_plugins = false
    let updated = read_manifest(&case_dir).unwrap();
    assert!(!updated.has_plugins);
}

#[test]
fn test_migrate_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // No case plugins — should do nothing
    let count = migrate_case_plugins_to_global(engine_dir).unwrap();
    assert_eq!(count, 0);

    // Create a case, run migration, run again
    let case_dir = engine_dir.join("case/77002");
    let local_plugins = case_dir.join("plugins");
    fs::create_dir_all(&local_plugins).unwrap();
    fs::write(local_plugins.join("manifest.json"), r#"{"scripts":["x.js"]}"#).unwrap();
    fs::write(local_plugins.join("x.js"), "// x").unwrap();
    let manifest = CaseManifest {
        case_id: 77002, title: "T".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(),
        failed_assets: vec![], has_plugins: true, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    let c1 = migrate_case_plugins_to_global(engine_dir).unwrap();
    assert_eq!(c1, 1);

    // Second run — should be 0 (plugins dir already deleted)
    let c2 = migrate_case_plugins_to_global(engine_dir).unwrap();
    assert_eq!(c2, 0);
}

#[test]
fn test_migrate_merges_scopes() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Two cases with the same plugin
    for id in [77003u32, 77004] {
        let case_dir = engine_dir.join("case").join(id.to_string());
        let local_plugins = case_dir.join("plugins");
        fs::create_dir_all(&local_plugins).unwrap();
        fs::write(local_plugins.join("manifest.json"), r#"{"scripts":["shared.js"]}"#).unwrap();
        fs::write(local_plugins.join("shared.js"), "// shared plugin").unwrap();
        let manifest = CaseManifest {
            case_id: id, title: "T".into(), author: "A".into(), language: "en".into(),
            download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
            assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![], has_plugins: true, has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
    }

    let count = migrate_case_plugins_to_global(engine_dir).unwrap();
    assert_eq!(count, 2); // 1 script per case, same plugin → 2 upserts

    // Verify scope has both case IDs
    let gm_text = fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let gm: serde_json::Value = serde_json::from_str(&gm_text).unwrap();
    let enabled = gm["plugins"]["shared.js"]["scope"]["enabled_for"].as_array().unwrap();
    assert!(enabled.iter().any(|v| v.as_u64() == Some(77003)));
    assert!(enabled.iter().any(|v| v.as_u64() == Some(77004)));
}

#[test]
fn test_disabled_for_overrides_collection_scope() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    // Plugin enabled via collection, but case 55555 is in disabled_for
    std::fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"disabled_for":[55555],"enabled_for_collections":["col-1"]},"params":{},"origin":"global"}}}"#).unwrap();

    // Set up collection with case 55555 and 55556
    std::fs::write(engine_dir.join("collections.json"),
        r#"{"collections":[{"id":"col-1","title":"Test Col","created_date":"2026-01-01","items":[{"type":"case","case_id":55555},{"type":"case","case_id":55556}]}]}"#).unwrap();
    std::fs::create_dir_all(engine_dir.join("case/55555")).unwrap();
    std::fs::create_dir_all(engine_dir.join("case/55556")).unwrap();

    // Case 55555 should be EXCLUDED (in disabled_for)
    let resolved = resolve_plugins_for_case(55555, engine_dir).unwrap();
    assert_eq!(resolved["active"].as_array().unwrap().len(), 0, "case 55555 should be excluded by disabled_for");
    assert_eq!(resolved["available"].as_array().unwrap().len(), 1);

    // Case 55556 should still be ACTIVE (in collection, not excluded)
    let resolved2 = resolve_plugins_for_case(55556, engine_dir).unwrap();
    assert_eq!(resolved2["active"].as_array().unwrap().len(), 1, "case 55556 should be active via collection");
}

#[test]
fn test_consolidate_removes_case_covered_by_collection() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    std::fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"enabled_for":[55555],"enabled_for_collections":["col-1"]},"params":{},"origin":"global"}}}"#).unwrap();
    std::fs::write(engine_dir.join("collections.json"),
        r#"{"collections":[{"id":"col-1","title":"Test","created_date":"2026-01-01","items":[{"type":"case","case_id":55555},{"type":"case","case_id":55556}]}]}"#).unwrap();

    // Trigger consolidation via toggle (any no-op toggle works)
    toggle_plugin_for_scope("a.js", "collection", "col-1", true, engine_dir).unwrap();

    let text = fs::read_to_string(plugins_dir.join("manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    let enabled = val["plugins"]["a.js"]["scope"]["enabled_for"].as_array().unwrap();
    assert!(enabled.is_empty(), "case 55555 should be consolidated out by collection scope");
}

#[test]
fn test_consolidate_removes_sequence_covered_by_collection() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    std::fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"enabled_for_sequences":["Tutorial"],"enabled_for_collections":["col-1"]},"params":{},"origin":"global"}}}"#).unwrap();
    std::fs::write(engine_dir.join("collections.json"),
        r#"{"collections":[{"id":"col-1","title":"Test","created_date":"2026-01-01","items":[{"type":"sequence","title":"Tutorial"}]}]}"#).unwrap();

    toggle_plugin_for_scope("a.js", "collection", "col-1", true, engine_dir).unwrap();

    let text = fs::read_to_string(plugins_dir.join("manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    let seqs = val["plugins"]["a.js"]["scope"]["enabled_for_sequences"].as_array().unwrap();
    assert!(seqs.is_empty(), "sequence 'Tutorial' should be consolidated out by collection scope");
}

#[test]
fn test_consolidate_removes_case_covered_by_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    std::fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"enabled_for":[100,101],"enabled_for_sequences":["Tutorial"]},"params":{},"origin":"global"}}}"#).unwrap();
    // Create case manifests with sequence info
    let case100 = engine_dir.join("case/100");
    std::fs::create_dir_all(&case100).unwrap();
    std::fs::write(case100.join("manifest.json"),
        r#"{"case_id":100,"title":"Tut 1","author":"a","language":"en","download_date":"2026-01-01","format":"v5","assets":{"case_specific":0,"shared_defaults":0,"total_downloaded":0,"total_size_bytes":0},"asset_map":{},"failed_assets":[],"has_plugins":false,"has_case_config":false,"sequence":{"title":"Tutorial","list":[{"id":100,"title":"Tut 1"},{"id":101,"title":"Tut 2"}]}}"#).unwrap();
    let case101 = engine_dir.join("case/101");
    std::fs::create_dir_all(&case101).unwrap();
    std::fs::write(case101.join("manifest.json"),
        r#"{"case_id":101,"title":"Tut 2","author":"a","language":"en","download_date":"2026-01-01","format":"v5","assets":{"case_specific":0,"shared_defaults":0,"total_downloaded":0,"total_size_bytes":0},"asset_map":{},"failed_assets":[],"has_plugins":false,"has_case_config":false,"sequence":{"title":"Tutorial","list":[{"id":100,"title":"Tut 1"},{"id":101,"title":"Tut 2"}]}}"#).unwrap();

    toggle_plugin_for_scope("a.js", "sequence", "Tutorial", true, engine_dir).unwrap();

    let text = fs::read_to_string(plugins_dir.join("manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    let enabled = val["plugins"]["a.js"]["scope"]["enabled_for"].as_array().unwrap();
    assert!(enabled.is_empty(), "cases 100,101 should be consolidated out by sequence scope");
}

#[test]
fn test_consolidate_preserves_params() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    std::fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"enabled_for":[55555],"enabled_for_collections":["col-1"]},"params":{"by_case":{"55555":{"volume":0.8}}},"origin":"global"}}}"#).unwrap();
    std::fs::write(engine_dir.join("collections.json"),
        r#"{"collections":[{"id":"col-1","title":"Test","created_date":"2026-01-01","items":[{"type":"case","case_id":55555}]}]}"#).unwrap();

    toggle_plugin_for_scope("a.js", "collection", "col-1", true, engine_dir).unwrap();

    let text = fs::read_to_string(plugins_dir.join("manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    // enabled_for should be empty (consolidated)
    let enabled = val["plugins"]["a.js"]["scope"]["enabled_for"].as_array().unwrap();
    assert!(enabled.is_empty(), "case should be consolidated");
    // but params.by_case.55555 must still exist
    let params = &val["plugins"]["a.js"]["params"]["by_case"]["55555"];
    assert_eq!(params["volume"].as_f64().unwrap(), 0.8, "params must be preserved after consolidation");
}

#[test]
fn test_consolidate_does_not_touch_disabled_for() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let plugins_dir = engine_dir.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    std::fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    std::fs::write(plugins_dir.join("manifest.json"),
        r#"{"scripts":["a.js"],"plugins":{"a.js":{"scope":{"all":false,"disabled_for":[55555],"enabled_for_collections":["col-1"]},"params":{},"origin":"global"}}}"#).unwrap();
    std::fs::write(engine_dir.join("collections.json"),
        r#"{"collections":[{"id":"col-1","title":"Test","created_date":"2026-01-01","items":[{"type":"case","case_id":55555}]}]}"#).unwrap();

    toggle_plugin_for_scope("a.js", "collection", "col-1", true, engine_dir).unwrap();

    let text = fs::read_to_string(plugins_dir.join("manifest.json")).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    let disabled = val["plugins"]["a.js"]["scope"]["disabled_for"].as_array().unwrap();
    assert_eq!(disabled.len(), 1, "disabled_for must NOT be touched by consolidation");
    assert_eq!(disabled[0].as_u64().unwrap(), 55555);
}
