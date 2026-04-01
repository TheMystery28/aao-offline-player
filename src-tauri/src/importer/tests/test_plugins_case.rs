use super::*;

fn test_client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Sync wrapper for attach_plugin_code (stores globally now).
fn attach_plugin_code_sync(code: &str, filename: &str, case_ids: &[u32], engine_dir: &std::path::Path) -> Result<Vec<u32>, String> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(attach_plugin_code(code, filename, case_ids, engine_dir, &test_client(), "case"))
}

#[tokio::test]
async fn test_import_aaoplug_extracts_to_global() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create a fake case directory with minimal manifest
    let case_dir = engine_dir.join("case/99999");
    std::fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id: 99999,
        title: "Test".to_string(), author: "Test".to_string(), language: "en".to_string(),
        download_date: "2026-01-01".to_string(), format: "test".to_string(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(), failed_assets: vec![],
        has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Create a .aaoplug ZIP
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

    // Import the plugin (goes to global plugins/)
    let result = import_aaoplug(&plug_path, &[99999], engine_dir, &test_client(), "case").await;
    assert!(result.is_ok(), "import_aaoplug should succeed");

    // Verify files in global plugins/ (not case/{id}/plugins/)
    assert!(engine_dir.join("plugins/test_plugin.js").exists());
    assert!(engine_dir.join("plugins/assets/test_sound.opus").exists());

    // Verify global manifest has the plugin scoped to case 99999
    let gm_text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let gm: serde_json::Value = serde_json::from_str(&gm_text).unwrap();
    let scripts = gm["scripts"].as_array().unwrap();
    assert!(scripts.iter().any(|s| s.as_str() == Some("test_plugin.js")));
    let scope = &gm["plugins"]["test_plugin.js"]["scope"];
    let enabled_for = scope["enabled_for"].as_array().unwrap();
    assert!(enabled_for.iter().any(|v| v.as_u64() == Some(99999)));
}

#[tokio::test]
async fn test_import_aaoplug_invalid_zip() {
    let dir = tempfile::tempdir().unwrap();
    let bad_path = dir.path().join("bad.aaoplug");
    std::fs::write(&bad_path, "not a zip").unwrap();
    let result = import_aaoplug(&bad_path, &[1], dir.path(), &test_client(), "case").await;
    assert!(result.is_err());
}

#[test]
fn test_attach_plugin_code_stores_globally() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create a fake case
    let case_dir = engine_dir.join("case/88888");
    std::fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id: 88888,
        title: "Test".to_string(), author: "Test".to_string(), language: "en".to_string(),
        download_date: "2026-01-01".to_string(), format: "test".to_string(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(), failed_assets: vec![],
        has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    let result = attach_plugin_code_sync("console.log('hello');", "my_plugin.js", &[88888], engine_dir);
    assert!(result.is_ok());

    // Verify file in global plugins/ (not case/88888/plugins/)
    assert!(engine_dir.join("plugins/my_plugin.js").exists());
    assert!(!case_dir.join("plugins/my_plugin.js").exists());

    // Verify global manifest
    let gm_text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let gm: serde_json::Value = serde_json::from_str(&gm_text).unwrap();
    let scripts = gm["scripts"].as_array().unwrap();
    assert!(scripts.iter().any(|s| s.as_str() == Some("my_plugin.js")));
}

#[test]
fn test_list_plugins_empty_case() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
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
        title: "Test".to_string(), author: "Test".to_string(), language: "en".to_string(),
        download_date: "2026-01-01".to_string(), format: "test".to_string(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(), failed_assets: vec![],
        has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    attach_plugin_code_sync("// a", "a.js", &[90002], engine_dir).unwrap();
    attach_plugin_code_sync("// b", "b.js", &[90002], engine_dir).unwrap();

    let result = list_plugins(90002, engine_dir).unwrap();
    let scripts = result.get("scripts").unwrap().as_array().unwrap();
    assert_eq!(scripts.len(), 2);
    let names: Vec<&str> = scripts.iter().map(|s| s.as_str().unwrap()).collect();
    assert!(names.contains(&"a.js"));
    assert!(names.contains(&"b.js"));
}

