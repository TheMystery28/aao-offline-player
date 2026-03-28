use super::*;

#[test]
fn test_dedup_index_scan_register_and_find() {
    let dir = tempfile::tempdir().unwrap();

    // Create defaults/ with a known file
    let defaults_dir = dir.path().join("defaults").join("images");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("sprite.gif"), b"sprite content").unwrap();

    // Open index and scan
    let index = DedupIndex::open(dir.path()).unwrap();
    let count = index.scan_and_register(dir.path(), "defaults").unwrap();
    assert_eq!(count, 1, "Should register 1 file");

    // Create a candidate file with same content
    let candidate = dir.path().join("candidate.gif");
    fs::write(&candidate, b"sprite content").unwrap();
    let result = index.find_duplicate(&candidate, dir.path());
    assert!(result.is_some(), "Should find duplicate");
    assert!(
        result.unwrap().contains("sprite.gif"),
        "Should return the defaults path"
    );

    // Create a candidate with different content
    let different = dir.path().join("different.gif");
    fs::write(&different, b"different content here").unwrap();
    let result = index.find_duplicate(&different, dir.path());
    assert!(result.is_none(), "Different content should not match");
}

#[test]
fn test_dedup_index_size_mismatch_skips_hash() {
    let dir = tempfile::tempdir().unwrap();

    // Create defaults/ with a known file
    let defaults_dir = dir.path().join("defaults");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("small.gif"), b"small").unwrap();

    let index = DedupIndex::open(dir.path()).unwrap();
    index.scan_and_register(dir.path(), "defaults").unwrap();

    // Different size, same extension → no match
    let candidate = dir.path().join("candidate.gif");
    fs::write(&candidate, b"this is a much larger file with different size").unwrap();
    let result = index.find_duplicate(&candidate, dir.path());
    assert!(
        result.is_none(),
        "Different file size should not match even with same extension"
    );
}

#[test]
fn test_dedup_index_register_and_find() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    // Register a file and create it on disk (find_duplicate verifies existence)
    let content = b"test content";
    let hash = xxh3_64(content);
    let defaults_dir = dir.path().join("defaults").join("images");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("test.gif"), content).unwrap();
    index.register("defaults/images/test.gif", 12, hash).unwrap();

    // Create a candidate with same content
    let candidate = dir.path().join("candidate.gif");
    fs::write(&candidate, content).unwrap();
    let result = index.find_duplicate(&candidate, dir.path());
    assert!(result.is_some(), "Should find registered duplicate");
    assert_eq!(result.unwrap(), "defaults/images/test.gif");

    // Different content → no match
    let different = dir.path().join("different.gif");
    fs::write(&different, b"other content!").unwrap();
    let result = index.find_duplicate(&different, dir.path());
    assert!(result.is_none(), "Different content should not match");
}

#[test]
fn test_dedup_index_unregister() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    let content = b"removable";
    let hash = xxh3_64(content);
    let sounds_dir = dir.path().join("defaults").join("sounds");
    fs::create_dir_all(&sounds_dir).unwrap();
    fs::write(sounds_dir.join("test.mp3"), content).unwrap();
    index.register("defaults/sounds/test.mp3", 9, hash).unwrap();

    // Verify it's findable
    let candidate = dir.path().join("candidate.mp3");
    fs::write(&candidate, content).unwrap();
    assert!(index.find_duplicate(&candidate, dir.path()).is_some());

    // Unregister
    index.unregister("defaults/sounds/test.mp3").unwrap();

    // No longer findable
    assert!(index.find_duplicate(&candidate, dir.path()).is_none());
}

#[test]
fn test_dedup_index_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let content = b"persistent data";

    // Create the file on disk
    let music_dir = dir.path().join("defaults").join("music");
    fs::create_dir_all(&music_dir).unwrap();
    fs::write(music_dir.join("song.mp3"), content).unwrap();

    // Register in one instance
    {
        let index = DedupIndex::open(dir.path()).unwrap();
        let hash = xxh3_64(content);
        index.register("defaults/music/song.mp3", 15, hash).unwrap();
    }

    // Re-open from same path — entries should survive
    {
        let index = DedupIndex::open(dir.path()).unwrap();
        let candidate = dir.path().join("candidate.mp3");
        fs::write(&candidate, content).unwrap();
        let result = index.find_duplicate(&candidate, dir.path());
        assert!(result.is_some(), "Entries should persist across open/close");
        assert_eq!(result.unwrap(), "defaults/music/song.mp3");
    }
}

