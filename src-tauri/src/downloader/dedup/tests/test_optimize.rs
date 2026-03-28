use super::*;

#[test]
fn test_optimize_all_cases_promotes_shared() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();
    let content = b"shared sprite data for testing";

    // Two cases with identical assets (different filenames)
    make_case_with_asset(data_dir, 100, "bg-aaa.jpg", content);
    make_case_with_asset(data_dir, 200, "bg-bbb.jpg", content);

    let (count, bytes) = optimize_all_cases(data_dir, None).unwrap();
    assert!(count >= 2, "Should dedup at least 2 files, got {}", count);
    // Net savings: deleted 2 case copies, created 1 shared copy → net = 1x file size
    assert_eq!(bytes, content.len() as u64, "Net savings should be 1x file size (2 deleted - 1 created)");

    // Verify shared file exists in defaults/shared/
    let shared_dir = data_dir.join("defaults").join("shared");
    assert!(shared_dir.is_dir(), "defaults/shared/ should exist");
    let shared_files: Vec<_> = fs::read_dir(&shared_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();
    assert_eq!(shared_files.len(), 1, "Should have exactly 1 shared file");

    // Verify original assets/ files deleted
    assert!(!data_dir.join("case/100/assets/bg-aaa.jpg").exists());
    assert!(!data_dir.join("case/200/assets/bg-bbb.jpg").exists());

    // Verify manifests updated to shared path
    let m100 = read_manifest(&data_dir.join("case/100")).unwrap();
    let path100 = &m100.asset_map["http://example.com/bg-aaa.jpg"];
    assert!(path100.starts_with("defaults/shared/"), "Manifest should point to shared, got: {}", path100);

    let m200 = read_manifest(&data_dir.join("case/200")).unwrap();
    let path200 = &m200.asset_map["http://example.com/bg-bbb.jpg"];
    assert!(path200.starts_with("defaults/shared/"), "Manifest should point to shared, got: {}", path200);
}

#[test]
fn test_optimize_all_cases_skips_singletons() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    // One case with unique asset (no duplicate anywhere)
    make_case_with_asset(data_dir, 300, "unique-xyz.gif", b"unique content");

    let (count, _) = optimize_all_cases(data_dir, None).unwrap();
    assert_eq!(count, 0, "Singleton should not be promoted or deduped");
    assert!(data_dir.join("case/300/assets/unique-xyz.gif").exists(), "File should still exist");
}

#[test]
fn test_optimize_all_cases_empty_no_crash() {
    let dir = tempfile::tempdir().unwrap();
    // No case/ dir at all
    let (count, bytes) = optimize_all_cases(dir.path(), None).unwrap();
    assert_eq!(count, 0);
    assert_eq!(bytes, 0);
}

#[test]
fn test_optimize_all_cases_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();

    make_case_with_asset(data_dir, 400, "sprite-a.gif", b"identical content");
    make_case_with_asset(data_dir, 500, "sprite-b.gif", b"identical content");

    let (count1, _) = optimize_all_cases(data_dir, None).unwrap();
    assert!(count1 >= 2);

    // Run again — should do nothing
    let (count2, bytes2) = optimize_all_cases(data_dir, None).unwrap();
    assert_eq!(count2, 0, "Second run should find nothing to dedup");
    assert_eq!(bytes2, 0);
}

#[test]
fn test_optimize_reads_from_index() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();
    let content = b"shared asset across cases";

    // Create two cases with identical assets
    make_case_with_asset(data_dir, 600, "img-aaa.jpg", content);
    make_case_with_asset(data_dir, 700, "img-bbb.jpg", content);

    // Populate the index from disk (migration path)
    let index = DedupIndex::open(data_dir).unwrap();
    let scan_count = index.scan_and_register_cases(data_dir).unwrap();
    assert_eq!(scan_count, 2, "Should index 2 case assets");

    // Verify query_case_assets returns them
    let assets = index.query_case_assets().unwrap();
    assert_eq!(assets.len(), 2, "Should have 2 entries in index");

    // Run optimize — should read from index, not disk
    let (count, _) = optimize_all_cases(data_dir, None).unwrap();
    assert!(count >= 2, "Should dedup at least 2 files, got {}", count);

    // Verify shared file created
    let shared_dir = data_dir.join("defaults").join("shared");
    assert!(shared_dir.is_dir(), "defaults/shared/ should exist");

    // Verify case assets deleted
    assert!(!data_dir.join("case/600/assets/img-aaa.jpg").exists());
    assert!(!data_dir.join("case/700/assets/img-bbb.jpg").exists());
}

#[test]
fn test_optimize_multiple_cases_share_same_promoted_default() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path();
    let content = b"widely shared background image";

    // Create 3 cases with identical asset
    make_case_with_asset(data_dir, 800, "bg-aaa.jpg", content);
    make_case_with_asset(data_dir, 900, "bg-bbb.jpg", content);
    make_case_with_asset(data_dir, 1000, "bg-ccc.jpg", content);

    let (count, _) = optimize_all_cases(data_dir, None).unwrap();
    assert!(count >= 3, "Should dedup 3 files, got {}", count);

    // Verify all 3 manifests point to the same shared path
    let m800 = read_manifest(&data_dir.join("case/800")).unwrap();
    let m900 = read_manifest(&data_dir.join("case/900")).unwrap();
    let m1000 = read_manifest(&data_dir.join("case/1000")).unwrap();

    let p800 = &m800.asset_map["http://example.com/bg-aaa.jpg"];
    let p900 = &m900.asset_map["http://example.com/bg-bbb.jpg"];
    let p1000 = &m1000.asset_map["http://example.com/bg-ccc.jpg"];

    assert!(p800.starts_with("defaults/shared/"), "Case 800: {}", p800);
    assert!(p900.starts_with("defaults/shared/"), "Case 900: {}", p900);
    assert!(p1000.starts_with("defaults/shared/"), "Case 1000: {}", p1000);

    // All 3 should point to the SAME shared file
    assert_eq!(p800, p900, "All cases should point to same shared path");
    assert_eq!(p900, p1000, "All cases should point to same shared path");

    // The shared file should exist on disk
    assert!(data_dir.join(p800).is_file(), "Shared file should exist on disk");
}
