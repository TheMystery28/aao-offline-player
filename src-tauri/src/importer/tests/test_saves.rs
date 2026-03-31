use super::*;
use std::io;

/// Sync wrapper for attach_plugin_code in tests (no @assets, no downloads needed).
fn attach_plugin_code_sync(code: &str, filename: &str, case_ids: &[u32], engine_dir: &std::path::Path) -> Result<Vec<u32>, String> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = reqwest::Client::new();
    rt.block_on(attach_plugin_code(code, filename, case_ids, engine_dir, &client))
}

#[test]
fn test_export_aaosave_basic() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    create_test_case_for_save(engine_dir, 50001);

    let saves = serde_json::json!({
        "50001": { "1700000000000": "{\"trial_id\":50001}" }
    });
    let dest = engine_dir.join("test.aaosave");
    let size = export_aaosave(&[50001], &saves, false, &dest, engine_dir).unwrap();
    assert!(size > 0);

    // Verify ZIP contents
    let file = std::fs::File::open(&dest).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    assert!(archive.by_name("saves.json").is_ok());
    assert!(archive.by_name("metadata.json").is_ok());

    let meta_text = read_zip_text(&mut archive, "metadata.json").unwrap();
    let meta: serde_json::Value = serde_json::from_str(&meta_text).unwrap();
    assert_eq!(meta["version"], 1);
    let export_date = meta["export_date"].as_str().unwrap();
    assert!(export_date.contains("T"), "export_date should be ISO-8601: {}", export_date);
    assert!(export_date.ends_with("Z"), "export_date should end with Z: {}", export_date);
    assert_eq!(meta["has_plugins"], false);
    let cases = meta["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0]["id"], 50001);
    assert_eq!(cases[0]["save_count"], 1);
}

#[test]
fn test_export_aaosave_with_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    create_test_case_for_save(engine_dir, 50002);
    attach_plugin_code_sync("// test", "test.js", &[50002], engine_dir).unwrap();

    let saves = serde_json::json!({
        "50002": { "1700000000000": "{\"trial_id\":50002}" }
    });
    let dest = engine_dir.join("test_plug.aaosave");
    export_aaosave(&[50002], &saves, true, &dest, engine_dir).unwrap();

    let file = std::fs::File::open(&dest).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    assert!(archive.by_name("plugins/50002/manifest.json").is_ok());
    assert!(archive.by_name("plugins/50002/test.js").is_ok());

    let meta_text = read_zip_text(&mut archive, "metadata.json").unwrap();
    let meta: serde_json::Value = serde_json::from_str(&meta_text).unwrap();
    assert_eq!(meta["has_plugins"], true);
}

#[test]
fn test_import_aaosave_basic() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create a .aaosave manually
    let dest = engine_dir.join("import_test.aaosave");
    let file = std::fs::File::create(&dest).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    let saves = serde_json::json!({ "60001": { "999": "{\"trial_id\":60001}" } });
    zip.start_file("saves.json", options).unwrap();
    io::Write::write_all(&mut zip, serde_json::to_string(&saves).unwrap().as_bytes()).unwrap();

    let meta = serde_json::json!({ "version": 1, "cases": [], "has_plugins": false });
    zip.start_file("metadata.json", options).unwrap();
    io::Write::write_all(&mut zip, serde_json::to_string(&meta).unwrap().as_bytes()).unwrap();
    zip.finish().unwrap();

    let result = import_aaosave(&dest, engine_dir).unwrap();
    assert_eq!(result.saves["60001"]["999"], "{\"trial_id\":60001}");
    assert!(result.plugins_installed.is_empty());
}

#[test]
fn test_import_aaosave_with_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    let case_dir = create_test_case_for_save(engine_dir, 60002);

    // Create .aaosave with plugins
    let dest = engine_dir.join("plug_import.aaosave");
    let file = std::fs::File::create(&dest).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    let saves = serde_json::json!({ "60002": { "111": "{}" } });
    zip.start_file("saves.json", options).unwrap();
    io::Write::write_all(&mut zip, serde_json::to_string(&saves).unwrap().as_bytes()).unwrap();

    let meta = serde_json::json!({ "version": 1, "cases": [], "has_plugins": true });
    zip.start_file("metadata.json", options).unwrap();
    io::Write::write_all(&mut zip, serde_json::to_string(&meta).unwrap().as_bytes()).unwrap();

    zip.start_file("plugins/60002/manifest.json", options).unwrap();
    io::Write::write_all(&mut zip, b"{\"scripts\":[\"plugin.js\"]}").unwrap();

    zip.start_file("plugins/60002/plugin.js", options).unwrap();
    io::Write::write_all(&mut zip, b"console.log('hi');").unwrap();
    zip.finish().unwrap();

    let result = import_aaosave(&dest, engine_dir).unwrap();
    assert_eq!(result.plugins_installed, vec![60002]);
    assert!(case_dir.join("plugins/manifest.json").exists());
    assert!(case_dir.join("plugins/plugin.js").exists());
    assert!(read_manifest(&case_dir).unwrap().has_plugins);
}

#[test]
fn test_import_aaosave_missing_saves() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("bad.aaosave");
    let file = std::fs::File::create(&dest).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("metadata.json", options).unwrap();
    io::Write::write_all(&mut zip, b"{}").unwrap();
    zip.finish().unwrap();

    let result = import_aaosave(&dest, dir.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("saves.json"));
}

#[test]
fn test_export_import_aaosave_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();
    create_test_case_for_save(engine_dir, 70001);
    attach_plugin_code_sync("// roundtrip", "rt.js", &[70001], engine_dir).unwrap();

    let saves = serde_json::json!({
        "70001": {
            "1000": "{\"trial_id\":70001,\"frame\":5}",
            "2000": "{\"trial_id\":70001,\"frame\":10}"
        }
    });

    let dest = engine_dir.join("roundtrip.aaosave");
    export_aaosave(&[70001], &saves, true, &dest, engine_dir).unwrap();

    // Import into a fresh engine dir with the same case
    let dir2 = tempfile::tempdir().unwrap();
    let engine_dir2 = dir2.path();
    create_test_case_for_save(engine_dir2, 70001);

    let result = import_aaosave(&dest, engine_dir2).unwrap();

    // Saves preserved
    assert_eq!(result.saves["70001"]["1000"], "{\"trial_id\":70001,\"frame\":5}");
    assert_eq!(result.saves["70001"]["2000"], "{\"trial_id\":70001,\"frame\":10}");

    // Plugins installed
    assert_eq!(result.plugins_installed, vec![70001]);
    let case_dir2 = engine_dir2.join("case/70001");
    assert!(case_dir2.join("plugins/rt.js").exists());
    assert!(read_manifest(&case_dir2).unwrap().has_plugins);
}
