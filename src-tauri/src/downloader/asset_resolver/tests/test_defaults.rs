use super::*;

// --- parse_js_object_assignment ---

#[test]
fn test_parse_js_object_assignment_simple() {
    let src = r#"var foo = {"a": 1, "b": 2};"#;
    let result = parse_js_object_assignment(src, "foo");
    assert_eq!(result.unwrap(), r#"{"a": 1, "b": 2}"#);
}

#[test]
fn test_parse_js_object_assignment_not_found() {
    let src = "var foo = 42;";
    assert!(parse_js_object_assignment(src, "bar").is_none());
}

#[test]
fn test_parse_default_profiles_nb_from_real_format() {
    let src = r#"var default_profiles_nb = {"Juge2": 6, "Phoenix": 20, "Inconnu": 0};"#;
    let json_str = parse_js_object_assignment(src, "default_profiles_nb").unwrap();
    let map: HashMap<String, u32> = serde_json::from_str(&json_str).unwrap();
    assert_eq!(map["Juge2"], 6);
    assert_eq!(map["Phoenix"], 20);
    assert_eq!(map["Inconnu"], 0);
}

// --- extract_default_sprite_assets ---

#[test]
fn test_extract_default_sprite_assets_generates_urls() {
    let dir = tempfile::tempdir().unwrap();
    let js_dir = dir.path().join("Javascript");
    std::fs::create_dir_all(&js_dir).unwrap();
    std::fs::write(
        js_dir.join("default_data.js"),
        r#"var default_profiles_nb = {"Juge2": 3, "Inconnu": 0};
var default_profiles_startup = {"Juge2/2": 880};"#,
    ).unwrap();

    let data = json!({
        "profiles": [null, {"base": "Juge2", "icon": "", "custom_sprites": []}]
    });
    let paths = test_site_paths();
    let assets = extract_default_sprite_assets(&data, &paths, dir.path());

    let talking: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_talking").collect();
    let still: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_still").collect();
    let startup: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_startup").collect();

    assert_eq!(talking.len(), 3);
    assert_eq!(still.len(), 3);
    assert_eq!(startup.len(), 1);

    assert!(talking.iter().any(|a| a.local_path == "defaults/images/chars/Juge2/1.gif"));
    assert!(still.iter().any(|a| a.local_path == "defaults/images/charsStill/Juge2/2.gif"));
    assert!(startup[0].local_path == "defaults/images/charsStartup/Juge2/2.gif");
}

#[test]
fn test_extract_default_sprite_assets_skips_inconnu() {
    let dir = tempfile::tempdir().unwrap();
    let js_dir = dir.path().join("Javascript");
    std::fs::create_dir_all(&js_dir).unwrap();
    std::fs::write(
        js_dir.join("default_data.js"),
        r#"var default_profiles_nb = {"Inconnu": 0};
var default_profiles_startup = {};"#,
    ).unwrap();

    let data = json!({
        "profiles": [null, {"base": "Inconnu", "icon": "", "custom_sprites": []}]
    });
    let assets = extract_default_sprite_assets(&data, &test_site_paths(), dir.path());
    assert!(assets.is_empty());
}

// --- extract_default_place_assets ---

#[test]
fn test_extract_default_place_assets_from_real_data() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let engine_dir = manifest_dir.parent().unwrap().join("engine");
    if !engine_dir.join("Javascript/default_data.js").exists() {
        return; // Skip if engine dir doesn't exist (CI)
    }

    let paths = test_site_paths();
    let assets = extract_default_place_assets(&engine_dir, &paths);

    assert!(assets.len() >= 20,
        "Expected at least 20 default place assets, got {}",
        assets.len());

    let bgs: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_place_bg").collect();
    assert!(bgs.len() >= 15, "Expected at least 15 backgrounds, got {}", bgs.len());

    let fgs: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_place_fg").collect();
    assert!(fgs.len() >= 5, "Expected at least 5 foreground objects, got {}", fgs.len());

    for asset in &assets {
        assert!(asset.local_path.starts_with("defaults/images/defaultplaces/"),
            "Local path should start with defaults/images/defaultplaces/, got: {}",
            asset.local_path);
        assert!(asset.is_default, "Default place assets should be marked as default");
    }

    let has_courtroom = assets.iter().any(|a| a.local_path.contains("pw_courtroom.jpg"));
    assert!(has_courtroom, "Should include pw_courtroom.jpg");

    let has_benches = assets.iter().any(|a| a.local_path.contains("pw_courtroom_benches.gif"));
    assert!(has_benches, "Should include pw_courtroom_benches.gif foreground object");
}
