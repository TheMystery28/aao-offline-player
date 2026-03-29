use super::*;

#[test]
fn test_dedup_case_assets_removes_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with a known file
    let defaults_chars = data_dir.join("defaults").join("images").join("chars").join("Olga");
    fs::create_dir_all(&defaults_chars).unwrap();
    fs::write(defaults_chars.join("1.gif"), b"sprite bytes").unwrap();

    // Create case with assets/ containing identical file
    let case_dir = data_dir.join("case").join("99");
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("sprite-abc123.gif"), b"sprite bytes").unwrap();

    // Create manifest
    let mut asset_map = HashMap::new();
    asset_map.insert(
        "http://example.com/sprite.gif".to_string(),
        "assets/sprite-abc123.gif".to_string(),
    );
    let manifest = CaseManifest {
        case_id: 99,
        title: "Test".to_string(),
        author: "Author".to_string(),
        language: "en".to_string(),
        download_date: "2025-01-01".to_string(),
        format: "v6".to_string(),
        sequence: None,
        assets: AssetSummary {
            case_specific: 1,
            shared_defaults: 0,
            total_downloaded: 1,
            total_size_bytes: 12,
        },
        asset_map,
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Create trial_data.json with reference to the asset
    let trial_data = serde_json::json!({
        "profiles": [null, {
            "custom_sprites": [{
                "talking": "case/99/assets/sprite-abc123.gif",
                "still": "",
                "startup": ""
            }]
        }]
    });
    fs::write(
        case_dir.join("trial_data.json"),
        serde_json::to_string_pretty(&trial_data).unwrap(),
    ).unwrap();

    // Run dedup
    let (count, bytes) = dedup_case_assets(99, data_dir).unwrap();
    assert_eq!(count, 1, "Should dedup 1 file");
    assert_eq!(bytes, 12, "Should save 12 bytes");

    // Verify file deleted from assets/
    assert!(!assets_dir.join("sprite-abc123.gif").exists());

    // Verify manifest updated
    let updated = read_manifest(&case_dir).unwrap();
    assert_eq!(
        updated.asset_map["http://example.com/sprite.gif"],
        "defaults/images/chars/Olga/1.gif"
    );

    // Verify trial_data rewritten
    let td_str = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
    let td: Value = serde_json::from_str(&td_str).unwrap();
    assert_eq!(
        td["profiles"][1]["custom_sprites"][0]["talking"],
        "defaults/images/chars/Olga/1.gif"
    );
}

#[test]
fn test_dedup_case_assets_preserves_unique() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with a known file
    let defaults_dir = data_dir.join("defaults");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("other.gif"), b"other content").unwrap();

    // Create case with a UNIQUE asset (different content)
    let case_dir = data_dir.join("case").join("50");
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("unique-abc.gif"), b"unique content").unwrap();

    let mut asset_map = HashMap::new();
    asset_map.insert(
        "http://example.com/unique.gif".to_string(),
        "assets/unique-abc.gif".to_string(),
    );
    let manifest = CaseManifest {
        case_id: 50,
        title: "Test".to_string(),
        author: "Author".to_string(),
        language: "en".to_string(),
        download_date: "2025-01-01".to_string(),
        format: "v6".to_string(),
        sequence: None,
        assets: AssetSummary {
            case_specific: 1,
            shared_defaults: 0,
            total_downloaded: 1,
            total_size_bytes: 14,
        },
        asset_map,
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    let (count, bytes) = dedup_case_assets(50, data_dir).unwrap();
    assert_eq!(count, 0, "Unique asset should not be deduped");
    assert_eq!(bytes, 0);
    assert!(assets_dir.join("unique-abc.gif").exists(), "File should still exist");
}

#[test]
fn test_dedup_case_assets_no_defaults_dir() {
    let dir = tempfile::tempdir().unwrap();
    // No defaults/ dir exists
    let case_dir = dir.path().join("case").join("1");
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("file.gif"), b"data").unwrap();

    let (count, _) = dedup_case_assets(1, dir.path()).unwrap();
    assert_eq!(count, 0, "No defaults dir → no dedup");
}

#[test]
fn test_dedup_case_assets_no_assets_dir() {
    let dir = tempfile::tempdir().unwrap();
    let case_dir = dir.path().join("case").join("2");
    fs::create_dir_all(&case_dir).unwrap();
    // No assets/ dir

    let (count, _) = dedup_case_assets(2, dir.path()).unwrap();
    assert_eq!(count, 0, "No assets dir → no dedup");
}

