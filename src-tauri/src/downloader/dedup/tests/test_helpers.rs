use super::*;

#[test]
fn test_normalize_ext() {
    assert_eq!(normalize_ext("GIF"), "gif");
    assert_eq!(normalize_ext("jpeg"), "jpg");
    assert_eq!(normalize_ext("JPEG"), "jpg");
    assert_eq!(normalize_ext("PNG"), "png");
    assert_eq!(normalize_ext("htm"), "html");
    assert_eq!(normalize_ext("HTM"), "html");
    assert_eq!(normalize_ext("tiff"), "tif");
    assert_eq!(normalize_ext("TIFF"), "tif");
    assert_eq!(normalize_ext("mp3"), "mp3");
    assert_eq!(normalize_ext("jpg"), "jpg");
}

#[test]
fn test_hash_file_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let file_a = dir.path().join("a.bin");
    let file_b = dir.path().join("b.bin");
    let file_c = dir.path().join("c.bin");

    fs::write(&file_a, b"hello world").unwrap();
    fs::write(&file_b, b"hello world").unwrap(); // same content
    fs::write(&file_c, b"different content").unwrap();

    let hash_a = hash_file(&file_a).unwrap();
    let hash_b = hash_file(&file_b).unwrap();
    let hash_c = hash_file(&file_c).unwrap();

    assert_eq!(hash_a, hash_b, "Same content should produce same hash");
    assert_ne!(hash_a, hash_c, "Different content should produce different hash");
}

#[test]
fn test_rewrite_value_recursive() {
    let mut data = serde_json::json!({
        "profiles": [null, {
            "custom_sprites": [{
                "talking": "case/99/assets/sprite-abc.gif",
                "still": "case/99/assets/sprite-abc.gif",
                "startup": ""
            }]
        }],
        "nested": {
            "deep": "case/99/assets/sprite-abc.gif"
        },
        "unrelated": "keep this"
    });

    rewrite_value_recursive(
        &mut data,
        "case/99/assets/sprite-abc.gif",
        "defaults/images/chars/Olga/1.gif",
    );

    assert_eq!(
        data["profiles"][1]["custom_sprites"][0]["talking"],
        "defaults/images/chars/Olga/1.gif"
    );
    assert_eq!(
        data["profiles"][1]["custom_sprites"][0]["still"],
        "defaults/images/chars/Olga/1.gif"
    );
    assert_eq!(
        data["profiles"][1]["custom_sprites"][0]["startup"],
        ""
    );
    assert_eq!(data["nested"]["deep"], "defaults/images/chars/Olga/1.gif");
    assert_eq!(data["unrelated"], "keep this");
}

#[test]
fn test_content_hash_deterministic() {
    // xxh3_64 is deterministic — same input always produces same hash
    let content = b"known test content for hash verification";
    let hash1 = xxh3_64(content);
    let hash2 = xxh3_64(content);
    assert_eq!(hash1, hash2, "Same content must produce same hash");
    assert_ne!(hash1, 0, "Hash should not be zero for non-empty content");

    // Different content → different hash
    let other = b"different content";
    let hash3 = xxh3_64(other);
    assert_ne!(hash1, hash3, "Different content should produce different hash");
}

#[test]
fn test_normalize_ext_empty_and_edge_cases() {
    assert_eq!(normalize_ext(""), "");
    assert_eq!(normalize_ext("JPEG"), "jpg");
    assert_eq!(normalize_ext("MP3"), "mp3");
    assert_eq!(normalize_ext("Gif"), "gif");
    assert_eq!(normalize_ext("HTML"), "html");
    assert_eq!(normalize_ext("htm"), "html");
    assert_eq!(normalize_ext("TIFF"), "tif");
    assert_eq!(normalize_ext("ogg"), "ogg");
    assert_eq!(normalize_ext("WAV"), "wav");
}

#[test]
fn test_dedup_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // Create a 0-byte default and a 0-byte case asset
    let defaults_dir = data_dir.join("defaults").join("images");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(defaults_dir.join("empty.gif"), b"").unwrap();

    let case_dir = data_dir.join("case").join("1");
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("empty-abc.gif"), b"").unwrap();

    let mut asset_map = HashMap::new();
    asset_map.insert("http://x.com/e.gif".into(), "assets/empty-abc.gif".into());
    let manifest = CaseManifest {
        case_id: 1,
        title: "Empty".into(), author: "A".into(), language: "en".into(),
        download_date: "2025-01-01".into(), format: "v6".into(), sequence: None,
        assets: AssetSummary {
            case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 0,
        },
        asset_map, failed_assets: vec![], has_plugins: false, has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    let (count, _) = dedup_case_assets(1, data_dir).unwrap();
    assert_eq!(count, 1, "Empty files with same hash should dedup");
    assert!(!assets_dir.join("empty-abc.gif").exists(), "Empty case file should be deleted");
}

#[test]
fn test_dedup_same_content_different_extension_matches() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    // Register a .gif file
    let hash = xxh3_64(b"five!");
    index.register("defaults/images/sprite.gif", 5, hash).unwrap();

    // With hash-keyed lookup, same content matches regardless of extension
    let result = index.find_by_hash(hash, None);
    assert!(result.is_some(), "Same content should match via hash lookup");
}

#[test]
fn test_dedup_different_content_no_match() {
    let dir = tempfile::tempdir().unwrap();
    let index = DedupIndex::open(dir.path()).unwrap();

    // Register a file
    let hash_a = xxh3_64(b"AAAAA");
    index.register("defaults/images/a.gif", 5, hash_a).unwrap();

    // Different content → different hash → no match
    let hash_b = xxh3_64(b"BBBBB");
    let result = index.find_by_hash(hash_b, None);
    assert!(result.is_none(), "Different content should not match");
}
