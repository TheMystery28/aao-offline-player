use super::*;
use crate::importer::aaoffline_helpers::*;
use std::path::PathBuf;

#[test]
fn test_extract_trial_information_basic() {
    let html = r#"<html>
<script>
var trial_information = {"author":"TestUser","author_id":123,"can_read":true,"can_write":false,"format":"Def6","id":42,"language":"en","last_edit_date":1611519081,"sequence":null,"title":"Test Case"};
var initial_trial_data = {"frames":[]};
</script>
</html>"#;
    let info = extract_trial_information(html).unwrap();
    assert_eq!(info.id, 42);
    assert_eq!(info.title, "Test Case");
    assert_eq!(info.author, "TestUser");
    assert_eq!(info.language, "en");
    assert_eq!(info.format, "Def6");
    assert!(info.sequence.is_none());
}

#[test]
fn test_extract_trial_data_basic() {
    let html = r#"var initial_trial_data = {"frames":[0,{"id":1}],"profiles":[0]};"#;
    let data = extract_trial_data(html).unwrap();
    assert!(data["frames"].is_array());
    assert_eq!(data["frames"].as_array().unwrap().len(), 2);
}

#[test]
fn test_extract_trial_data_nested_braces() {
    let html = r#"var initial_trial_data = {"a":{"b":{"c":1}},"d":[{"e":2}]}; var other = 1;"#;
    let data = extract_trial_data(html).unwrap();
    assert_eq!(data["a"]["b"]["c"], 1);
    assert_eq!(data["d"][0]["e"], 2);
}

#[test]
fn test_extract_trial_data_escaped_quotes() {
    let html = r#"var initial_trial_data = {"text":"He said \"hello\""};"#;
    let data = extract_trial_data(html).unwrap();
    assert_eq!(data["text"].as_str().unwrap(), r#"He said "hello""#);
}

#[test]
fn test_import_aaoffline_with_default_sprites() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // HTML with getDefaultSpriteUrl override (like aaoffline downloader produces)
    let html = r#"<html>
<script>
var trial_information = {"author":"Test","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":77777,"language":"en","last_edit_date":1000000,"sequence":null,"title":"Sprite Test"};
var initial_trial_data = {"profiles":[0,{"icon":"assets/icon.png","short_name":"Phoenix","base":"Phoenix","custom_sprites":[]}],"frames":[0],"evidence":[0],"places":[0],"cross_examinations":[0]};
function getDefaultSpriteUrl(base, sprite_id, status)
{
if (base === 'Phoenix' && sprite_id === 1 && status === 'talking') return 'assets/1-aaa.gif';
if (base === 'Phoenix' && sprite_id === 1 && status === 'still') return 'assets/1-bbb.gif';
if (base === 'Phoenix' && sprite_id === 2 && status === 'talking') return 'assets/2-ccc.gif';
return 'data:image/gif;base64,'
}
</script>
</html>"#;
    fs::write(source.path().join("index.html"), html).unwrap();

    let assets_dir = source.path().join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("icon.png"), b"icon").unwrap();
    fs::write(assets_dir.join("1-aaa.gif"), b"talking1").unwrap();
    fs::write(assets_dir.join("1-bbb.gif"), b"still1").unwrap();
    fs::write(assets_dir.join("2-ccc.gif"), b"talking2").unwrap();

    let (manifest, _) = import_aaoffline(source.path(), engine.path(), None).unwrap();
    assert_eq!(manifest.case_id, 77777);
    assert_eq!(manifest.assets.shared_defaults, 3, "Should have 3 default sprites");
    assert_eq!(manifest.assets.case_specific, 4); // icon + 3 sprite files in assets/

    // Verify default sprites were copied to the right locations
    assert!(engine.path().join("defaults/images/chars/Phoenix/1.gif").exists(),
        "talking sprite should exist");
    assert!(engine.path().join("defaults/images/charsStill/Phoenix/1.gif").exists(),
        "still sprite should exist");
    assert!(engine.path().join("defaults/images/chars/Phoenix/2.gif").exists(),
        "talking sprite 2 should exist");
}