#[test]
fn test_clear_unused_defaults_removes_only_unreferenced() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with 3 files: 2 used by a case, 1 unused
    let chars_dir = data_dir.join("defaults").join("images").join("chars").join("Olga");
    fs::create_dir_all(&chars_dir).unwrap();
    fs::write(chars_dir.join("1.gif"), b"used sprite").unwrap();
    fs::write(chars_dir.join("2.gif"), b"also used").unwrap();
    let unused_dir = data_dir.join("defaults").join("music");
    fs::create_dir_all(&unused_dir).unwrap();
    fs::write(unused_dir.join("old_track.mp3"), b"unused music file").unwrap();

    // Create a case whose manifest references only the 2 used sprites
    let case_dir = data_dir.join("case").join("10");
    fs::create_dir_all(&case_dir).unwrap();
    let mut asset_map = HashMap::new();
    asset_map.insert("http://a.com/1".into(), "defaults/images/chars/Olga/1.gif".into());
    asset_map.insert("http://a.com/2".into(), "defaults/images/chars/Olga/2.gif".into());
    let manifest = CaseManifest {
        case_id: 10,
        title: "Test".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(),
        sequence: None,
        assets: AssetSummary {
            case_specific: 0, shared_defaults: 2, total_downloaded: 2, total_size_bytes: 20,
        },
        asset_map,
        failed_assets: vec![], has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Run clear
    let (deleted, bytes) = clear_unused_defaults(data_dir).unwrap();
    assert_eq!(deleted, 1, "Should delete only the unused music file");
    assert_eq!(bytes, b"unused music file".len() as u64);

    // Verify used files still exist
    assert!(chars_dir.join("1.gif").exists(), "Used sprite should remain");
    assert!(chars_dir.join("2.gif").exists(), "Used sprite should remain");
    // Verify unused file is gone
    assert!(!unused_dir.join("old_track.mp3").exists(), "Unused file should be deleted");
}

#[test]
fn test_clear_unused_defaults_no_cases_clears_everything() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with files but NO cases
    let defaults_dir = data_dir.join("defaults").join("sounds");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("sound.mp3"), b"orphaned").unwrap();

    let (deleted, _) = clear_unused_defaults(data_dir).unwrap();
    assert_eq!(deleted, 1, "All files should be cleared when no cases reference them");
    assert!(!defaults_dir.join("sound.mp3").exists());
}

#[test]
fn test_clear_unused_defaults_no_defaults_dir() {
    let dir = tempfile::tempdir().unwrap();
    let (deleted, bytes) = clear_unused_defaults(dir.path()).unwrap();
    assert_eq!(deleted, 0);
    assert_eq!(bytes, 0);
}

#[test]
fn test_clear_unused_defaults_updates_index() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with 2 files, index them
    let defaults_dir = data_dir.join("defaults");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("used.gif"), b"used content").unwrap();
    fs::write(defaults_dir.join("unused.gif"), b"unused content").unwrap();

    {
        let index = DedupIndex::open(data_dir).unwrap();
        index.scan_and_register(data_dir, "defaults").unwrap();
    } // Drop index before clear_unused_defaults opens its own

    // Create a case that references only "used.gif"
    let case_dir = data_dir.join("case").join("8");
    fs::create_dir_all(&case_dir).unwrap();
    let mut asset_map = HashMap::new();
    asset_map.insert("http://x.com/u.gif".into(), "defaults/used.gif".into());
    let manifest = CaseManifest {
        case_id: 8,
        title: "Test".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
        assets: AssetSummary {
            case_specific: 0, shared_defaults: 1, total_downloaded: 1, total_size_bytes: 12,
        },
        asset_map, failed_assets: vec![], has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Clear unused
    let (deleted, _) = clear_unused_defaults(data_dir).unwrap();
    assert_eq!(deleted, 1, "Should delete 1 unused file");

    // Verify the used file still exists on disk
    assert!(defaults_dir.join("used.gif").exists(), "Used file should still exist on disk");

    // Verify the index was updated: unused entry should be gone
    let fresh_index = DedupIndex::open(data_dir).unwrap();
    let unused_hash = xxh3_64(b"unused content");
    assert!(
        fresh_index.find_by_hash(unused_hash, None).is_none(),
        "Unused entry should be removed from index after clear"
    );

    // Used entry should still be in the index
    let used_hash = xxh3_64(b"used content");
    assert!(
        fresh_index.find_by_hash(used_hash, None).is_some(),
        "Used entry should remain in index after clear"
    );
}

