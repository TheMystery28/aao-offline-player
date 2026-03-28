use super::*;

// --- sanitize_path ---

#[test]
fn test_sanitize_path_replaces_colons() {
    assert_eq!(
        sanitize_path("defaults/music/Ace Attorney Investigations : Miles Edgeworth 2/song.mp3"),
        "defaults/music/Ace Attorney Investigations _ Miles Edgeworth 2/song.mp3"
    );
}

#[test]
fn test_sanitize_path_replaces_all_illegal_chars() {
    assert_eq!(sanitize_path("a:b*c?d\"e<f>g|h"), "a_b_c_d_e_f_g_h");
}

#[test]
fn test_sanitize_path_preserves_valid_paths() {
    let path = "defaults/images/backgrounds/Court.jpg";
    assert_eq!(sanitize_path(path), path);
}

// --- is_external ---

#[test]
fn test_is_external_bool() {
    assert!(is_external(&json!(true)));
    assert!(!is_external(&json!(false)));
}

#[test]
fn test_is_external_int() {
    assert!(is_external(&json!(1)));
    assert!(!is_external(&json!(0)));
}

#[test]
fn test_is_external_null() {
    assert!(!is_external(&json!(null)));
}

// --- add_asset ---

#[test]
fn test_add_asset_upgrades_empty_local_path() {
    let mut assets: Vec<AssetRef> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let url = "http://example.com/sprite.gif".to_string();

    // First add: external (empty local_path)
    add_asset(&mut assets, &mut seen, url.clone(), "custom_sprite_talking", false, String::new());
    assert_eq!(assets.len(), 1);
    assert!(assets[0].local_path.is_empty());
    assert!(!assets[0].is_default);

    // Second add: default with proper local_path -> should upgrade
    add_asset(&mut assets, &mut seen, url.clone(), "default_sprite_talking", true, "defaults/images/chars/Test/1.gif".to_string());
    assert_eq!(assets.len(), 1, "Should still be 1 entry, not 2");
    assert_eq!(assets[0].local_path, "defaults/images/chars/Test/1.gif");
    assert!(assets[0].is_default);
}

#[test]
fn test_add_asset_no_upgrade_when_existing_has_path() {
    let mut assets: Vec<AssetRef> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let url = "http://example.com/sprite.gif".to_string();

    // First add: has a local_path
    add_asset(&mut assets, &mut seen, url.clone(), "bg", false, "some/existing/path.gif".to_string());
    assert_eq!(assets[0].local_path, "some/existing/path.gif");

    // Second add: different local_path -> should NOT overwrite
    add_asset(&mut assets, &mut seen, url.clone(), "default_sprite_talking", true, "defaults/other.gif".to_string());
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].local_path, "some/existing/path.gif", "Original local_path should be preserved");
}

// --- unicode ---

#[test]
fn test_unicode_filename_in_sanitize_path() {
    // Accented characters should be preserved (they're valid on all OS)
    assert_eq!(
        sanitize_path("Ace Attorney/Th\u{00e8}me \u{00e9}t\u{00e9}.mp3"),
        "Ace Attorney/Th\u{00e8}me \u{00e9}t\u{00e9}.mp3",
        "Unicode letters should be preserved"
    );
    // Japanese characters should be preserved
    assert_eq!(
        sanitize_path("\u{9006}\u{8ee2}\u{88c1}\u{5224}/\u{30c6}\u{30fc}\u{30de}.mp3"),
        "\u{9006}\u{8ee2}\u{88c1}\u{5224}/\u{30c6}\u{30fc}\u{30de}.mp3",
        "Japanese characters should be preserved"
    );
    // Only Windows-illegal chars should be replaced
    assert_eq!(
        sanitize_path("Th\u{00e8}me: \u{00e9}t\u{00e9} \"test\" <special>.mp3"),
        "Th\u{00e8}me_ \u{00e9}t\u{00e9} _test_ _special_.mp3",
        "Only :\"<> should be replaced, accented chars kept"
    );
}

// --- Property-based tests ---

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn sanitize_path_removes_all_illegal_chars(input in "\\PC{0,200}") {
            let result = sanitize_path(&input);
            for c in result.chars() {
                prop_assert!(
                    c != ':' && c != '*' && c != '?' && c != '"' && c != '<' && c != '>' && c != '|',
                    "sanitize_path({:?}) still contains illegal char '{}' in result: {:?}",
                    input, c, result
                );
            }
        }
    }
}