#[test]
fn test_import_aaoffline_missing_index() {
    let dir = tempfile::tempdir().unwrap();
    let result: Result<(CaseManifest, u64), String> = import_aaoffline(dir.path(), dir.path(), None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No index.html found"));
}

#[test]
fn test_import_aaoffline_full() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Create a minimal index.html
    let html = r#"<html>
<script>
var trial_information = {"author":"Tester","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":99999,"language":"fr","last_edit_date":1000000,"sequence":null,"title":"Import Test"};
var initial_trial_data = {"profiles":[0,{"icon":"assets/icon.png","short_name":"Hero","custom_sprites":[]}],"frames":[0],"evidence":[0],"places":[0],"cross_examinations":[0]};
</script>
</html>"#;
    fs::write(source.path().join("index.html"), html).unwrap();

    // Create assets
    let assets_dir = source.path().join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("icon.png"), "fake png data").unwrap();
    fs::write(assets_dir.join("bg.jpg"), "fake jpg data").unwrap();

    let (manifest, _) = import_aaoffline(source.path(), engine.path(), None).unwrap();

    assert_eq!(manifest.case_id, 99999);
    assert_eq!(manifest.title, "Import Test");
    assert_eq!(manifest.author, "Tester");
    assert_eq!(manifest.language, "fr");
    assert_eq!(manifest.assets.total_downloaded, 2);
    assert!(manifest.failed_assets.is_empty());

    // Verify files were created
    let case_dir = engine.path().join("case/99999");
    assert!(case_dir.join("manifest.json").exists());
    assert!(case_dir.join("trial_info.json").exists());
    assert!(case_dir.join("trial_data.json").exists());
    assert!(case_dir.join("assets/icon.png").exists());
    assert!(case_dir.join("assets/bg.jpg").exists());

    // Verify URL rewriting in trial_data
    let data_str = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
    let data: serde_json::Value = serde_json::from_str(&data_str).unwrap();
    assert_eq!(
        data["profiles"][1]["icon"].as_str().unwrap(),
        "case/99999/assets/icon.png"
    );
}

/// Integration test: parse the REAL aaoffline download if present.
#[test]
fn test_parse_real_aaoffline_download() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_dir = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("Random/Ace Prosecutor Zero 1  A Trial in the Rain_102059");

    if !source_dir.exists() {
        eprintln!("Skipping: real aaoffline download not found at {}", source_dir.display());
        return;
    }

    let html = fs::read_to_string(source_dir.join("index.html")).unwrap();

    // Parse trial_information
    let info = extract_trial_information(&html).unwrap();
    assert_eq!(info.id, 102059);
    assert_eq!(info.title, "Ace Prosecutor Zero 1 | A Trial in the Rain");
    assert_eq!(info.author, "Exedeb");
    assert_eq!(info.language, "en");
    assert_eq!(info.format, "Def6");

    // Parse trial_data (large JSON)
    let data = extract_trial_data(&html).unwrap();
    assert!(data["frames"].is_array());
    assert!(data["profiles"].is_array());
    let frames = data["frames"].as_array().unwrap();
    assert!(frames.len() > 100, "Expected many frames, got {}", frames.len());
    let profiles = data["profiles"].as_array().unwrap();
    assert!(profiles.len() > 5, "Expected several profiles, got {}", profiles.len());

    // Verify asset references exist in the parsed data
    // Profile icons should reference assets/
    let first_profile = &profiles[1]; // skip the 0 sentinel
    let icon = first_profile["icon"].as_str().unwrap_or("");
    assert!(icon.starts_with("assets/"), "Profile icon should start with 'assets/', got: {}", icon);

    // Full import test into a temp dir
    let engine = tempfile::tempdir().unwrap();
    let (manifest, _) = import_aaoffline(&source_dir, engine.path(), None).unwrap();
    assert_eq!(manifest.case_id, 102059);
    assert!(manifest.assets.total_downloaded > 300, "Expected 300+ assets, got {}", manifest.assets.total_downloaded);
    assert!(manifest.assets.total_size_bytes > 10_000_000, "Expected 10MB+ of assets");

    // Verify trial_data was rewritten correctly
    let case_dir = engine.path().join("case/102059");
    let data_str = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
    assert!(data_str.contains("case/102059/assets/"), "URLs should be rewritten to case/102059/assets/");
    assert!(!data_str.contains("\"assets/"), "No raw 'assets/' refs should remain");
}

