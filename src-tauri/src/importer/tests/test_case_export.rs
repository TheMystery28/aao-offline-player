use super::*;
use std::collections::HashMap;

#[test]
fn test_export_aaocase_basic() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // First, import a case so we have something to export
    let html = r#"<html>
<script>
var trial_information = {"author":"Tester","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":44444,"language":"en","last_edit_date":1000000,"sequence":null,"title":"Export Test"};
var initial_trial_data = {"profiles":[0,{"icon":"assets/icon.png","short_name":"Hero","custom_sprites":[]}],"frames":[0],"evidence":[0],"places":[0],"cross_examinations":[0]};
</script>
</html>"#;
    fs::write(source.path().join("index.html"), html).unwrap();
    let assets_dir = source.path().join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("icon.png"), "fake png data").unwrap();
    fs::write(assets_dir.join("music.mp3"), "fake mp3 data").unwrap();

    import_aaoffline(source.path(), engine.path(), None).unwrap();

    // Now export it
    let export_path = source.path().join("test.aaocase");
    let size = export_aaocase(44444, engine.path(), &export_path, None, None, true).unwrap();
    assert!(size > 0, "ZIP file should have non-zero size");
    assert!(export_path.exists(), "ZIP file should exist on disk");

    // Verify we can reimport the exported file into a fresh engine dir
    let engine2 = tempfile::tempdir().unwrap();
    let manifest = import_aaocase_zip(&export_path, engine2.path(), None).unwrap().manifest;
    assert_eq!(manifest.case_id, 44444);
    assert_eq!(manifest.title, "Export Test");

    let case_dir = engine2.path().join("case/44444");
    assert!(case_dir.join("manifest.json").exists());
    assert!(case_dir.join("trial_data.json").exists());
    assert!(case_dir.join("assets/icon.png").exists());
    assert!(case_dir.join("assets/music.mp3").exists());
}