#[test]
fn test_export_after_dedup_includes_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with a known file
    let defaults_chars = data_dir.join("defaults").join("images").join("chars").join("Olga");
    fs::create_dir_all(&defaults_chars).unwrap();
    fs::write(defaults_chars.join("1.gif"), b"olga sprite content").unwrap();

    // Create case with identical asset in assets/
    let case_dir = data_dir.join("case").join("77");
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("sprite-olga.gif"), b"olga sprite content").unwrap();

    // Create manifest and trial_data
    let mut asset_map = HashMap::new();
    asset_map.insert(
        "http://example.com/olga.gif".to_string(),
        "assets/sprite-olga.gif".to_string(),
    );
    let manifest = CaseManifest {
        case_id: 77,
        title: "Export Dedup Test".to_string(),
        author: "Author".to_string(),
        language: "en".to_string(),
        download_date: "2025-01-01".to_string(),
        format: "v6".to_string(),
        sequence: None,
        assets: AssetSummary {
            case_specific: 1, shared_defaults: 0,
            total_downloaded: 1, total_size_bytes: 19,
        },
        asset_map,
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    let trial_data = serde_json::json!({
        "profiles": [null, {
            "custom_sprites": [{
                "talking": "case/77/assets/sprite-olga.gif",
                "still": "", "startup": ""
            }]
        }]
    });
    fs::write(
        case_dir.join("trial_data.json"),
        serde_json::to_string_pretty(&trial_data).unwrap(),
    ).unwrap();

    // Run dedup — asset should be deduped to default path
    let (count, _) = dedup_case_assets(77, data_dir).unwrap();
    assert_eq!(count, 1, "Should dedup 1 file");
    assert!(!assets_dir.join("sprite-olga.gif").exists(), "Original should be deleted");

    // Verify manifest points to defaults/
    let updated_manifest = read_manifest(&case_dir).unwrap();
    let path = &updated_manifest.asset_map["http://example.com/olga.gif"];
    assert!(path.starts_with("defaults/"), "Manifest should point to defaults/, got: {}", path);

    // Export the case
    let export_path = dir.path().join("test.aaocase");
    crate::importer::export_aaocase(77, data_dir, &export_path, None, None, true).unwrap();
    assert!(export_path.exists(), "ZIP should exist");

    // Verify the ZIP contains the defaults/ file
    let file = fs::File::open(&export_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut found_default = false;
    let mut found_manifest = false;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).unwrap();
        let name = entry.name().to_string();
        if name.contains("defaults/images/chars/Olga/1.gif") {
            found_default = true;
        }
        if name == "manifest.json" {
            found_manifest = true;
        }
    }
    assert!(found_default, "ZIP should contain the defaults/ sprite file");
    assert!(found_manifest, "ZIP should contain manifest.json");

    // Verify the exported manifest has the correct path
    let manifest_text = {
        let mut entry = archive.by_name("manifest.json").unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut entry, &mut s).unwrap();
        s
    };
    let exported_manifest: CaseManifest =
        serde_json::from_str(&manifest_text).unwrap();
    let exported_path = &exported_manifest.asset_map["http://example.com/olga.gif"];
    assert!(
        exported_path.starts_with("defaults/"),
        "Exported manifest should point to defaults/, got: {}",
        exported_path
    );
}

#[test]
fn test_dedup_promotes_cross_case_to_shared() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create two cases with identical custom assets (no defaults/ involved)
    let content = b"identical custom image data";
    make_case_with_asset(data_dir, 100, "photo.png", content);
    make_case_with_asset(data_dir, 200, "photo.png", content);

    // Run dedup on case 200 — it should find case 100 has the same file
    let (count, _) = dedup_case_assets(200, data_dir).unwrap();
    assert_eq!(count, 1, "Should dedup 1 file");

    // The file should be promoted to defaults/shared/
    let manifest_200 = read_manifest(&data_dir.join("case/200")).unwrap();
    let path_200 = &manifest_200.asset_map["http://example.com/photo.png"];
    assert!(
        path_200.starts_with("defaults/shared/"),
        "Case 200 should point to shared, got: {}", path_200
    );

    // Case 100 should also be rewritten to point to the shared copy
    let manifest_100 = read_manifest(&data_dir.join("case/100")).unwrap();
    let path_100 = &manifest_100.asset_map["http://example.com/photo.png"];
    assert!(
        path_100.starts_with("defaults/shared/"),
        "Case 100 should also point to shared, got: {}", path_100
    );

    // Both should point to the same shared path
    assert_eq!(path_200, path_100, "Both cases should point to same shared path");

    // Original case files should be deleted
    assert!(!data_dir.join("case/200/assets/photo.png").exists());
    assert!(!data_dir.join("case/100/assets/photo.png").exists());

    // Shared file should exist
    assert!(data_dir.join(path_200).exists(), "Shared file should exist on disk");
}