#[test]
fn test_import_aaoffline_duplicate_rejected() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    let html = r#"<html>
<script>
var trial_information = {"author":"A","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":12345,"language":"en","last_edit_date":0,"sequence":null,"title":"Dup Test"};
var initial_trial_data = {"frames":[0]};
</script>
</html>"#;
    fs::write(source.path().join("index.html"), html).unwrap();

    // First import succeeds
    let _ = import_aaoffline(source.path(), engine.path(), None).unwrap();

    // Second import should fail (duplicate)
    let result = import_aaoffline(source.path(), engine.path(), None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already exists"));
}

/// Regression: filenames with URL-unsafe characters (like +, #, &) must be
/// sanitized during import so they can be served over HTTP without issues.
#[test]
fn test_import_aaoffline_sanitizes_filenames() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    let html = r#"<html>
<script>
var trial_information = {"author":"A","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":55555,"language":"en","last_edit_date":0,"sequence":null,"title":"Sanitize Test"};
var initial_trial_data = {"frames":[0],"profiles":[0,{"icon":"assets/pioggia+car-123.png","short_name":"Test","custom_sprites":[]}],"evidence":[0],"places":[0],"cross_examinations":[0]};
</script>
</html>"#;
    fs::write(source.path().join("index.html"), html).unwrap();

    let assets_dir = source.path().join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    // Files with problematic characters
    fs::write(assets_dir.join("pioggia+car-123.png"), "img data").unwrap();
    fs::write(assets_dir.join("file#with&special-456.mp3"), "audio data").unwrap();
    fs::write(assets_dir.join("normal-file-789.gif"), "gif data").unwrap();

    let (manifest, _) = import_aaoffline(source.path(), engine.path(), None).unwrap();
    let case_dir = engine.path().join("case/55555");

    // Sanitized files should exist (+ -> -, # -> -, & -> -)
    assert!(case_dir.join("assets/pioggia-car-123.png").exists(),
        "File with + should be renamed with - on disk");
    assert!(case_dir.join("assets/file-with-special-456.mp3").exists(),
        "File with # and & should be renamed with - on disk");
    // Normal files should be unchanged
    assert!(case_dir.join("assets/normal-file-789.gif").exists(),
        "Normal file should keep its name");

    // Original unsanitized names should NOT exist
    assert!(!case_dir.join("assets/pioggia+car-123.png").exists(),
        "Original file with + should not exist");

    // trial_data.json should reference sanitized filenames
    let data_str = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
    assert!(data_str.contains("case/55555/assets/pioggia-car-123.png"),
        "trial_data.json should reference the sanitized filename");
    assert!(!data_str.contains("pioggia+car"),
        "trial_data.json should not contain the unsanitized filename");

    // Manifest asset_map should map old -> new names
    assert_eq!(manifest.assets.total_downloaded, 3);
}

/// Regression: sanitize_imported_filename must handle all URL-unsafe characters.
#[test]
fn test_sanitize_imported_filename() {
    assert_eq!(sanitize_imported_filename("pioggia+car-123.mp3"), "pioggia-car-123.mp3");
    assert_eq!(sanitize_imported_filename("file#fragment-456.png"), "file-fragment-456.png");
    assert_eq!(sanitize_imported_filename("a&b=c-789.gif"), "a-b-c-789.gif");
    assert_eq!(sanitize_imported_filename("100%done-111.jpg"), "100-done-111.jpg");
    assert_eq!(sanitize_imported_filename("normal-file_ok-222.mp3"), "normal-file_ok-222.mp3");
    // Already-safe filenames should be unchanged
    assert_eq!(sanitize_imported_filename("safe-name-333.png"), "safe-name-333.png");
}

#[test]
fn test_sanitize_imported_filename_preserves_extension_case() {
    // Extension should be lowercased
    assert_eq!(sanitize_imported_filename("image.PNG"), "image.png");
    assert_eq!(sanitize_imported_filename("music.MP3"), "music.mp3");
    assert_eq!(sanitize_imported_filename("sprite.GIF"), "sprite.gif");
    assert_eq!(sanitize_imported_filename("normal.jpg"), "normal.jpg");
    // Mixed case extension
    assert_eq!(sanitize_imported_filename("file.JpG"), "file.jpg");
}

