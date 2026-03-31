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