#[test]
fn test_dedup_prefers_existing_defaults_over_case() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create a defaults/ file
    let content = b"shared sprite content";
    let defaults_dir = data_dir.join("defaults/images/chars/Test");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("1.gif"), content).unwrap();

    // Create a case with the same content
    make_case_with_asset(data_dir, 300, "sprite.gif", content);

    // Run dedup — should prefer defaults/ over promoting to shared
    let (count, _) = dedup_case_assets(300, data_dir).unwrap();
    assert_eq!(count, 1, "Should dedup 1 file");

    let manifest = read_manifest(&data_dir.join("case/300")).unwrap();
    let path = &manifest.asset_map["http://example.com/sprite.gif"];
    assert!(
        path.starts_with("defaults/images/"),
        "Should point to existing defaults/, not shared. Got: {}", path
    );
}

#[test]
fn test_find_by_hash_prefers_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    let hash: u64 = 12345;
    let size: u64 = 100;
    index.register("case/1/assets/file.png", size, hash).unwrap();
    index.register("defaults/images/bg/room.png", size, hash).unwrap();

    let result = index.find_by_hash(hash, None);
    assert_eq!(result, Some("defaults/images/bg/room.png".to_string()));
}

#[test]
fn test_find_by_hash_excludes_self() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    let hash: u64 = 67890;
    let size: u64 = 50;
    index.register("case/1/assets/only.png", size, hash).unwrap();

    // With exclude = self, should return None
    let result = index.find_by_hash(hash, Some("case/1/assets/only.png"));
    assert_eq!(result, None, "Should exclude self-match");

    // Without exclude, should find it
    let result = index.find_by_hash(hash, None);
    assert_eq!(result, Some("case/1/assets/only.png".to_string()));
}

#[test]
fn test_promote_to_shared_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create a source file
    let content = b"file to promote";
    let source = data_dir.join("source.png");
    fs::write(&source, content).unwrap();
    let hash = xxh3_64(content);

    let index = DedupIndex::open(data_dir).unwrap();

    // First promotion
    let path1 = promote_to_shared(data_dir, &source, hash, &index).unwrap();
    assert!(path1.starts_with("defaults/shared/"));
    assert!(data_dir.join(&path1).exists());

    // Second promotion — should return same path, no error
    let path2 = promote_to_shared(data_dir, &source, hash, &index).unwrap();
    assert_eq!(path1, path2, "Idempotent: same path returned");
}

#[test]
fn test_dedup_manifest_trial_data_consistency() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with a known file
    let defaults_dir = data_dir.join("defaults/images/chars/Test");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("1.gif"), b"sprite bytes here").unwrap();

    // Create case with matching asset
    make_case_with_asset(data_dir, 50, "sprite.gif", b"sprite bytes here");

    // Run dedup — should replace case asset with defaults path
    let (count, _) = dedup_case_assets(50, data_dir).unwrap();
    assert!(count > 0, "Should dedup at least 1 file");

    // Verify manifest and trial_data agree
    let case_dir = data_dir.join("case/50");
    assert_manifest_trial_data_agree(&case_dir, 50);
}

#[test]
fn test_cross_case_dedup_manifest_trial_data_consistency() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create two cases with identical custom assets
    let content = b"shared custom image data";
    make_case_with_asset(data_dir, 600, "photo.png", content);
    make_case_with_asset(data_dir, 700, "photo.png", content);

    // Dedup case 700 — promotes to shared, rewrites case 600 too
    let (count, _) = dedup_case_assets(700, data_dir).unwrap();
    assert!(count > 0, "Should dedup at least 1 file");

    // Both cases must have consistent manifest + trial_data
    assert_manifest_trial_data_agree(&data_dir.join("case/700"), 700);
    assert_manifest_trial_data_agree(&data_dir.join("case/600"), 600);
}