/// sanitize_imported_filename with no extension should return sanitized name only.
#[test]
fn test_sanitize_imported_filename_no_extension() {
    assert_eq!(sanitize_imported_filename("filename"), "filename");
    assert_eq!(sanitize_imported_filename("file+name"), "file-name");
    assert_eq!(sanitize_imported_filename("a&b#c"), "a-b-c");
    // Name with only invalid characters
    assert_eq!(sanitize_imported_filename("+++"), "---");
}

#[test]
fn test_extract_default_sprite_mappings() {
    let html = r#"
function getDefaultSpriteUrl(base, sprite_id, status)
{
if (base === 'Phoenix' && sprite_id === 1 && status === 'talking') return 'assets/1-abc.gif';
if (base === 'Phoenix' && sprite_id === 1 && status === 'still') return 'assets/1-def.gif';
if (base === 'Edgeworth' && sprite_id === 3 && status === 'startup') return 'assets/3-ghi.gif';
return 'data:image/gif;base64,'
}
"#;
    let mappings = extract_default_sprite_mappings(html);
    assert_eq!(mappings.len(), 3);

    assert_eq!(mappings[0].base, "Phoenix");
    assert_eq!(mappings[0].sprite_id, 1);
    assert_eq!(mappings[0].status, "talking");
    assert_eq!(mappings[0].asset_path, "assets/1-abc.gif");

    assert_eq!(mappings[1].base, "Phoenix");
    assert_eq!(mappings[1].status, "still");

    assert_eq!(mappings[2].base, "Edgeworth");
    assert_eq!(mappings[2].sprite_id, 3);
    assert_eq!(mappings[2].status, "startup");
}

#[test]
fn test_copy_default_sprites() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Create fake asset files
    let assets_dir = source.path().join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("1-abc.gif"), b"talking_gif").unwrap();
    fs::write(assets_dir.join("1-def.gif"), b"still_gif").unwrap();
    fs::write(assets_dir.join("3-ghi.gif"), b"startup_gif").unwrap();

    let mappings = vec![
        DefaultSpriteMapping { base: "Phoenix".into(), sprite_id: 1, status: "talking".into(), asset_path: "assets/1-abc.gif".into() },
        DefaultSpriteMapping { base: "Phoenix".into(), sprite_id: 1, status: "still".into(), asset_path: "assets/1-def.gif".into() },
        DefaultSpriteMapping { base: "Edgeworth".into(), sprite_id: 3, status: "startup".into(), asset_path: "assets/3-ghi.gif".into() },
    ];

    let (copied, bytes) = copy_default_sprites(&mappings, source.path(), engine.path());
    assert_eq!(copied, 3);
    assert!(bytes > 0);

    // Verify files were placed correctly
    assert!(engine.path().join("defaults/images/chars/Phoenix/1.gif").exists());
    assert!(engine.path().join("defaults/images/charsStill/Phoenix/1.gif").exists());
    assert!(engine.path().join("defaults/images/charsStartup/Edgeworth/3.gif").exists());

    // Running again should skip existing files
    let (copied2, _) = copy_default_sprites(&mappings, source.path(), engine.path());
    assert_eq!(copied2, 0);
}

// --- Batch import tests ---

fn make_aaoffline_case(dir: &std::path::Path, case_id: u32, title: &str) {
    fs::create_dir_all(dir).unwrap();
    let html = format!(
        r#"<html>
<script>
var trial_information = {{"author":"BatchTester","author_id":1,"can_read":true,"can_write":false,"format":"Def6","id":{},"language":"en","last_edit_date":1000000,"sequence":null,"title":"{}"}};
var initial_trial_data = {{"profiles":[0,{{"icon":"","short_name":"Hero","custom_sprites":[]}}],"frames":[0],"evidence":[0],"places":[0],"cross_examinations":[0]}};
</script>
</html>"#,
        case_id, title
    );
    fs::write(dir.join("index.html"), html).unwrap();
}