#[test]
fn test_export_aaocase_missing_case() {
    let engine = tempfile::tempdir().unwrap();
    let export_path = engine.path().join("missing.aaocase");
    let result = export_aaocase(99999, engine.path(), &export_path, None, None, true);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

/// Test multi-case sequence export creates valid ZIP structure.
#[test]
fn test_export_sequence_creates_valid_zip() {
    let engine = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    // Set up two cases on disk
    for &case_id in &[69063u32, 69064] {
        let case_dir = engine.path().join("case").join(case_id.to_string());
        fs::create_dir_all(case_dir.join("assets")).unwrap();

        let manifest = CaseManifest {
            case_id,
            title: format!("Part {}", case_id),
            author: "Author".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: Some(serde_json::json!({
                "title": "Test Sequence",
                "list": [{"id": 69063, "title": "Part 1"}, {"id": 69064, "title": "Part 2"}]
            })),
            assets: AssetSummary {
                case_specific: 1, shared_defaults: 0,
                total_downloaded: 1, total_size_bytes: 10,
            },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), r#"{"id":0}"#).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
        fs::write(case_dir.join("assets").join("test.png"), "fake").unwrap();
    }

    let seq_list = serde_json::json!([
        {"id": 69063, "title": "Part 1"},
        {"id": 69064, "title": "Part 2"}
    ]);
    let export_path = tmp.path().join("sequence.aaocase");
    let size = export_sequence(
        &[69063, 69064],
        "Test Sequence",
        &seq_list,
        engine.path(),
        &export_path,
        None,
        None,
        true,
    ).unwrap();

    assert!(size > 0);
    assert!(export_path.exists());

    // Verify ZIP structure
    let file = fs::File::open(&export_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut entry_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        entry_names.push(archive.by_index(i).unwrap().name().to_string());
    }
    assert!(entry_names.contains(&"sequence.json".to_string()));
    assert!(entry_names.contains(&"69063/manifest.json".to_string()));
    assert!(entry_names.contains(&"69064/manifest.json".to_string()));
    assert!(entry_names.contains(&"69063/trial_data.json".to_string()));
    assert!(entry_names.contains(&"69064/trial_data.json".to_string()));
}

/// Exporting a sequence where one case doesn't exist should return an error.
#[test]
fn test_export_sequence_missing_case_returns_error() {
    let engine = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    // Set up only one case
    let case_dir = engine.path().join("case/70001");
    fs::create_dir_all(case_dir.join("assets")).unwrap();
    let manifest = CaseManifest {
        case_id: 70001,
        title: "Existing Part".to_string(),
        author: "Author".to_string(),
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
    fs::write(case_dir.join("trial_info.json"), "{}").unwrap();
    fs::write(case_dir.join("trial_data.json"), "{}").unwrap();

    let seq_list = serde_json::json!([
        {"id": 70001, "title": "Part 1"},
        {"id": 70002, "title": "Part 2"}
    ]);
    let export_path = tmp.path().join("missing_case.aaocase");
    let result = export_sequence(
        &[70001, 70002],
        "Broken Sequence",
        &seq_list,
        engine.path(),
        &export_path,
        None,
        None,
        true,
    );
    assert!(result.is_err(), "Should fail when a case in the sequence doesn't exist");
    assert!(
        result.unwrap_err().to_string().contains("not found"),
        "Error should mention case not found"
    );
}

/// Export with empty case_ids list should create a valid ZIP containing only sequence.json.
#[test]
fn test_export_sequence_empty_list_creates_valid_zip() {
    let engine = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let seq_list = serde_json::json!([]);
    let export_path = tmp.path().join("empty_seq.aaocase");
    let size = export_sequence(
        &[],
        "Empty Sequence",
        &seq_list,
        engine.path(),
        &export_path,
        None,
        None,
        true,
    ).unwrap();

    assert!(size > 0, "ZIP file should have non-zero size");
    assert!(export_path.exists());

    // Verify ZIP structure -- should only have sequence.json
    let file = fs::File::open(&export_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    assert_eq!(archive.len(), 1, "Should contain only sequence.json");
    assert_eq!(archive.by_index(0).unwrap().name(), "sequence.json");
}

/// Regression: export_aaocase should NOT produce a saves.json entry in the ZIP.
#[test]
fn test_export_aaocase_no_saves_json_in_zip() {
    let engine = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    // Set up a case on disk
    let case_dir = engine.path().join("case/77001");
    fs::create_dir_all(case_dir.join("assets")).unwrap();
    let manifest = CaseManifest {
        case_id: 77001,
        title: "No Saves Test".to_string(),
        author: "Tester".to_string(),
        language: "en".to_string(),
        download_date: "2025-01-01T00:00:00Z".to_string(),
        format: "v6".to_string(),
        sequence: None,
        assets: AssetSummary { case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 10 },
        asset_map: HashMap::new(),
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    fs::write(case_dir.join("trial_info.json"), r#"{"id":77001}"#).unwrap();
    fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
    fs::write(case_dir.join("assets/test.png"), "fake").unwrap();

    let export_path = tmp.path().join("no_saves.aaocase");
    export_aaocase(77001, engine.path(), &export_path, None, None, true).unwrap();

    // Verify ZIP does NOT contain saves.json
    let file = fs::File::open(&export_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let entry_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(!entry_names.contains(&"saves.json".to_string()),
        "Current export should not contain saves.json");
    // Should contain the standard files
    assert!(entry_names.contains(&"manifest.json".to_string()));
    assert!(entry_names.contains(&"trial_data.json".to_string()));
    assert!(entry_names.contains(&"trial_info.json".to_string()));
    assert!(entry_names.contains(&"assets/test.png".to_string()));
}

/// Regression: export_sequence should NOT produce saves.json in the ZIP.
#[test]
fn test_export_sequence_no_saves_json_in_zip() {
    let engine = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    // Set up two cases
    for &case_id in &[77002u32, 77003] {
        let case_dir = engine.path().join("case").join(case_id.to_string());
        fs::create_dir_all(case_dir.join("assets")).unwrap();
        let manifest = CaseManifest {
            case_id,
            title: format!("Part {}", case_id),
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
        fs::write(case_dir.join("trial_info.json"), "{}").unwrap();
        fs::write(case_dir.join("trial_data.json"), "{}").unwrap();
    }

    let seq_list = serde_json::json!([{"id": 77002, "title": "P1"}, {"id": 77003, "title": "P2"}]);
    let export_path = tmp.path().join("no_saves_seq.aaocase");
    export_sequence(&[77002, 77003], "No Saves Seq", &seq_list, engine.path(), &export_path, None, None, true).unwrap();

    // Verify ZIP does NOT contain saves.json
    let file = fs::File::open(&export_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let entry_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(!entry_names.contains(&"saves.json".to_string()),
        "Current sequence export should not contain saves.json");
    assert!(entry_names.contains(&"sequence.json".to_string()));
}

/// Export a single case with saves included -- ZIP should contain saves.json.
#[test]
fn test_export_aaocase_with_saves() {
    let engine = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let case_dir = engine.path().join("case/78001");
    fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id: 78001,
        title: "With Saves".to_string(),
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
    fs::write(case_dir.join("trial_info.json"), r#"{"id":78001}"#).unwrap();
    fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();

    let saves = serde_json::json!({
        "78001": {
            "1710000000000": "{\"frame\":5,\"health\":100}"
        }
    });

    let export_path = tmp.path().join("with_saves.aaocase");
    export_aaocase(78001, engine.path(), &export_path, None, Some(&saves), true).unwrap();

    // Verify ZIP contains saves.json
    let file = fs::File::open(&export_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let entry_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(entry_names.contains(&"saves.json".to_string()),
        "Export with saves should contain saves.json");
    assert!(entry_names.contains(&"manifest.json".to_string()));

    // Verify saves.json content
    let saves_content = read_zip_text(&mut archive, "saves.json").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&saves_content).unwrap();
    assert!(parsed["78001"].is_object());
    assert!(parsed["78001"]["1710000000000"].is_string());
}

/// Export a sequence with saves -- ZIP should contain saves.json.
#[test]
fn test_export_sequence_with_saves() {
    let engine = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    for &case_id in &[78005u32, 78006] {
        let case_dir = engine.path().join("case").join(case_id.to_string());
        fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id,
            title: format!("Part {}", case_id),
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
        fs::write(case_dir.join("trial_info.json"), format!(r#"{{"id":{}}}"#, case_id)).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
    }

    let saves = serde_json::json!({
        "78005": { "1710000000000": "{\"frame\":2}" },
        "78006": { "1710001000000": "{\"frame\":5}" }
    });

    let seq_list = serde_json::json!([{"id": 78005, "title": "P1"}, {"id": 78006, "title": "P2"}]);
    let zip_path = tmp.path().join("seq_saves.aaocase");
    export_sequence(&[78005, 78006], "Seq Saves", &seq_list, engine.path(), &zip_path, None, Some(&saves), true).unwrap();

    // Verify ZIP contains saves.json
    let file = fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let entry_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(entry_names.contains(&"saves.json".to_string()),
        "Sequence export with saves should contain saves.json");
    assert!(entry_names.contains(&"sequence.json".to_string()));

    // Verify saves content
    let saves_str = read_zip_text(&mut archive, "saves.json").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&saves_str).unwrap();
    assert!(parsed["78005"].is_object());
    assert!(parsed["78006"].is_object());
}

/// Export with None saves should not include saves.json (same as regression test, but confirms new API).
#[test]
fn test_export_with_none_saves_is_backward_compatible() {
    let engine = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    let case_dir = engine.path().join("case/78009");
    fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id: 78009,
        title: "None Saves Compat".to_string(),
        author: "T".to_string(),
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
    fs::write(case_dir.join("trial_info.json"), r#"{"id":78009}"#).unwrap();
    fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();

    // Export with None saves (backward compatible)
    let zip_path = tmp.path().join("none_saves.aaocase");
    export_aaocase(78009, engine.path(), &zip_path, None, None, true).unwrap();

    let file = fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let entry_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(!entry_names.contains(&"saves.json".to_string()));

    // Import should have saves=None
    let engine2 = tempfile::tempdir().unwrap();
    let result = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap();
    assert!(result.saves.is_none());
}

#[test]
fn test_export_roundtrip_preserves_data() {
    // Import from ZIP, export, reimport -- data should be identical
    let tmp = tempfile::tempdir().unwrap();
    let engine1 = tempfile::tempdir().unwrap();

    let zip_path = create_test_aaocase(tmp.path(), 66666);
    let manifest1 = import_aaocase_zip(&zip_path, engine1.path(), None).unwrap().manifest;

    // Export
    let export_path = tmp.path().join("roundtrip.aaocase");
    export_aaocase(66666, engine1.path(), &export_path, None, None, true).unwrap();

    // Reimport into fresh dir
    let engine2 = tempfile::tempdir().unwrap();
    let manifest2 = import_aaocase_zip(&export_path, engine2.path(), None).unwrap().manifest;

    assert_eq!(manifest1.case_id, manifest2.case_id);
    assert_eq!(manifest1.title, manifest2.title);
    assert_eq!(manifest1.author, manifest2.author);
    assert_eq!(manifest1.language, manifest2.language);

    // Verify asset contents match
    let case1 = engine1.path().join("case/66666");
    let case2 = engine2.path().join("case/66666");
    let data1 = fs::read_to_string(case1.join("trial_data.json")).unwrap();
    let data2 = fs::read_to_string(case2.join("trial_data.json")).unwrap();
    assert_eq!(data1, data2, "trial_data.json should be identical after roundtrip");
}

/// Regression: single-case export + import roundtrip preserves all metadata.
#[test]
fn test_export_import_roundtrip_metadata_preserved() {
    let engine1 = tempfile::tempdir().unwrap();
    let engine2 = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    // Set up case with full metadata
    let case_dir = engine1.path().join("case/77005");
    fs::create_dir_all(case_dir.join("assets")).unwrap();
    let original = CaseManifest {
        case_id: 77005,
        title: "Metadata Roundtrip".to_string(),
        author: "AuthorZ".to_string(),
        language: "fr".to_string(),
        download_date: "2025-03-14T12:00:00Z".to_string(),
        format: "Def6".to_string(),
        sequence: Some(serde_json::json!({"title": "Test Seq", "list": [{"id": 77005, "title": "Only"}]})),
        assets: AssetSummary { case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 8 },
        asset_map: HashMap::new(),
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&original, &case_dir).unwrap();
    fs::write(case_dir.join("trial_info.json"), r#"{"id":77005}"#).unwrap();
    fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0,{"id":1}]}"#).unwrap();
    fs::write(case_dir.join("assets/bg.png"), "fakebg").unwrap();

    // Export
    let zip_path = tmp.path().join("roundtrip.aaocase");
    export_aaocase(77005, engine1.path(), &zip_path, None, None, true).unwrap();

    // Import into fresh engine
    let imported = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap().manifest;
    assert_eq!(imported.case_id, original.case_id);
    assert_eq!(imported.title, original.title);
    assert_eq!(imported.author, original.author);
    assert_eq!(imported.language, original.language);
    assert_eq!(imported.format, original.format);
    assert!(imported.sequence.is_some());

    // Verify asset preserved
    let case2 = engine2.path().join("case/77005");
    assert_eq!(fs::read_to_string(case2.join("assets/bg.png")).unwrap(), "fakebg");
}

/// Regression: sequence export + import roundtrip preserves all case data.
#[test]
fn test_export_import_sequence_roundtrip_data_preserved() {
    let engine1 = tempfile::tempdir().unwrap();
    let engine2 = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    for &case_id in &[77006u32, 77007] {
        let case_dir = engine1.path().join("case").join(case_id.to_string());
        fs::create_dir_all(case_dir.join("assets")).unwrap();
        let manifest = CaseManifest {
            case_id,
            title: format!("Seq Part {}", case_id),
            author: "SeqAuthor".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: Some(serde_json::json!({"title": "Seq Test", "list": [{"id": 77006}, {"id": 77007}]})),
            assets: AssetSummary { case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 5 },
            asset_map: HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();
        fs::write(case_dir.join("trial_info.json"), format!(r#"{{"id":{}}}"#, case_id)).unwrap();
        fs::write(case_dir.join("trial_data.json"), format!(r#"{{"frames":[0,{{"id":{}}}]}}"#, case_id)).unwrap();
        fs::write(case_dir.join("assets/sprite.png"), format!("data{}", case_id)).unwrap();
    }

    let seq_list = serde_json::json!([{"id": 77006, "title": "P1"}, {"id": 77007, "title": "P2"}]);
    let zip_path = tmp.path().join("seq_roundtrip.aaocase");
    export_sequence(&[77006, 77007], "Seq Test", &seq_list, engine1.path(), &zip_path, None, None, true).unwrap();

    let manifest = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap().manifest;
    assert_eq!(manifest.case_id, 77006);

    // Verify both cases present with correct data
    for &case_id in &[77006u32, 77007] {
        let case_dir = engine2.path().join("case").join(case_id.to_string());
        assert!(case_dir.join("manifest.json").exists());
        let data = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
        assert!(data.contains(&format!("\"id\":{}", case_id)));
        let asset = fs::read_to_string(case_dir.join("assets/sprite.png")).unwrap();
        assert_eq!(asset, format!("data{}", case_id));
    }
}

/// Sequence export+import roundtrip with saves.
#[test]
fn test_export_import_sequence_saves_roundtrip() {
    let engine1 = tempfile::tempdir().unwrap();
    let engine2 = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();

    for &case_id in &[78007u32, 78008] {
        let case_dir = engine1.path().join("case").join(case_id.to_string());
        fs::create_dir_all(&case_dir).unwrap();
        let manifest = CaseManifest {
            case_id,
            title: format!("Seq Saves RT {}", case_id),
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
        fs::write(case_dir.join("trial_info.json"), format!(r#"{{"id":{}}}"#, case_id)).unwrap();
        fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0]}"#).unwrap();
    }

    let saves = serde_json::json!({
        "78007": { "1710000000000": "{\"frame\":1}" },
        "78008": { "1710002000000": "{\"frame\":3}" }
    });

    let seq_list = serde_json::json!([{"id": 78007, "title": "P1"}, {"id": 78008, "title": "P2"}]);
    let zip_path = tmp.path().join("seq_saves_rt.aaocase");
    export_sequence(&[78007, 78008], "Seq Saves RT", &seq_list, engine1.path(), &zip_path, None, Some(&saves), true).unwrap();

    // Import
    let result = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap();
    assert_eq!(result.manifest.case_id, 78007);
    assert!(result.saves.is_some(), "Sequence saves should survive roundtrip");

    let imported_saves = result.saves.unwrap();
    assert!(imported_saves["78007"].is_object());
    assert!(imported_saves["78008"].is_object());
}

#[test]
fn test_export_import_plugin_params_with_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create a case with a sequence
    let case_id: u32 = 88001;
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id,
        title: "Seq Export Test".into(),
        author: "A".into(),
        language: "en".into(),
        download_date: "2026-01-01".into(),
        format: "v6".into(),
        sequence: Some(serde_json::json!({"title": "Test Seq", "index": 0})),
        assets: AssetSummary {
            case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0,
        },
        asset_map: std::collections::HashMap::new(),
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    fs::write(case_dir.join("trial_data.json"), "{}").unwrap();

    // Create global plugin manifest with by_sequence override
    let plugins_dir = engine_dir.join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::write(plugins_dir.join("a.js"), "// plugin").unwrap();
    fs::write(plugins_dir.join("manifest.json"), serde_json::to_string_pretty(
        &serde_json::json!({
            "scripts": ["a.js"],
            "plugins": {
                "a.js": {
                    "scope": {"all": true},
                    "params": {
                        "default": {"theme": "dark"},
                        "by_sequence": {
                            "Test Seq": {"theme": "light"}
                        }
                    },
                    "descriptors": null
                }
            }
        })
    ).unwrap()).unwrap();

    // Export
    let export_path = dir.path().join("test_seq.aaocase");
    export_aaocase(case_id, engine_dir, &export_path, None, None, true).unwrap();

    // Verify the ZIP contains by_sequence in plugin_params.json
    let file = fs::File::open(&export_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let pp_text = {
        let mut entry = archive.by_name("plugin_params.json").expect("plugin_params.json should exist");
        let mut s = String::new();
        std::io::Read::read_to_string(&mut entry, &mut s).unwrap();
        s
    };
    let pp: serde_json::Value = serde_json::from_str(&pp_text).unwrap();
    assert_eq!(pp["a.js"]["by_sequence"]["Test Seq"]["theme"], "light");

    // Import into fresh engine dir and verify
    let engine2 = tempfile::tempdir().unwrap();
    let engine2_dir = engine2.path();
    // Create global manifest in target so merge has something to merge into
    let plugins_dir2 = engine2_dir.join("plugins");
    fs::create_dir_all(&plugins_dir2).unwrap();
    fs::write(plugins_dir2.join("a.js"), "// plugin").unwrap();
    fs::write(plugins_dir2.join("manifest.json"), serde_json::to_string_pretty(
        &serde_json::json!({
            "scripts": ["a.js"],
            "plugins": {
                "a.js": {
                    "scope": {"all": true},
                    "params": {"default": {"theme": "dark"}},
                    "descriptors": null
                }
            }
        })
    ).unwrap()).unwrap();

    import_aaocase_zip(&export_path, engine2_dir, None).unwrap();

    let gm_text = fs::read_to_string(plugins_dir2.join("manifest.json")).unwrap();
    let gm: serde_json::Value = serde_json::from_str(&gm_text).unwrap();
    assert_eq!(
        gm["plugins"]["a.js"]["params"]["by_sequence"]["Test Seq"]["theme"],
        "light",
        "by_sequence override should be merged on import"
    );
}

#[test]
fn test_export_import_plugin_params_with_collection() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Create a case
    let case_id: u32 = 88002;
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    fs::create_dir_all(&case_dir).unwrap();
    let manifest = CaseManifest {
        case_id,
        title: "Col Export Test".into(),
        author: "A".into(),
        language: "en".into(),
        download_date: "2026-01-01".into(),
        format: "v6".into(),
        sequence: None,
        assets: AssetSummary {
            case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0,
        },
        asset_map: std::collections::HashMap::new(),
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    fs::write(case_dir.join("trial_data.json"), "{}").unwrap();

    // Create a collection containing this case
    let collections = crate::collections::CollectionsData {
        collections: vec![crate::collections::Collection {
            id: "col_test_1".into(),
            title: "Test Collection".into(),
            items: vec![crate::collections::CollectionItem::Case { case_id }],
            created_date: "2026-01-01".into(),
        }],
    };
    fs::write(
        engine_dir.join("collections.json"),
        serde_json::to_string_pretty(&collections).unwrap(),
    ).unwrap();

    // Create global plugin manifest with by_collection override
    let plugins_dir = engine_dir.join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::write(plugins_dir.join("b.js"), "// plugin").unwrap();
    fs::write(plugins_dir.join("manifest.json"), serde_json::to_string_pretty(
        &serde_json::json!({
            "scripts": ["b.js"],
            "plugins": {
                "b.js": {
                    "scope": {"all": true},
                    "params": {
                        "default": {"enabled": true},
                        "by_collection": {
                            "col_test_1": {"enabled": false}
                        }
                    },
                    "descriptors": null
                }
            }
        })
    ).unwrap()).unwrap();

    // Export
    let export_path = dir.path().join("test_col.aaocase");
    export_aaocase(case_id, engine_dir, &export_path, None, None, true).unwrap();

    // Verify the ZIP contains by_collection in plugin_params.json
    let file = fs::File::open(&export_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let pp_text = {
        let mut entry = archive.by_name("plugin_params.json").expect("plugin_params.json should exist");
        let mut s = String::new();
        std::io::Read::read_to_string(&mut entry, &mut s).unwrap();
        s
    };
    let pp: serde_json::Value = serde_json::from_str(&pp_text).unwrap();
    assert_eq!(pp["b.js"]["by_collection"]["col_test_1"]["enabled"], false);

    // Import into fresh engine dir and verify
    let engine2 = tempfile::tempdir().unwrap();
    let engine2_dir = engine2.path();
    let plugins_dir2 = engine2_dir.join("plugins");
    fs::create_dir_all(&plugins_dir2).unwrap();
    fs::write(plugins_dir2.join("b.js"), "// plugin").unwrap();
    fs::write(plugins_dir2.join("manifest.json"), serde_json::to_string_pretty(
        &serde_json::json!({
            "scripts": ["b.js"],
            "plugins": {
                "b.js": {
                    "scope": {"all": true},
                    "params": {"default": {"enabled": true}},
                    "descriptors": null
                }
            }
        })
    ).unwrap()).unwrap();

    import_aaocase_zip(&export_path, engine2_dir, None).unwrap();

    let gm_text = fs::read_to_string(plugins_dir2.join("manifest.json")).unwrap();
    let gm: serde_json::Value = serde_json::from_str(&gm_text).unwrap();
    assert_eq!(
        gm["plugins"]["b.js"]["params"]["by_collection"]["col_test_1"]["enabled"],
        false,
        "by_collection override should be merged on import"
    );
}

// =====================================================================
// VFS pointer export/import roundtrip tests
// =====================================================================

/// Export must include VFS pointer paths as real files (not pointer text).
/// When dedup creates a VFS pointer (charsStill → chars), the manifest only
/// records the target path. The export must scan for pointers and include both.
#[test]
fn test_export_includes_vfs_pointer_defaults() {
    let engine1 = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let gif_data = b"GIF89a fake sprite data for Apollo 3";

    // Create case with manifest referencing ONLY the talking sprite (dedup rewrote still → talking)
    let case_dir = engine1.path().join("case/99001");
    fs::create_dir_all(case_dir.join("assets")).unwrap();
    let manifest = CaseManifest {
        case_id: 99001,
        title: "VFS Export Test".to_string(),
        author: "Tester".to_string(),
        language: "en".to_string(),
        download_date: "2026-01-01".to_string(),
        format: "Def6".to_string(),
        sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 1, total_downloaded: 1, total_size_bytes: gif_data.len() as u64 },
        asset_map: {
            let mut m = HashMap::new();
            // Manifest only has the talking path (dedup rewrote still entry to point here)
            m.insert("https://aaonline.fr/persos/Apollo/3.gif".to_string(), "defaults/images/chars/Apollo/3.gif".to_string());
            m
        },
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    fs::write(case_dir.join("trial_info.json"), r#"{"id":99001}"#).unwrap();
    fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0],"profiles":[0],"evidence":[0],"places":[0]}"#).unwrap();

    // Create the REAL talking sprite
    let chars_dir = engine1.path().join("defaults/images/chars/Apollo");
    fs::create_dir_all(&chars_dir).unwrap();
    fs::write(chars_dir.join("3.gif"), gif_data).unwrap();

    // Create a VFS pointer for the still sprite (this is what dedup does)
    let still_dir = engine1.path().join("defaults/images/charsStill/Apollo");
    fs::create_dir_all(&still_dir).unwrap();
    crate::downloader::vfs::write_vfs_pointer(
        &still_dir.join("3.gif"),
        "defaults/images/chars/Apollo/3.gif",
    ).unwrap();

    // Export
    let zip_path = tmp.path().join("vfs_test.aaocase");
    export_aaocase(99001, engine1.path(), &zip_path, None, None, false).unwrap();

    // Verify ZIP contains BOTH the talking and still sprites as real data
    let file = fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();

    // Talking sprite (from manifest)
    let talking_data = {
        let mut entry = archive.by_name("defaults/images/chars/Apollo/3.gif").unwrap();
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buf).unwrap();
        buf
    };
    assert_eq!(talking_data, gif_data, "Talking sprite should be real GIF data");

    // Still sprite (from VFS pointer scan)
    let still_data = {
        let mut entry = archive.by_name("defaults/images/charsStill/Apollo/3.gif").unwrap();
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buf).unwrap();
        buf
    };
    assert_eq!(still_data, gif_data, "Still sprite should be real GIF data, not VFS pointer text");
    assert_eq!(talking_data, still_data, "Both sprites should have identical content");
}

/// Full roundtrip: export with VFS pointers → import into clean dir → both sprites exist.
#[test]
fn test_export_import_roundtrip_vfs_sprites() {
    let engine1 = tempfile::tempdir().unwrap();
    let engine2 = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let gif_data = b"GIF89a identical sprite for roundtrip test";

    // Setup: case + real talking sprite + VFS pointer for still sprite
    let case_dir = engine1.path().join("case/99002");
    fs::create_dir_all(case_dir.join("assets")).unwrap();
    let manifest = CaseManifest {
        case_id: 99002,
        title: "VFS Roundtrip".to_string(),
        author: "Tester".to_string(),
        language: "en".to_string(),
        download_date: "2026-01-01".to_string(),
        format: "Def6".to_string(),
        sequence: None,
        assets: AssetSummary { case_specific: 0, shared_defaults: 1, total_downloaded: 1, total_size_bytes: gif_data.len() as u64 },
        asset_map: {
            let mut m = HashMap::new();
            m.insert("https://aaonline.fr/persos/Juge/3.gif".to_string(), "defaults/images/chars/Juge/3.gif".to_string());
            m
        },
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    fs::write(case_dir.join("trial_info.json"), r#"{"id":99002}"#).unwrap();
    fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0],"profiles":[0],"evidence":[0],"places":[0]}"#).unwrap();

    // Real file + VFS pointer
    let chars_dir = engine1.path().join("defaults/images/chars/Juge");
    fs::create_dir_all(&chars_dir).unwrap();
    fs::write(chars_dir.join("3.gif"), gif_data).unwrap();

    let still_dir = engine1.path().join("defaults/images/charsStill/Juge");
    fs::create_dir_all(&still_dir).unwrap();
    crate::downloader::vfs::write_vfs_pointer(
        &still_dir.join("3.gif"),
        "defaults/images/chars/Juge/3.gif",
    ).unwrap();

    // Export → Import
    let zip_path = tmp.path().join("roundtrip_vfs.aaocase");
    export_aaocase(99002, engine1.path(), &zip_path, None, None, false).unwrap();
    let result = import_aaocase_zip(&zip_path, engine2.path(), None).unwrap();
    assert_eq!(result.manifest.case_id, 99002);

    // Both sprites should be resolvable after import.
    // The import's dedup may create VFS pointers for identical content — that's fine,
    // as long as resolve_path returns the real data for both paths.
    let talking = engine2.path().join("defaults/images/chars/Juge/3.gif");
    let still = engine2.path().join("defaults/images/charsStill/Juge/3.gif");
    assert!(talking.is_file(), "Talking sprite must exist after import");
    assert!(still.is_file(), "Still sprite must exist after import (as file or VFS pointer)");

    // Both must resolve to real GIF data (resolve_path follows VFS pointers)
    let talking_resolved = crate::downloader::vfs::resolve_path(&talking, engine2.path(), engine2.path());
    let still_resolved = crate::downloader::vfs::resolve_path(&still, engine2.path(), engine2.path());
    assert_eq!(fs::read(&talking_resolved).unwrap(), gif_data, "Talking must resolve to real GIF");
    assert_eq!(fs::read(&still_resolved).unwrap(), gif_data, "Still must resolve to real GIF");
}

/// Export must resolve VFS pointers in case-specific assets (case/*/assets/).
#[test]
fn test_export_resolves_case_asset_vfs_pointers() {
    let engine1 = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let real_data = b"the actual image content for deduped case asset";

    // Create case with a VFS pointer in assets/
    let case_dir = engine1.path().join("case/99003");
    fs::create_dir_all(case_dir.join("assets")).unwrap();

    // Real file at shared location
    let shared_dir = engine1.path().join("defaults/shared/abcd");
    fs::create_dir_all(&shared_dir).unwrap();
    fs::write(shared_dir.join("abcd1234.gif"), real_data).unwrap();

    // VFS pointer in case assets
    crate::downloader::vfs::write_vfs_pointer(
        &case_dir.join("assets/sprite.gif"),
        "defaults/shared/abcd/abcd1234.gif",
    ).unwrap();

    let manifest = CaseManifest {
        case_id: 99003,
        title: "Case VFS Test".to_string(),
        author: "Tester".to_string(),
        language: "en".to_string(),
        download_date: "2026-01-01".to_string(),
        format: "Def6".to_string(),
        sequence: None,
        assets: AssetSummary { case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: real_data.len() as u64 },
        asset_map: {
            let mut m = HashMap::new();
            m.insert("http://example.com/sprite.gif".to_string(), "assets/sprite.gif".to_string());
            m
        },
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    fs::write(case_dir.join("trial_info.json"), r#"{"id":99003}"#).unwrap();
    fs::write(case_dir.join("trial_data.json"), r#"{"frames":[0],"profiles":[0],"evidence":[0],"places":[0]}"#).unwrap();

    // Export
    let zip_path = tmp.path().join("case_vfs.aaocase");
    export_aaocase(99003, engine1.path(), &zip_path, None, None, false).unwrap();

    // Verify ZIP has real data, not pointer text
    let file = fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut entry = archive.by_name("assets/sprite.gif").unwrap();
    let mut data = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut data).unwrap();
    assert_eq!(data, real_data, "Exported case asset should be real data, not VFS pointer text");
    assert!(!String::from_utf8_lossy(&data).starts_with("AAO_VFS_ALIAS:"), "Must not be VFS pointer text");
}
