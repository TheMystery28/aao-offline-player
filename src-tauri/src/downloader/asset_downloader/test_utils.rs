use super::*;
use std::path::PathBuf;
use crate::downloader::AssetRef;

#[test]
fn test_generate_filename() {
    let name = generate_filename("https://aaonline.fr/uploads/sprites/chars/Phoenix/1.gif");
    assert!(name.ends_with(".gif"));
    assert!(name.contains('-'));
    let name2 = generate_filename("https://aaonline.fr/uploads/sprites/chars/Phoenix/1.gif");
    assert_eq!(name, name2);
    let name3 = generate_filename("https://aaonline.fr/uploads/sprites/chars/Phoenix/2.gif");
    assert_ne!(name, name3);
}

#[test]
fn test_generate_filename_strips_query_string() {
    let name = generate_filename("https://example.com/image.png?v=123&t=456");
    assert!(name.ends_with(".png"));
}

#[test]
fn test_generate_filename_no_extension_uses_bin() {
    let name = generate_filename("https://example.com/asset");
    assert!(name.ends_with(".bin"));
}

#[test]
fn test_generate_filename_sanitizes_special_chars() {
    let name = generate_filename("https://example.com/my image (1).png");
    assert!(name.ends_with(".png"));
    assert!(!name.contains(' '));
    assert!(!name.contains('('));
}

/// Regression: external assets were saved to case_dir/assets/assets/{hash}
/// because save_dir was case_dir/assets/ and relative_path was "assets/{hash}".
/// Fix: save_dir must be case_dir (not case_dir/assets/).
#[test]
fn test_external_asset_path_no_double_nesting() {
    let case_dir = PathBuf::from("/data/case/123");
    let url = "http://i.imgur.com/abc.png";
    let filename = generate_filename(url);

    // Replicate the path construction from download_assets for external assets
    let local_path = ""; // external → empty local_path
    let (save_dir, relative_path) = if local_path.is_empty() {
        (case_dir.clone(), format!("assets/{}", filename))
    } else {
        unreachable!()
    };

    let final_path = save_dir.join(&relative_path);
    let final_str = final_path.to_string_lossy();

    // Must NOT contain double-nested assets/assets/
    assert!(
        !final_str.contains("assets/assets") && !final_str.contains("assets\\assets"),
        "Double-nested assets directory detected: {}",
        final_str
    );
    // Must be exactly case_dir/assets/{filename}
    assert_eq!(final_path, case_dir.join("assets").join(&filename));
}

/// Verify internal assets go to engine_dir/{local_path}, not case_dir.
#[test]
fn test_internal_asset_path_uses_engine_dir() {
    let case_dir = PathBuf::from("/data/case/123");
    let engine_dir = PathBuf::from("/data/engine");
    let local_path = "defaults/images/chars/Phoenix.png";

    // Replicate the path construction from download_assets for internal assets
    let (save_dir, relative_path) = if !local_path.is_empty() {
        (engine_dir.clone(), local_path.to_string())
    } else {
        unreachable!()
    };

    let final_path = save_dir.join(&relative_path);

    // Must be under engine_dir, not case_dir
    assert!(final_path.starts_with(&engine_dir));
    assert!(!final_path.starts_with(&case_dir));
}

// --- check_skip_existing tests ---

/// Default asset already on disk → skip download.
#[test]
fn test_skip_existing_default_asset() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Simulate a bundled default sprite
    let rel = "defaults/images/chars/Phoenix/1.gif";
    let file_path = engine_dir.join(rel);
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    std::fs::write(&file_path, b"GIF89a_fake_image_data").unwrap();

    let result = check_skip_existing(engine_dir, rel);
    assert!(result.is_some(), "Should skip download for existing default asset");
    assert_eq!(result.unwrap(), 22); // "GIF89a_fake_image_data" is 22 bytes
}

/// Missing asset → must download.
#[test]
fn test_no_skip_missing_asset() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    let result = check_skip_existing(engine_dir, "defaults/images/chars/Phoenix/1.gif");
    assert!(result.is_none(), "Should not skip download for missing asset");
}

/// Empty file (0 bytes) → must re-download.
#[test]
fn test_no_skip_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    let rel = "defaults/music/AA1/track.mp3";
    let file_path = engine_dir.join(rel);
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    std::fs::write(&file_path, b"").unwrap();

    let result = check_skip_existing(engine_dir, rel);
    assert!(result.is_none(), "Should not skip download for empty file");
}

/// Nested default paths (backgrounds, sounds, voices) all skip correctly.
#[test]
fn test_skip_existing_various_default_types() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    let cases = vec![
        ("defaults/images/backgrounds/AA4/Court.jpg", b"JFIF_fake" as &[u8]),
        ("defaults/sounds/sfx-blipmale.wav", b"RIFF_fake"),
        ("defaults/voices/French/Objection.mp3", b"ID3_fake"),
        ("defaults/images/charsStill/Phoenix/1.gif", b"GIF87a"),
        ("defaults/images/charsStartup/Apollo/1.gif", b"GIF89a"),
    ];

    for (rel, content) in &cases {
        let file_path = engine_dir.join(rel);
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, content).unwrap();
    }

    for (rel, content) in &cases {
        let result = check_skip_existing(engine_dir, rel);
        assert!(
            result.is_some(),
            "Should skip download for existing default: {}",
            rel
        );
        assert_eq!(
            result.unwrap(),
            content.len() as u64,
            "Size mismatch for {}",
            rel
        );
    }
}