#[test]
fn test_dedup_index_populated_after_dedup() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with a file
    let defaults_dir = data_dir.join("defaults").join("images");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("sprite.gif"), b"sprite data").unwrap();

    // Create a case with assets/ (no overlap, just to trigger dedup to run scan_and_register)
    let case_dir = data_dir.join("case").join("42");
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("unique.gif"), b"unique data").unwrap();

    let mut asset_map = HashMap::new();
    asset_map.insert("http://example.com/unique.gif".into(), "assets/unique.gif".into());
    let manifest = CaseManifest {
        case_id: 42,
        title: "Test".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(),
        sequence: None,
        assets: AssetSummary {
            case_specific: 1, shared_defaults: 0,
            total_downloaded: 1, total_size_bytes: 11,
        },
        asset_map,
        failed_assets: vec![], has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Run dedup — this calls scan_and_register internally
    let _ = dedup_case_assets(42, data_dir).unwrap();

    // Open a FRESH index and verify the defaults/ file was registered
    let fresh_index = DedupIndex::open(data_dir).unwrap();
    let candidate = dir.path().join("candidate.gif");
    fs::write(&candidate, b"sprite data").unwrap();
    let result = fresh_index.find_duplicate(&candidate, data_dir);
    assert!(result.is_some(), "Index should contain the defaults/ file after dedup ran");
    assert!(
        result.unwrap().contains("sprite.gif"),
        "Should find the defaults/images/sprite.gif entry"
    );
}

#[test]
fn test_scan_and_register_cases() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create case asset files
    let a_dir = data_dir.join("case").join("10").join("assets");
    fs::create_dir_all(&a_dir).unwrap();
    fs::write(a_dir.join("a.gif"), b"content a").unwrap();

    let b_dir = data_dir.join("case").join("20").join("assets");
    fs::create_dir_all(&b_dir).unwrap();
    fs::write(b_dir.join("b.gif"), b"content b").unwrap();

    let index = DedupIndex::open(data_dir).unwrap();
    let count = index.scan_and_register_cases(data_dir).unwrap();
    assert_eq!(count, 2, "Should register 2 case asset files");

    // Verify idempotent
    let count2 = index.scan_and_register_cases(data_dir).unwrap();
    assert_eq!(count2, 0, "Second scan should register 0 (already indexed)");

    // Verify findable
    let candidate = dir.path().join("match.gif");
    fs::write(&candidate, b"content a").unwrap();
    let result = index.find_duplicate(&candidate, data_dir);
    assert!(result.is_some(), "Should find case asset duplicate");
    assert!(result.unwrap().contains("case/10/assets/a.gif"));
}

#[test]
fn test_unregister_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    // Create files on disk and register entries under case/99/ and case/100/
    let d99 = dir.path().join("case/99/assets");
    fs::create_dir_all(&d99).unwrap();
    fs::write(d99.join("a.gif"), b"data1").unwrap();
    fs::write(d99.join("b.gif"), b"data2").unwrap();
    let d100 = dir.path().join("case/100/assets");
    fs::create_dir_all(&d100).unwrap();
    fs::write(d100.join("c.gif"), b"data3").unwrap();

    let h1 = xxh3_64(b"data1");
    let h2 = xxh3_64(b"data2");
    let h3 = xxh3_64(b"data3");
    index.register("case/99/assets/a.gif", 5, h1).unwrap();
    index.register("case/99/assets/b.gif", 5, h2).unwrap();
    index.register("case/100/assets/c.gif", 5, h3).unwrap();

    // Unregister case/99/
    let removed = index.unregister_prefix("case/99/").unwrap();
    assert_eq!(removed, 2, "Should remove 2 entries under case/99/");

    // Verify case/99/ entries are gone
    let candidate99 = dir.path().join("match99.gif");
    fs::write(&candidate99, b"data1").unwrap();
    assert!(index.find_duplicate(&candidate99, dir.path()).is_none(),
        "case/99 entries should be gone");

    // Verify case/100/ entries are still present
    let candidate100 = dir.path().join("match100.gif");
    fs::write(&candidate100, b"data3").unwrap();
    let result = index.find_duplicate(&candidate100, dir.path());
    assert!(result.is_some(), "case/100 entries should still exist");
    assert!(result.unwrap().contains("case/100/"));
}

