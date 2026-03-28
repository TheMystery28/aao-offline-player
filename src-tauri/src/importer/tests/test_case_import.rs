use super::*;
use std::collections::HashMap;

#[test]
fn test_import_aaocase_zip_basic() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    let zip_path = create_test_aaocase(tmp.path(), 77777);
    let manifest = import_aaocase_zip(&zip_path, engine.path(), None).unwrap().manifest;

    assert_eq!(manifest.case_id, 77777);
    assert_eq!(manifest.title, "ZIP Test Case");
    assert_eq!(manifest.author, "ZipTester");
    assert_eq!(manifest.language, "en");
    assert_eq!(manifest.assets.total_downloaded, 2);

    // Verify files extracted
    let case_dir = engine.path().join("case/77777");
    assert!(case_dir.join("manifest.json").exists());
    assert!(case_dir.join("trial_info.json").exists());
    assert!(case_dir.join("trial_data.json").exists());
    assert!(case_dir.join("assets/bg.png").exists());
    assert!(case_dir.join("assets/music.mp3").exists());

    // Verify asset content
    let bg = fs::read_to_string(case_dir.join("assets/bg.png")).unwrap();
    assert_eq!(bg, "fake png data");
}

#[test]
fn test_import_aaocase_zip_duplicate_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    let zip_path = create_test_aaocase(tmp.path(), 88888);

    // First import succeeds
    import_aaocase_zip(&zip_path, engine.path(), None).unwrap();

    // Second import should fail
    let result = import_aaocase_zip(&zip_path, engine.path(), None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already exists"));
}

#[test]
fn test_import_aaocase_zip_invalid_file() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Write a non-ZIP file
    let bad_path = tmp.path().join("bad.aaocase");
    fs::write(&bad_path, "this is not a zip file").unwrap();

    let result = import_aaocase_zip(&bad_path, engine.path(), None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid ZIP file"));
}

#[test]
fn test_import_aaocase_zip_missing_manifest() {
    use std::io::Write;

    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Create ZIP without manifest.json
    let zip_path = tmp.path().join("no_manifest.aaocase");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("trial_data.json", options).unwrap();
    zip.write_all(b"{}").unwrap();
    zip.finish().unwrap();

    let result = import_aaocase_zip(&zip_path, engine.path(), None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("manifest.json"));
}

/// Regression: single-case ZIP import still works after multi-case support was added.
#[test]
fn test_import_single_case_still_works_after_multi_support() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Create a classic single-case ZIP (no sequence.json)
    let zip_path = create_test_aaocase(tmp.path(), 11111);
    let manifest = import_aaocase_zip(&zip_path, engine.path(), None).unwrap().manifest;

    assert_eq!(manifest.case_id, 11111);
    assert_eq!(manifest.title, "ZIP Test Case");
    let case_dir = engine.path().join("case/11111");
    assert!(case_dir.join("manifest.json").exists());
    assert!(case_dir.join("trial_data.json").exists());
    assert!(case_dir.join("assets/bg.png").exists());
}

/// Single-case ZIP import preserves failed_assets field in manifest roundtrip.
#[test]
fn test_import_single_case_backward_compat_with_failed_assets() {
    use std::io::Write;

    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    let zip_path = tmp.path().join("with_failures.aaocase");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // manifest.json with failed_assets
    let manifest = serde_json::json!({
        "case_id": 88001,
        "title": "Failed Assets Test",
        "author": "Tester",
        "language": "en",
        "download_date": "2025-06-01T00:00:00Z",
        "format": "Def6",
        "sequence": null,
        "assets": {
            "case_specific": 1,
            "shared_defaults": 0,
            "total_downloaded": 1,
            "total_size_bytes": 50
        },
        "asset_map": {
            "http://ok.com/bg.png": "assets/bg.png"
        },
        "failed_assets": [
            {
                "url": "http://dead.com/music.mp3",
                "asset_type": "music",
                "local_path": "assets/music-hash.mp3",
                "error": "HTTP 404"
            }
        ]
    });
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes()).unwrap();

    // trial_info.json
    let info = serde_json::json!({"id": 88001, "title": "Failed Assets Test", "author": "Tester", "language": "en", "format": "Def6", "last_edit_date": 0, "sequence": null});
    zip.start_file("trial_info.json", options).unwrap();
    zip.write_all(serde_json::to_string(&info).unwrap().as_bytes()).unwrap();

    // trial_data.json
    zip.start_file("trial_data.json", options).unwrap();
    zip.write_all(b"{}").unwrap();

    // asset
    zip.start_file("assets/bg.png", options).unwrap();
    zip.write_all(b"fake png").unwrap();

    zip.finish().unwrap();

    let imported = import_aaocase_zip(&zip_path, engine.path(), None).unwrap().manifest;
    assert_eq!(imported.case_id, 88001);
    assert_eq!(imported.failed_assets.len(), 1, "failed_assets should roundtrip");
    assert_eq!(imported.failed_assets[0].url, "http://dead.com/music.mp3");
    assert_eq!(imported.failed_assets[0].error, "HTTP 404");
}