#[test]
fn test_import_aaoffline_batch_multiple_subfolders() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    make_aaoffline_case(&source.path().join("case1"), 70001, "Batch Part 1");
    make_aaoffline_case(&source.path().join("case2"), 70002, "Batch Part 2");
    make_aaoffline_case(&source.path().join("case3"), 70003, "Batch Part 3");

    let result = import_aaoffline_batch(source.path(), engine.path(), None, None).unwrap();
    assert_eq!(result.batch_manifests.len(), 3, "Should import 3 cases");

    let ids: Vec<u32> = result.batch_manifests.iter().map(|m| m.case_id).collect();
    assert!(ids.contains(&70001));
    assert!(ids.contains(&70002));
    assert!(ids.contains(&70003));
}

#[test]
fn test_import_aaoffline_batch_skips_existing() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Import case1 first
    make_aaoffline_case(&source.path().join("case1"), 70010, "Already There");
    let _ = import_aaoffline(&source.path().join("case1"), engine.path(), None).unwrap();

    // Now batch import case1 + case2
    make_aaoffline_case(&source.path().join("case2"), 70011, "New Case");
    let result = import_aaoffline_batch(source.path(), engine.path(), None, None).unwrap();

    // case1 silently skipped (already exists), case2 succeeds
    assert_eq!(result.batch_manifests.len(), 1, "Only new case should be in manifests");
    assert_eq!(result.batch_manifests[0].case_id, 70011);
}

#[test]
fn test_import_aaoffline_batch_with_root_and_subfolders() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    // Root case (index.html in parent dir)
    make_aaoffline_case(source.path(), 70020, "Root Case");
    // Subfolder case
    make_aaoffline_case(&source.path().join("sub1"), 70021, "Sub Case");

    let result = import_aaoffline_batch(source.path(), engine.path(), None, None).unwrap();
    assert_eq!(result.batch_manifests.len(), 2, "Root + subfolder should both import");

    let ids: Vec<u32> = result.batch_manifests.iter().map(|m| m.case_id).collect();
    assert!(ids.contains(&70020));
    assert!(ids.contains(&70021));
}

#[test]
fn test_import_aaoffline_batch_empty_folder() {
    let source = tempfile::tempdir().unwrap();
    let engine = tempfile::tempdir().unwrap();

    let result = import_aaoffline_batch(source.path(), engine.path(), None, None);
    assert!(result.is_err(), "Empty folder should return error");
    assert!(result.unwrap_err().contains("No index.html found"));
}

// --- Test collection data validation ---

/// Helper: path to test data directory.
fn test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test-data")
}

/// All 6 test manifests should be valid with correct sequence metadata.
#[test]
fn test_collection_manifests_valid() {
    let base = test_data_dir();
    let collections = [
        ("collection-a", &[99901u32, 99902][..], "Test A - Explicit Redirect"),
        ("collection-b", &[99903u32, 99904][..], "Test B - Auto-Continue (action=0)"),
        ("collection-c", &[99905u32, 99906][..], "Test C - Auto-Continue (no GameOver)"),
    ];

    for (folder, ids, expected_title) in &collections {
        for &id in *ids {
            let case_dir = base.join(folder).join(id.to_string());
            let manifest = read_manifest(&case_dir)
                .unwrap_or_else(|e| panic!("Failed to read manifest for {}: {}", id, e));
            assert_eq!(manifest.case_id, id);
            assert_eq!(manifest.author, "TestBot");
            assert_eq!(manifest.language, "en");
            assert_eq!(manifest.format, "Def6");
            assert!(manifest.sequence.is_some(), "Case {} should have sequence", id);

            let seq = manifest.sequence.as_ref().unwrap();
            assert_eq!(seq["title"].as_str().unwrap(), *expected_title,
                "Case {} sequence title mismatch", id);
            let list = seq["list"].as_array().unwrap();
            assert_eq!(list.len(), 2, "Case {} should list 2 parts", id);
        }
    }
}

/// Collection A Part 1: last frame has GameOver with action=val=1 (explicit next).
#[test]
fn test_collection_a_part1_has_gameover_next() {
    let data_str = fs::read_to_string(
        test_data_dir().join("collection-a/99901/trial_data.json")
    ).unwrap();
    let data: serde_json::Value = serde_json::from_str(&data_str).unwrap();
    let frames = data["frames"].as_array().unwrap();
    let last = &frames[frames.len() - 1];

    assert_eq!(last["action_name"], "GameOver");
    assert_eq!(last["action_parameters"]["global"]["action"], "val=1");
}