#[test]
fn test_register_case_asset_and_find() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    let content = b"case sprite data";
    let hash = xxh3_64(content);
    let case_dir = dir.path().join("case").join("99").join("assets");
    fs::create_dir_all(&case_dir).unwrap();
    fs::write(case_dir.join("sprite.gif"), content).unwrap();
    index.register("case/99/assets/sprite.gif", 16, hash).unwrap();

    // Matching file
    let candidate = dir.path().join("match.gif");
    fs::write(&candidate, content).unwrap();
    let result = index.find_duplicate(&candidate, dir.path());
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "case/99/assets/sprite.gif");

    // Non-matching
    let diff = dir.path().join("diff.gif");
    fs::write(&diff, b"different data!!").unwrap();
    assert!(index.find_duplicate(&diff, dir.path()).is_none());
}

#[test]
fn test_dedup_index_corrupt_db_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("dedup_index.redb");

    // Write garbage to the db file
    fs::write(&db_path, b"this is not a valid redb file").unwrap();

    // open() should recover by deleting and recreating
    let index = DedupIndex::open(dir.path());
    assert!(index.is_ok(), "Should recover from corrupt db");

    // Should work normally after recovery
    let index = index.unwrap();
    let content = b"test";
    let hash = xxh3_64(content);
    let defaults_dir = dir.path().join("defaults");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("test.gif"), content).unwrap();
    index.register("defaults/test.gif", 4, hash).unwrap();
    let candidate = dir.path().join("test.gif");
    fs::write(&candidate, content).unwrap();
    assert!(index.find_duplicate(&candidate, dir.path()).is_some());
}

#[test]
fn test_dedup_stale_index_entry_file_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with a file and index it
    let defaults_dir = data_dir.join("defaults");
    fs::create_dir_all(&defaults_dir).unwrap();
    let file_path = defaults_dir.join("sprite.gif");
    fs::write(&file_path, b"sprite data").unwrap();

    let index = DedupIndex::open(data_dir).unwrap();
    index.scan_and_register(data_dir, "defaults").unwrap();

    // Now delete the file from disk (stale entry in index)
    fs::remove_file(&file_path).unwrap();

    // dedup_case_assets should handle this gracefully:
    // find_duplicate may return a match but dedup checks disk before deleting
    let case_dir = data_dir.join("case").join("5");
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("sprite-x.gif"), b"sprite data").unwrap();

    let mut asset_map = HashMap::new();
    asset_map.insert("http://x.com/s.gif".into(), "assets/sprite-x.gif".into());
    let manifest = CaseManifest {
        case_id: 5,
        title: "Test".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
        assets: AssetSummary {
            case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 11,
        },
        asset_map, failed_assets: vec![], has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    // Should NOT dedup because the default file doesn't exist on disk
    let (count, _) = dedup_case_assets(5, data_dir).unwrap();
    assert_eq!(count, 0, "Should not dedup against stale index entry (file missing on disk)");
    assert!(assets_dir.join("sprite-x.gif").exists(), "Case file should still exist");
}

#[test]
fn test_dedup_index_scan_skips_existing() {
    let dir = tempfile::tempdir().unwrap();

    // Create a file in defaults/
    let defaults_dir = dir.path().join("defaults");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("file.gif"), b"content").unwrap();

    let index = DedupIndex::open(dir.path()).unwrap();

    // First scan registers 1 file
    let count1 = index.scan_and_register(dir.path(), "defaults").unwrap();
    assert_eq!(count1, 1);

    // Second scan skips it (already in db)
    let count2 = index.scan_and_register(dir.path(), "defaults").unwrap();
    assert_eq!(count2, 0, "Should not re-register existing files");
}

#[test]
fn test_unregister_prefix_no_matches() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    let content = b"data";
    let hash = xxh3_64(content);
    let defaults_dir = dir.path().join("defaults");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("test.gif"), content).unwrap();
    index.register("defaults/test.gif", 4, hash).unwrap();

    // Unregister a prefix that doesn't exist
    let removed = index.unregister_prefix("case/999/").unwrap();
    assert_eq!(removed, 0, "No entries to remove");

    // Original entry should still be there
    let candidate = dir.path().join("test.gif");
    fs::write(&candidate, content).unwrap();
    assert!(index.find_duplicate(&candidate, dir.path()).is_some());
}