#[test]
fn test_remove_plugin_ref_counted() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create two cases
    for id in [90003, 90004] {
        let case_dir = engine_dir.join("case").join(id.to_string());
        std::fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id: id,
            title: "Test".to_string(), author: "Test".to_string(), language: "en".to_string(),
            download_date: "2026-01-01".to_string(), format: "test".to_string(), sequence: None,
            assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
            asset_map: std::collections::HashMap::new(), failed_assets: vec![],
            has_plugins: false, has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
    }

    // Attach same plugin to both cases
    attach_plugin_code_sync("// shared", "shared.js", &[90003], engine_dir).unwrap();
    attach_plugin_code_sync("// shared", "shared.js", &[90004], engine_dir).unwrap();

    // Remove from first case — file should still exist
    remove_plugin(90003, "shared.js", engine_dir).unwrap();
    assert!(engine_dir.join("plugins/shared.js").exists(), "File should survive — case 90004 still uses it");

    // Remove from second case — file should be deleted
    remove_plugin(90004, "shared.js", engine_dir).unwrap();
    assert!(!engine_dir.join("plugins/shared.js").exists(), "File should be deleted — no scopes remain");
}

#[test]
fn test_remove_plugin_deletes_assets() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    let case_dir = engine_dir.join("case/90010");
    std::fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id: 90010,
        title: "Test".into(), author: "T".into(), language: "en".into(),
        download_date: "2026-01-01".into(), format: "test".into(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(), failed_assets: vec![],
        has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Plugin with @assets block
    let code = "/**\n * @assets\n * blip1.opus = https://example.com/blip1.opus\n * blip2.opus = https://example.com/blip2.opus\n */\nEnginePlugins.register({ name: 'blips', version: '1.0', init: function() {} });";
    attach_plugin_code_sync(code, "blips.js", &[90010], engine_dir).unwrap();

    // Manually create asset files (download_plugin_assets is async/network)
    let assets_dir = engine_dir.join("plugins/assets");
    std::fs::create_dir_all(&assets_dir).unwrap();
    std::fs::write(assets_dir.join("blip1.opus"), b"fake").unwrap();
    std::fs::write(assets_dir.join("blip2.opus"), b"fake").unwrap();
    // Also create an unrelated asset that should NOT be deleted
    std::fs::write(assets_dir.join("other.mp3"), b"keep").unwrap();

    assert!(assets_dir.join("blip1.opus").exists());
    assert!(assets_dir.join("blip2.opus").exists());

    // Remove plugin — should delete assets
    remove_plugin(90010, "blips.js", engine_dir).unwrap();

    assert!(!engine_dir.join("plugins/blips.js").exists(), "JS file should be deleted");
    assert!(!assets_dir.join("blip1.opus").exists(), "blip1.opus should be deleted");
    assert!(!assets_dir.join("blip2.opus").exists(), "blip2.opus should be deleted");
    assert!(assets_dir.join("other.mp3").exists(), "Unrelated asset should survive");
}