/// Collection A Part 2: no GameOver (destination part, just runs out).
#[test]
fn test_collection_a_part2_no_gameover() {
    let data_str = fs::read_to_string(
        test_data_dir().join("collection-a/99902/trial_data.json")
    ).unwrap();
    let data: serde_json::Value = serde_json::from_str(&data_str).unwrap();
    let frames = data["frames"].as_array().unwrap();
    for (i, frame) in frames.iter().enumerate() {
        if !frame.is_object() { continue; }
        assert_ne!(frame["action_name"].as_str().unwrap_or(""), "GameOver",
            "Collection A Part 2 frame {} should NOT have GameOver", i);
    }
}

/// Collection B Part 1: last frame has GameOver with action=val=0 (end and do nothing).
#[test]
fn test_collection_b_part1_has_gameover_end() {
    let data_str = fs::read_to_string(
        test_data_dir().join("collection-b/99903/trial_data.json")
    ).unwrap();
    let data: serde_json::Value = serde_json::from_str(&data_str).unwrap();
    let frames = data["frames"].as_array().unwrap();
    let last = &frames[frames.len() - 1];

    assert_eq!(last["action_name"], "GameOver");
    assert_eq!(last["action_parameters"]["global"]["action"], "val=0");
}

/// Collection C Part 1: no GameOver at all -- just runs out of frames.
#[test]
fn test_collection_c_part1_no_gameover() {
    let data_str = fs::read_to_string(
        test_data_dir().join("collection-c/99905/trial_data.json")
    ).unwrap();
    let data: serde_json::Value = serde_json::from_str(&data_str).unwrap();
    let frames = data["frames"].as_array().unwrap();
    for (i, frame) in frames.iter().enumerate() {
        if !frame.is_object() { continue; }
        assert_ne!(frame["action_name"].as_str().unwrap_or(""), "GameOver",
            "Collection C Part 1 frame {} should NOT have GameOver", i);
    }
}

/// Export/import roundtrip for each test collection.
#[test]
fn test_export_import_collections_roundtrip() {
    let base = test_data_dir();
    let collections: Vec<(&str, Vec<u32>, &str)> = vec![
        ("collection-a", vec![99901, 99902], "Test Collection A - Explicit Redirect"),
        ("collection-b", vec![99903, 99904], "Test Collection B - Auto-Continue (action=0)"),
        ("collection-c", vec![99905, 99906], "Test Collection C - Auto-Continue (no GameOver)"),
    ];

    for (folder, ids, title) in &collections {
        let engine_export = tempfile::tempdir().unwrap();
        let engine_import = tempfile::tempdir().unwrap();
        let tmp = tempfile::tempdir().unwrap();

        // Copy test data into engine_export
        for &id in ids {
            let src = base.join(folder).join(id.to_string());
            let dst = engine_export.path().join("case").join(id.to_string());
            fs::create_dir_all(&dst).unwrap();
            for name in &["manifest.json", "trial_info.json", "trial_data.json"] {
                let src_file = src.join(name);
                if src_file.exists() {
                    fs::copy(&src_file, dst.join(name)).unwrap();
                }
            }
        }

        // Build sequence list
        let seq_list: Vec<serde_json::Value> = ids.iter().map(|&id| {
            let m = read_manifest(
                &engine_export.path().join("case").join(id.to_string())
            ).unwrap();
            serde_json::json!({"id": id, "title": m.title})
        }).collect();

        let export_path = tmp.path().join("test.aaocase");
        let size = export_sequence(
            &ids, title, &serde_json::Value::Array(seq_list),
            engine_export.path(), &export_path, None, None, true,
        ).unwrap();
        assert!(size > 0, "{} export should have non-zero size", folder);

        // Import
        let manifest = import_aaocase_zip(&export_path, engine_import.path(), None).unwrap().manifest;
        assert_eq!(manifest.case_id, ids[0], "{} first case should match", folder);

        // Verify all parts present
        for &id in ids {
            let case_dir = engine_import.path().join("case").join(id.to_string());
            assert!(case_dir.join("manifest.json").exists(),
                "{} case {} manifest should exist after import", folder, id);
            assert!(case_dir.join("trial_data.json").exists(),
                "{} case {} trial_data should exist after import", folder, id);
        }
    }
}