#[test]
fn test_query_case_assets_empty_index() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();
    let assets = index.query_case_assets().unwrap();
    assert!(assets.is_empty(), "Empty index should return empty vec");
}

#[test]
fn test_query_case_assets_ignores_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    let h1 = xxh3_64(b"default");
    let h2 = xxh3_64(b"case");
    index.register("defaults/images/sprite.gif", 7, h1).unwrap();
    index.register("case/1/assets/custom.gif", 4, h2).unwrap();

    let assets = index.query_case_assets().unwrap();
    assert_eq!(assets.len(), 1, "Should only return case assets, not defaults");
    assert_eq!(assets[0].0, 1); // case_id
    assert_eq!(assets[0].1, "custom.gif"); // filename
}

#[test]
fn test_register_normalizes_path() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    // Register with backslash (the key should be normalized to forward slashes)
    let content = b"test content for normalization";
    let hash = xxh3_64(content);
    index.register("defaults\\music\\song.mp3", content.len() as u64, hash).unwrap();

    // Create a candidate with matching content
    let candidate = dir.path().join("match.mp3");
    fs::write(&candidate, content).unwrap();

    // Create the default file on disk so find_duplicate's existence check passes
    let default_dir = dir.path().join("defaults").join("music");
    fs::create_dir_all(&default_dir).unwrap();
    fs::write(default_dir.join("song.mp3"), content).unwrap();

    // find_duplicate should find the match (register normalized backslash to forward slash)
    let result = index.find_duplicate(&candidate, dir.path());
    assert!(result.is_some(), "Should find match despite backslash in register path");
    let found = result.unwrap();
    assert_eq!(found, "defaults/music/song.mp3", "Key should be forward-slashed");
}

#[test]
fn test_find_duplicate_skips_deleted_file() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create a defaults/ file and index it
    let defaults_dir = data_dir.join("defaults").join("images");
    fs::create_dir_all(&defaults_dir).unwrap();
    let file_path = defaults_dir.join("sprite.gif");
    let content = b"sprite bytes for deletion test";
    fs::write(&file_path, content).unwrap();

    let index = DedupIndex::open(data_dir).unwrap();
    let hash = xxh3_64(content);
    index.register("defaults/images/sprite.gif", content.len() as u64, hash).unwrap();

    // Verify it's findable while file exists
    let candidate = dir.path().join("match.gif");
    fs::write(&candidate, content).unwrap();
    assert!(index.find_duplicate(&candidate, data_dir).is_some(), "Should find while file exists");

    // Delete the file from disk (stale entry)
    fs::remove_file(&file_path).unwrap();

    // find_duplicate should now return None (file doesn't exist on disk)
    let result = index.find_duplicate(&candidate, data_dir);
    assert!(result.is_none(), "Should return None for stale entry (file deleted from disk)");
}

#[test]
fn test_dedup_case_assets_no_trial_data() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create defaults/ with a known file
    let defaults_dir = data_dir.join("defaults").join("images");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("sprite.gif"), b"match content").unwrap();

    // Create case with manifest + assets but NO trial_data.json
    let case_dir = data_dir.join("case").join("33");
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("sprite-abc.gif"), b"match content").unwrap();

    let mut asset_map = HashMap::new();
    asset_map.insert("http://x.com/s.gif".into(), "assets/sprite-abc.gif".into());
    let manifest = CaseManifest {
        case_id: 33,
        title: "No Trial Data".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
        assets: AssetSummary {
            case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 13,
        },
        asset_map, failed_assets: vec![], has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    // Intentionally NO trial_data.json

    let (count, bytes) = dedup_case_assets(33, data_dir).unwrap();
    assert_eq!(count, 1, "Should dedup even without trial_data.json");
    assert_eq!(bytes, 13);
    assert!(!assets_dir.join("sprite-abc.gif").exists());

    // Verify manifest updated
    let updated = read_manifest(&case_dir).unwrap();
    assert!(updated.asset_map["http://x.com/s.gif"].starts_with("defaults/"));
}