#[test]
fn test_asset_collision_renames_and_rewrites() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Plugin A declares sound.opus
    let code_a = "/**\n * @assets\n * sound.opus = https://example.com/a/sound.opus\n */\nEnginePlugins.register({ name: 'plugin_a', version: '1.0', init: function(config, events, api) {\n\tvar url = 'case/1/plugins/assets/sound.opus';\n} });";
    attach_plugin_code_sync(code_a, "plugin_a.js", &[], engine_dir).unwrap();

    // Manually create the asset (download is network-dependent)
    let assets_dir = engine_dir.join("plugins/assets");
    std::fs::create_dir_all(&assets_dir).unwrap();
    std::fs::write(assets_dir.join("sound.opus"), b"content_a").unwrap();

    // Plugin B also declares sound.opus — different URL
    let code_b = "/**\n * @assets\n * sound.opus = https://example.com/b/sound.opus\n */\nEnginePlugins.register({ name: 'plugin_b', version: '1.0', init: function(config, events, api) {\n\tvar url = 'case/1/plugins/assets/sound.opus';\n} });";
    attach_plugin_code_sync(code_b, "plugin_b.js", &[], engine_dir).unwrap();

    // Read the stored plugin_b.js — should have renamed references
    let stored_b = std::fs::read_to_string(engine_dir.join("plugins/plugin_b.js")).unwrap();
    assert!(!stored_b.contains("* sound.opus = "), "Original @assets name should be rewritten");
    assert!(stored_b.contains("sound_2.opus"), "Should be renamed to sound_2.opus");
    assert!(stored_b.contains("plugins/assets/sound_2.opus"), "JS code reference should be rewritten");

    // Plugin A's original asset should be untouched
    assert_eq!(std::fs::read_to_string(assets_dir.join("sound.opus")).unwrap(), "content_a");

    // Plugin A's code should be untouched
    let stored_a = std::fs::read_to_string(engine_dir.join("plugins/plugin_a.js")).unwrap();
    assert!(stored_a.contains("sound.opus"), "Plugin A code unchanged");

    // Cleanup: removing plugin_b should delete sound_2.opus but not sound.opus
    let case_dir = engine_dir.join("case/90020");
    std::fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id: 90020,
        title: "T".into(), author: "T".into(), language: "en".into(),
        download_date: "2026-01-01".into(), format: "t".into(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(), failed_assets: vec![],
        has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Manually create sound_2.opus so delete_plugin_assets can clean it
    std::fs::write(assets_dir.join("sound_2.opus"), b"content_b").unwrap();

    // Delete plugin B via remove_global: delete assets then file
    delete_plugin_assets("plugin_b.js", &engine_dir.join("plugins"));
    std::fs::remove_file(engine_dir.join("plugins/plugin_b.js")).unwrap();

    assert!(!assets_dir.join("sound_2.opus").exists(), "Renamed asset should be deleted with plugin B");
    assert!(assets_dir.join("sound.opus").exists(), "Plugin A's asset should survive");
}

#[test]
fn test_asset_no_collision_no_rename() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Plugin A has sound_a.opus
    let code_a = "/**\n * @assets\n * sound_a.opus = https://example.com/a.opus\n */\nEnginePlugins.register({ name: 'a', version: '1.0', init: function() {} });";
    attach_plugin_code_sync(code_a, "a.js", &[], engine_dir).unwrap();
    std::fs::create_dir_all(engine_dir.join("plugins/assets")).unwrap();
    std::fs::write(engine_dir.join("plugins/assets/sound_a.opus"), b"a").unwrap();

    // Plugin B has sound_b.opus — no overlap
    let code_b = "/**\n * @assets\n * sound_b.opus = https://example.com/b.opus\n */\nEnginePlugins.register({ name: 'b', version: '1.0', init: function() { var f = 'sound_b.opus'; } });";
    attach_plugin_code_sync(code_b, "b.js", &[], engine_dir).unwrap();

    let stored_b = std::fs::read_to_string(engine_dir.join("plugins/b.js")).unwrap();
    assert!(stored_b.contains("sound_b.opus"), "No collision — name unchanged");
    assert!(!stored_b.contains("sound_b_2.opus"), "Should NOT be renamed");
}

#[test]
fn test_asset_reattach_same_plugin_no_rename() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    let code = "/**\n * @assets\n * effect.wav = https://example.com/effect.wav\n */\nEnginePlugins.register({ name: 'fx', version: '1.0', init: function() { var f = 'effect.wav'; } });";
    attach_plugin_code_sync(code, "fx.js", &[], engine_dir).unwrap();
    std::fs::create_dir_all(engine_dir.join("plugins/assets")).unwrap();
    std::fs::write(engine_dir.join("plugins/assets/effect.wav"), b"original").unwrap();

    // Re-attach same plugin (updated code) — should overwrite, not rename
    let code_v2 = "/**\n * @assets\n * effect.wav = https://example.com/effect_v2.wav\n */\nEnginePlugins.register({ name: 'fx', version: '2.0', init: function() { var f = 'effect.wav'; } });";
    attach_plugin_code_sync(code_v2, "fx.js", &[], engine_dir).unwrap();

    let stored = std::fs::read_to_string(engine_dir.join("plugins/fx.js")).unwrap();
    assert!(stored.contains("effect.wav"), "Same plugin re-attached — name unchanged");
    assert!(!stored.contains("effect_2.wav"), "Should NOT be renamed when re-attaching self");
}