/// Test multi-case ZIP import.
#[test]
fn test_import_multi_case_zip() {
    let engine_export = tempfile::tempdir().unwrap();
    let engine_import = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    // Set up two cases
    for &case_id in &[69063u32, 69064] {
        let case_dir = engine_export.path().join("case").join(case_id.to_string());
        fs::create_dir_all(case_dir.join("assets")).unwrap();

        let manifest = CaseManifest {
            case_id,
            title: format!("Part {}", if case_id == 69063 { "Investigation" } else { "Trial" }),
            author: "Author".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: Some(serde_json::json!({
                "title": "A Turnabout Called Justice",
                "list": [{"id": 69063, "title": "Investigation"}, {"id": 69064, "title": "Trial"}]
            })),
            assets: AssetSummary {
                case_specific: 1, shared_defaults: 0,
                total_downloaded: 1, total_size_bytes: 12,
            },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), format!(r#"{{"id":{}}}"#, case_id)).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
        fs::write(case_dir.join("assets").join("img.png"), "fake image").unwrap();
    }

    // Export sequence
    let seq_list = serde_json::json!([
        {"id": 69063, "title": "Investigation"},
        {"id": 69064, "title": "Trial"}
    ]);
    let export_path = tmp.path().join("sequence.aaocase");
    export_sequence(
        &[69063, 69064],
        "A Turnabout Called Justice",
        &seq_list,
        engine_export.path(),
        &export_path,
        None,
        None,
        true,
    ).unwrap();

    // Import into fresh engine dir
    let manifest = import_aaocase_zip(&export_path, engine_import.path(), None).unwrap().manifest;
    assert_eq!(manifest.case_id, 69063); // First case's manifest

    // Both cases should be imported
    assert!(engine_import.path().join("case/69063/manifest.json").exists());
    assert!(engine_import.path().join("case/69064/manifest.json").exists());
    assert!(engine_import.path().join("case/69063/assets/img.png").exists());
    assert!(engine_import.path().join("case/69064/assets/img.png").exists());
}

/// Test multi-case import skips existing cases.
#[test]
fn test_import_multi_case_skips_existing() {
    let engine_export = tempfile::tempdir().unwrap();
    let engine_import = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    // Set up two cases for export
    for &case_id in &[69063u32, 69064] {
        let case_dir = engine_export.path().join("case").join(case_id.to_string());
        fs::create_dir_all(case_dir.join("assets")).unwrap();
        let manifest = CaseManifest {
            case_id,
            title: format!("Part {}", case_id),
            author: "Author".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: AssetSummary {
                case_specific: 0, shared_defaults: 0,
                total_downloaded: 0, total_size_bytes: 0,
            },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), "{}").unwrap();
        fs::write(case_dir.join("trial_data.json"), "{}").unwrap();
    }

    // Export sequence
    let seq_list = serde_json::json!([{"id": 69063, "title": "P1"}, {"id": 69064, "title": "P2"}]);
    let export_path = tmp.path().join("seq.aaocase");
    export_sequence(&[69063, 69064], "Seq", &seq_list, engine_export.path(), &export_path, None, None, true).unwrap();

    // Pre-install case 69063 in import engine
    let pre_case_dir = engine_import.path().join("case/69063");
    fs::create_dir_all(&pre_case_dir).unwrap();
    let pre_manifest = CaseManifest {
        case_id: 69063,
        title: "Already Here".to_string(),
        author: "Pre".to_string(),
        language: "en".to_string(),
        download_date: "2024-01-01T00:00:00Z".to_string(),
        format: "v6".to_string(),
        sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: HashMap::new(),
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&pre_manifest, &pre_case_dir).unwrap();

    // Import -- should skip 69063 and import 69064
    let manifest = import_aaocase_zip(&export_path, engine_import.path(), None).unwrap().manifest;
    // First manifest should be the pre-existing one
    assert_eq!(manifest.case_id, 69063);
    assert_eq!(manifest.title, "Already Here"); // wasn't overwritten

    // 69064 should be imported
    assert!(engine_import.path().join("case/69064/manifest.json").exists());
}

/// Importing a multi-case ZIP where sequence.json has an empty list should return an error.
#[test]
fn test_import_multi_case_empty_sequence_list() {
    use std::io::Write;

    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Create a ZIP with sequence.json containing empty list
    let zip_path = tmp.path().join("empty_list.aaocase");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    let seq = serde_json::json!({
        "title": "Empty Sequence",
        "list": []
    });
    zip.start_file("sequence.json", options).unwrap();
    zip.write_all(serde_json::to_string(&seq).unwrap().as_bytes()).unwrap();
    zip.finish().unwrap();

    let result = import_aaocase_zip(&zip_path, engine.path(), None);
    assert!(result.is_err(), "Should fail with empty sequence list");
    assert!(
        result.unwrap_err().contains("empty list"),
        "Error should mention empty list"
    );
}