/// End-to-end: simulate the full path construction + skip check
/// for an internal default asset, as download_assets would do.
#[test]
fn test_skip_existing_end_to_end_internal_asset() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path();

    // Pre-populate a default asset
    let default_rel = "defaults/images/chars/Phoenix/1.gif";
    let full_path = engine_dir.join(default_rel);
    std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
    std::fs::write(&full_path, b"GIF89a_sprite_data_here").unwrap();

    // Simulate what download_assets does for an internal asset
    let asset = AssetRef {
        url: "https://aaonline.fr/Ressources/Images/Personnages/Phoenix/1.gif".to_string(),
        asset_type: "icon".to_string(),
        is_default: true,
        local_path: default_rel.to_string(),
    };

    // Path construction from download_assets
    let (save_dir, relative_path) = if asset.local_path.is_empty() {
        unreachable!("internal asset should have local_path");
    } else {
        (engine_dir.to_path_buf(), asset.local_path.clone())
    };

    // This is exactly the check that prevents re-downloading
    let result = check_skip_existing(&save_dir, &relative_path);
    assert!(
        result.is_some(),
        "Internal default asset with local_path='{}' should be skipped",
        asset.local_path
    );
}

/// External assets (empty local_path) use case_dir, not engine_dir.
/// If a previous download already saved the file, it should be skipped.
#[test]
fn test_skip_existing_external_asset_in_case_dir() {
    let dir = tempfile::tempdir().unwrap();
    let case_dir = dir.path();

    let url = "http://i.imgur.com/abc.png";
    let filename = generate_filename(url);
    let relative_path = format!("assets/{}", filename);

    // Pre-populate the external asset
    let full_path = case_dir.join(&relative_path);
    std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
    std::fs::write(&full_path, b"PNG_fake_external_image").unwrap();

    let result = check_skip_existing(case_dir, &relative_path);
    assert!(
        result.is_some(),
        "Previously downloaded external asset should be skipped"
    );
}

/// External asset not yet downloaded → must download.
#[test]
fn test_no_skip_external_asset_not_downloaded() {
    let dir = tempfile::tempdir().unwrap();
    let case_dir = dir.path();

    let url = "http://i.imgur.com/abc.png";
    let filename = generate_filename(url);
    let relative_path = format!("assets/{}", filename);

    let result = check_skip_existing(case_dir, &relative_path);
    assert!(
        result.is_none(),
        "Missing external asset should not be skipped"
    );
}

/// Same URL must always produce the same filename (determinism).
#[test]
fn test_generate_filename_deterministic() {
    let url = "https://aaonline.fr/uploads/sprites/chars/Apollo/7.gif";
    let name1 = generate_filename(url);
    let name2 = generate_filename(url);
    let name3 = generate_filename(url);
    assert_eq!(name1, name2);
    assert_eq!(name2, name3);
}

/// Different URLs must produce different filenames.
#[test]
fn test_generate_filename_different_urls_different_names() {
    let name_a = generate_filename("https://example.com/image_a.png");
    let name_b = generate_filename("https://example.com/image_b.png");
    let name_c = generate_filename("https://other.com/image_a.png");
    assert_ne!(name_a, name_b, "Different filenames on same host should differ");
    assert_ne!(name_a, name_c, "Same filename on different hosts should differ");
}

/// URL with unicode characters should produce a valid filename.
#[test]
fn test_generate_filename_unicode() {
    let name = generate_filename("https://example.com/images/café_résumé.png");
    assert!(name.ends_with(".png"));
    assert!(!name.is_empty());
    assert!(name.contains('-'), "Filename should contain hash separator");
    // Unicode alphanumeric chars are preserved by is_alphanumeric(), which is correct
    // behavior — they're valid in filenames on all platforms
    assert!(
        !name.contains(' ') && !name.contains('(') && !name.contains(')'),
        "Filename should not contain spaces or special chars: {}",
        name
    );
}

/// Very long URL should still produce a reasonable filename.
#[test]
fn test_generate_filename_very_long_url() {
    let long_path = "a".repeat(500);
    let url = format!("https://example.com/{}.jpg", long_path);
    let name = generate_filename(&url);
    assert!(name.ends_with(".jpg"));
    assert!(!name.is_empty());
    // The filename includes the full sanitized name + hash, which could be long,
    // but it should still be well-formed
    assert!(name.contains('-'), "Filename should contain hash separator");
}

/// check_skip_existing returns correct file size when file exists with known content.
#[test]
fn test_check_skip_existing_returns_correct_size() {
    let dir = tempfile::tempdir().unwrap();
    let rel = "assets/test-image.png";
    let file_path = dir.path().join(rel);
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    let content = b"PNG_image_data_exactly_42_bytes_long_paddd";
    assert_eq!(content.len(), 42);
    std::fs::write(&file_path, content).unwrap();

    let result = check_skip_existing(dir.path(), rel);
    assert!(result.is_some(), "File exists and has content, should return Some");
    assert_eq!(result.unwrap(), 42, "Should return exact file size in bytes");
}

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn generate_filename_always_valid(
            url in "https?://[a-z]{1,10}\\.[a-z]{2,4}/[a-zA-Z0-9 _\\-]{1,50}\\.[a-z]{2,4}"
        ) {
            let name = generate_filename(&url);
            prop_assert!(!name.is_empty(), "Filename should not be empty for URL: {}", url);
            prop_assert!(!name.contains('/'), "Filename contains slash: {} from URL: {}", name, url);
            prop_assert!(!name.contains('\\'), "Filename contains backslash: {} from URL: {}", name, url);
            prop_assert!(name.contains('.'), "Filename has no extension: {} from URL: {}", name, url);
        }
    }
}