#[test]
fn test_asset_multiple_collisions_in_one_plugin() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Plugin A has hit.wav and boom.wav
    let code_a = "/**\n * @assets\n * hit.wav = https://example.com/a/hit.wav\n * boom.wav = https://example.com/a/boom.wav\n */\nEnginePlugins.register({ name: 'sfx_a', version: '1.0', init: function() {} });";
    attach_plugin_code_sync(code_a, "sfx_a.js", &[], engine_dir).unwrap();
    let assets_dir = engine_dir.join("plugins/assets");
    std::fs::create_dir_all(&assets_dir).unwrap();
    std::fs::write(assets_dir.join("hit.wav"), b"a_hit").unwrap();
    std::fs::write(assets_dir.join("boom.wav"), b"a_boom").unwrap();

    // Plugin B also has hit.wav and boom.wav — both should be renamed
    let code_b = "/**\n * @assets\n * hit.wav = https://example.com/b/hit.wav\n * boom.wav = https://example.com/b/boom.wav\n */\nEnginePlugins.register({ name: 'sfx_b', version: '1.0', init: function() {\n\tvar h = 'hit.wav'; var b = 'boom.wav';\n} });";
    attach_plugin_code_sync(code_b, "sfx_b.js", &[], engine_dir).unwrap();

    let stored_b = std::fs::read_to_string(engine_dir.join("plugins/sfx_b.js")).unwrap();
    assert!(stored_b.contains("hit_2.wav"), "hit.wav should be renamed to hit_2.wav");
    assert!(stored_b.contains("boom_2.wav"), "boom.wav should be renamed to boom_2.wav");
    // @assets block should also be rewritten
    assert!(stored_b.contains("* hit_2.wav = "), "@assets entry for hit should be renamed");
    assert!(stored_b.contains("* boom_2.wav = "), "@assets entry for boom should be renamed");

    // Plugin A untouched
    let stored_a = std::fs::read_to_string(engine_dir.join("plugins/sfx_a.js")).unwrap();
    assert!(stored_a.contains("hit.wav"), "Plugin A unchanged");
}

#[test]
fn test_asset_triple_collision_increments_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let assets_dir = engine_dir.join("plugins/assets");

    // Plugin A: beep.opus
    let code_a = "/**\n * @assets\n * beep.opus = https://example.com/a/beep.opus\n */\nEnginePlugins.register({ name: 'a', version: '1.0', init: function() {} });";
    attach_plugin_code_sync(code_a, "a.js", &[], engine_dir).unwrap();
    std::fs::create_dir_all(&assets_dir).unwrap();
    std::fs::write(assets_dir.join("beep.opus"), b"a").unwrap();

    // Plugin B: beep.opus → renamed to beep_2.opus
    let code_b = "/**\n * @assets\n * beep.opus = https://example.com/b/beep.opus\n */\nEnginePlugins.register({ name: 'b', version: '1.0', init: function() { var f = 'beep.opus'; } });";
    attach_plugin_code_sync(code_b, "b.js", &[], engine_dir).unwrap();
    // Create the renamed asset on disk
    std::fs::write(assets_dir.join("beep_2.opus"), b"b").unwrap();

    let stored_b = std::fs::read_to_string(engine_dir.join("plugins/b.js")).unwrap();
    assert!(stored_b.contains("beep_2.opus"), "Plugin B gets _2 suffix");

    // Plugin C: beep.opus → _2 is taken → renamed to beep_3.opus
    let code_c = "/**\n * @assets\n * beep.opus = https://example.com/c/beep.opus\n */\nEnginePlugins.register({ name: 'c', version: '1.0', init: function() { var f = 'beep.opus'; } });";
    attach_plugin_code_sync(code_c, "c.js", &[], engine_dir).unwrap();

    let stored_c = std::fs::read_to_string(engine_dir.join("plugins/c.js")).unwrap();
    assert!(stored_c.contains("beep_3.opus"), "Plugin C gets _3 suffix (2 already taken)");
}