/// Regression: import_aaocase_zip without saves.json returns a valid manifest.
#[test]
fn test_import_aaocase_without_saves_returns_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    let zip_path = create_test_aaocase(tmp.path(), 77004);
    let manifest = import_aaocase_zip(&zip_path, engine.path(), None).unwrap().manifest;
    assert_eq!(manifest.case_id, 77004);
    assert_eq!(manifest.title, "ZIP Test Case");
    // Verify case files were properly installed
    let case_dir = engine.path().join("case/77004");
    assert!(case_dir.join("manifest.json").exists());
    assert!(case_dir.join("trial_data.json").exists());
}

/// Import a ZIP with saves.json -- result should include saves.
#[test]
fn test_import_aaocase_with_saves() {
    use std::io::Write;

    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Build a ZIP manually with saves.json
    let zip_path = tmp.path().join("with_saves.aaocase");
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    // manifest.json
    let manifest = serde_json::json!({
        "case_id": 78002,
        "title": "Import Saves Test",
        "author": "Tester",
        "language": "en",
        "download_date": "2025-01-01T00:00:00Z",
        "format": "v6",
        "sequence": null,
        "assets": { "case_specific": 0, "shared_defaults": 0, "total_downloaded": 0, "total_size_bytes": 0 },
        "asset_map": {},
        "failed_assets": []
    });
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(serde_json::to_string(&manifest).unwrap().as_bytes()).unwrap();

    // trial_info.json
    zip.start_file("trial_info.json", options).unwrap();
    zip.write_all(br#"{"id":78002}"#).unwrap();

    // trial_data.json
    zip.start_file("trial_data.json", options).unwrap();
    zip.write_all(br#"{"frames":[0]}"#).unwrap();

    // saves.json
    let saves = serde_json::json!({
        "78002": {
            "1710000000000": "{\"frame\":3}",
            "1710001000000": "{\"frame\":7}"
        }
    });
    zip.start_file("saves.json", options).unwrap();
    zip.write_all(serde_json::to_string(&saves).unwrap().as_bytes()).unwrap();

    zip.finish().unwrap();

    // Import
    let result = import_aaocase_zip(&zip_path, engine.path(), None).unwrap();
    assert_eq!(result.manifest.case_id, 78002);
    assert!(result.saves.is_some(), "Import result should contain saves");

    let imported_saves = result.saves.unwrap();
    assert!(imported_saves["78002"].is_object());
    let case_saves = imported_saves["78002"].as_object().unwrap();
    assert_eq!(case_saves.len(), 2, "Should have 2 save entries");
}

/// Import a ZIP without saves.json -- saves should be None.
#[test]
fn test_import_aaocase_without_saves_returns_none() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    let zip_path = create_test_aaocase(tmp.path(), 78003);
    let result = import_aaocase_zip(&zip_path, engine.path(), None).unwrap();
    assert_eq!(result.manifest.case_id, 78003);
    assert!(result.saves.is_none(), "Import without saves.json should have saves=None");
}

/// Full roundtrip: export with saves -> import -> saves preserved.
#[test]
fn test_export_import_saves_roundtrip() {
    let engine1 = tempfile::tempdir().unwrap();
    let engine2 = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let case_dir = engine1.path().join("case/78004");
    fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id: 78004,
        title: "Saves Roundtrip".to_string(),
        author: "Tester".to_string(),
        language: "en".to_string(),
        download_date: "2025-01-01T00:00:00Z".to_string(),
        format: "v6".to_string(),
        sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0 },
        asset_map: HashMap::new(),
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    fs::write(case_dir.join("trial_info.json"), r#"{"id":78004}"#).unwrap();
    fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();

    let saves = serde_json::json!({
        "78004": {
            "1710000000000": "{\"health\":80,\"frame\":10}",
            "1710005000000": "{\"health\":120,\"frame\":1}"
        }
    });

    // Export with saves
    let zip_path = tmp.path().join("saves_roundtrip.aaocase");
    export_aaocase(78004, engine1.path(), &zip_path, None, Some(&saves), true).unwrap();

    // Import
    let result = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap();
    assert_eq!(result.manifest.case_id, 78004);
    assert!(result.saves.is_some(), "Saves should survive roundtrip");

    let imported_saves = result.saves.unwrap();
    let case_saves = imported_saves["78004"].as_object().unwrap();
    assert_eq!(case_saves.len(), 2);
    assert!(case_saves.contains_key("1710000000000"));
    assert!(case_saves.contains_key("1710005000000"));
}
