use super::*;

fn test_client() -> reqwest::Client {
    reqwest::Client::new()
}

/// Sync wrapper for attach_plugin_code in tests (no @assets, no downloads needed).
fn attach_plugin_code_sync(code: &str, filename: &str, case_ids: &[u32], engine_dir: &std::path::Path) -> Result<Vec<u32>, String> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(attach_plugin_code(code, filename, case_ids, engine_dir, &test_client()))
}

#[tokio::test]
async fn test_import_aaoplug_extracts_to_case() {
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
    let result = import_aaoplug(&plug_path, &[99999], engine_dir, &test_client()).await;
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

#[tokio::test]
async fn test_import_aaoplug_invalid_zip() {
    let dir = tempfile::tempdir().unwrap();
    let bad_path = dir.path().join("bad.aaoplug");
    std::fs::write(&bad_path, "not a zip").unwrap();
    let result = import_aaoplug(&bad_path, &[1], dir.path(), &test_client()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_import_aaoplug_nonexistent_case() {
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
    let result = import_aaoplug(&plug_path, &[99998], dir.path(), &test_client()).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty(), "Should skip non-existent case");
}

#[test]
fn test_attach_plugin_code_sync() {
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
    let result = attach_plugin_code_sync(
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

    attach_plugin_code_sync("// x", "x.js", &[90003], engine_dir).unwrap();
    attach_plugin_code_sync("// y", "y.js", &[90003], engine_dir).unwrap();

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

    attach_plugin_code_sync("// only", "only.js", &[90004], engine_dir).unwrap();
    assert!(read_manifest(&case_dir).unwrap().has_plugins);

    remove_plugin(90004, "only.js", engine_dir).unwrap();

    // No more plugins -- has_plugins should be false
    let updated = read_manifest(&case_dir).unwrap();
    assert!(!updated.has_plugins);

    // File gone
    assert!(!case_dir.join("plugins/only.js").exists());
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

#[test]
fn test_toggle_plugin_disables() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    create_test_case_for_save(engine_dir, 80001);
    attach_plugin_code_sync("// test", "test.js", &[80001], engine_dir).unwrap();

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
    attach_plugin_code_sync("// test", "test.js", &[80002], engine_dir).unwrap();

    // Disable then re-enable
    toggle_plugin(80002, "test.js", false, engine_dir).unwrap();
    toggle_plugin(80002, "test.js", true, engine_dir).unwrap();

    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(engine_dir.join("case/80002/plugins/manifest.json")).unwrap()
    ).unwrap();
    let disabled = manifest.get("disabled").and_then(|d| d.as_array());
    assert!(disabled.is_none() || disabled.unwrap().is_empty());
}