#[test]
fn test_asset_collision_cleanup_isolation() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let assets_dir = engine_dir.join("plugins/assets");

    // Create case for remove_plugin
    let case_dir = engine_dir.join("case/90030");
    std::fs::create_dir_all(&case_dir).unwrap();
    write_manifest(&CaseManifest {
        case_id: 90030,
        title: "T".into(), author: "T".into(), language: "en".into(),
        download_date: "2026-01-01".into(), format: "t".into(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(), failed_assets: vec![],
        has_plugins: false, has_case_config: false,
    }, &case_dir).unwrap();

    // Plugin A: ring.mp3
    let code_a = "/**\n * @assets\n * ring.mp3 = https://example.com/a/ring.mp3\n */\nEnginePlugins.register({ name: 'alarm_a', version: '1.0', init: function() {} });";
    attach_plugin_code_sync(code_a, "alarm_a.js", &[90030], engine_dir).unwrap();
    std::fs::create_dir_all(&assets_dir).unwrap();
    std::fs::write(assets_dir.join("ring.mp3"), b"original_ring").unwrap();

    // Plugin B: ring.mp3 → renamed to ring_2.mp3
    let code_b = "/**\n * @assets\n * ring.mp3 = https://example.com/b/ring.mp3\n */\nEnginePlugins.register({ name: 'alarm_b', version: '1.0', init: function() {} });";
    attach_plugin_code_sync(code_b, "alarm_b.js", &[90030], engine_dir).unwrap();
    std::fs::write(assets_dir.join("ring_2.mp3"), b"renamed_ring").unwrap();

    // Deleting plugin B should remove ring_2.mp3 but NOT ring.mp3
    remove_plugin(90030, "alarm_b.js", engine_dir).unwrap();

    assert!(!engine_dir.join("plugins/alarm_b.js").exists(), "Plugin B JS deleted");
    assert!(!assets_dir.join("ring_2.mp3").exists(), "Renamed asset deleted with plugin B");
    assert!(assets_dir.join("ring.mp3").exists(), "Plugin A's asset survives");
    assert!(engine_dir.join("plugins/alarm_a.js").exists(), "Plugin A JS untouched");
}

#[test]
fn test_remove_plugin_cleans_config_and_resolved() {
    let engine = tempfile::tempdir().unwrap();
    let case_id = 88001u32;
    let case_dir = engine.path().join("case").join(case_id.to_string());
    std::fs::create_dir_all(&case_dir).unwrap();

    let manifest = CaseManifest {
        case_id,
        title: "Config Test".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: std::collections::HashMap::new(),
        failed_assets: vec![], has_plugins: false, has_case_config: true,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Attach plugin
    attach_plugin_code_sync("// plugin code", "test_plugin.js", &[case_id], engine.path()).unwrap();

    // Create case_config.json with plugin params
    fs::write(case_dir.join("case_config.json"), r#"{"plugins":{"test_plugin":{"volume":0.5}}}"#).unwrap();

    // Create resolved_plugins.json
    fs::write(case_dir.join("resolved_plugins.json"), r#"{"active":[]}"#).unwrap();

    // Remove the plugin
    remove_plugin(case_id, "test_plugin.js", engine.path()).unwrap();

    // Verify plugin file deleted from global
    assert!(!engine.path().join("plugins/test_plugin.js").exists());

    // Verify case_config.json cleaned
    let config_text = fs::read_to_string(case_dir.join("case_config.json")).unwrap();
    let config: serde_json::Value = serde_json::from_str(&config_text).unwrap();
    assert!(config["plugins"].get("test_plugin").is_none());

    // Verify resolved_plugins.json deleted
    assert!(!case_dir.join("resolved_plugins.json").exists());
}

#[test]
fn test_toggle_plugin_updates_scope() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let case_dir = engine_dir.join("case/80001");
    std::fs::create_dir_all(&case_dir).unwrap();
    create_test_case_for_save(engine_dir, 80001);

    attach_plugin_code_sync("// test", "test.js", &[80001], engine_dir).unwrap();

    // Disable — removes from enabled_for
    toggle_plugin(80001, "test.js", false, engine_dir).unwrap();
    let gm_text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let gm: serde_json::Value = serde_json::from_str(&gm_text).unwrap();
    let enabled = gm["plugins"]["test.js"]["scope"]["enabled_for"].as_array().unwrap();
    assert!(!enabled.iter().any(|v| v.as_u64() == Some(80001)));

    // Re-enable — adds back to enabled_for
    toggle_plugin(80001, "test.js", true, engine_dir).unwrap();
    let gm_text = std::fs::read_to_string(engine_dir.join("plugins/manifest.json")).unwrap();
    let gm: serde_json::Value = serde_json::from_str(&gm_text).unwrap();
    let enabled = gm["plugins"]["test.js"]["scope"]["enabled_for"].as_array().unwrap();
    assert!(enabled.iter().any(|v| v.as_u64() == Some(80001)));
}
