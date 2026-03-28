use super::*;

#[test]
fn test_extract_profiles_default_icon() {
    let data = json!({
        "profiles": [null, {"base": "Phoenix", "icon": "", "custom_sprites": []}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let icons: Vec<_> = assets.iter().filter(|a| a.asset_type == "profile_icon").collect();
    assert_eq!(icons.len(), 1);
    assert!(icons[0].url.contains("persos/Phoenix.png"));
    assert_eq!(icons[0].local_path, "defaults/images/chars/Phoenix.png");
    assert!(!icons[0].is_default);
}

#[test]
fn test_extract_profiles_external_icon() {
    let data = json!({
        "profiles": [null, {"base": "Phoenix", "icon": "http://i.imgur.com/abc.png", "custom_sprites": []}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let icons: Vec<_> = assets.iter().filter(|a| a.asset_type == "profile_icon").collect();
    assert_eq!(icons.len(), 1);
    assert_eq!(icons[0].url, "http://i.imgur.com/abc.png");
    assert!(icons[0].local_path.is_empty());
}

#[test]
fn test_extract_custom_sprites() {
    let data = json!({
        "profiles": [null, {
            "base": "Phoenix",
            "icon": "",
            "custom_sprites": [{"talking": "http://x.com/t.gif", "still": "http://x.com/s.gif", "startup": ""}]
        }]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let sprites: Vec<_> = assets.iter().filter(|a| a.asset_type.starts_with("custom_sprite")).collect();
    assert_eq!(sprites.len(), 2);
    assert!(sprites.iter().any(|s| s.url == "http://x.com/t.gif"));
    assert!(sprites.iter().any(|s| s.url == "http://x.com/s.gif"));
}

#[test]
fn test_extract_evidence_internal() {
    let data = json!({
        "evidence": [null, {"icon": "badge", "icon_external": false, "check_button_data": []}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let ev: Vec<_> = assets.iter().filter(|a| a.asset_type == "evidence_icon").collect();
    assert_eq!(ev.len(), 1);
    assert!(ev[0].url.contains("dossier/badge.png"));
    assert_eq!(ev[0].local_path, "defaults/images/evidence/badge.png");
}

#[test]
fn test_extract_evidence_external() {
    let data = json!({
        "evidence": [null, {"icon": "http://i.imgur.com/ev.png", "icon_external": true, "check_button_data": []}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let ev: Vec<_> = assets.iter().filter(|a| a.asset_type == "evidence_icon").collect();
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].url, "http://i.imgur.com/ev.png");
    assert!(ev[0].local_path.is_empty());
}

#[test]
fn test_extract_background_internal() {
    let data = json!({
        "places": [null, {
            "background": {"image": "Court", "external": false},
            "background_objects": [],
            "foreground_objects": []
        }]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let bgs: Vec<_> = assets.iter().filter(|a| a.asset_type == "background").collect();
    assert_eq!(bgs.len(), 1);
    assert!(bgs[0].url.contains("cinematiques/Court.jpg"));
    assert_eq!(bgs[0].local_path, "defaults/images/backgrounds/Court.jpg");
}

#[test]
fn test_extract_background_external() {
    let data = json!({
        "places": [null, {
            "background": {"image": "http://i.imgur.com/bg.png", "external": true},
            "background_objects": [],
            "foreground_objects": []
        }]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let bgs: Vec<_> = assets.iter().filter(|a| a.asset_type == "background").collect();
    assert_eq!(bgs.len(), 1);
    assert_eq!(bgs[0].url, "http://i.imgur.com/bg.png");
    assert!(bgs[0].local_path.is_empty());
}

#[test]
fn test_extract_music_internal_with_colon() {
    let data = json!({
        "music": [null, {"path": "Ace Attorney Investigations : ME2/song", "external": false}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let music: Vec<_> = assets.iter().filter(|a| a.asset_type == "music").collect();
    assert_eq!(music.len(), 1);
    assert!(music[0].local_path.contains("Investigations _ ME2"));
    assert!(!music[0].local_path.contains(':'));
}

#[test]
fn test_extract_music_external() {
    let data = json!({
        "music": [null, {"path": "http://example.com/song.mp3", "external": true}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let music: Vec<_> = assets.iter().filter(|a| a.asset_type == "music").collect();
    assert_eq!(music.len(), 1);
    assert_eq!(music[0].url, "http://example.com/song.mp3");
    assert!(music[0].local_path.is_empty());
}

#[test]
fn test_extract_default_sprites() {
    let data = json!({
        "profiles": [null, {"base": "Phoenix", "icon": "", "custom_sprites": []}],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -3}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let talking: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_talking").collect();
    let still: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_still").collect();
    assert_eq!(talking.len(), 1);
    assert_eq!(still.len(), 1);
    assert!(talking[0].is_default);
    assert_eq!(talking[0].local_path, "defaults/images/chars/Phoenix/3.gif");
    assert_eq!(still[0].local_path, "defaults/images/charsStill/Phoenix/3.gif");
}

#[test]
fn test_extract_voices() {
    let data = json!({});
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let voices: Vec<_> = assets.iter().filter(|a| a.asset_type == "voice").collect();
    assert_eq!(voices.len(), 9);
    assert!(voices.iter().all(|v| v.is_default));
}

#[test]
fn test_extract_deduplicates() {
    let data = json!({
        "profiles": [
            null,
            {"base": "Phoenix", "icon": "", "custom_sprites": []},
            {"base": "Phoenix", "icon": "", "custom_sprites": []}
        ]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let icons: Vec<_> = assets.iter().filter(|a| a.asset_type == "profile_icon").collect();
    assert_eq!(icons.len(), 1);
}

// --- classify_assets ---

#[test]
fn test_classify_assets() {
    let assets = vec![
        AssetRef { url: "a".into(), asset_type: "bg".into(), is_default: false, local_path: "p".into() },
        AssetRef { url: "b".into(), asset_type: "sprite".into(), is_default: true, local_path: "q".into() },
        AssetRef { url: "c".into(), asset_type: "music".into(), is_default: false, local_path: String::new() },
    ];
    let (case_specific, shared) = classify_assets(assets);
    assert_eq!(case_specific.len(), 2);
    assert_eq!(shared.len(), 1);
    assert!(shared[0].is_default);
}

#[test]
fn test_classify_then_filter_missing_defaults() {
    let assets = vec![
        AssetRef { url: "http://a.com/bg.jpg".into(), asset_type: "bg".into(), is_default: false, local_path: "defaults/images/backgrounds/Court.jpg".into() },
        AssetRef { url: "http://a.com/sprite.gif".into(), asset_type: "sprite".into(), is_default: true, local_path: "defaults/images/chars/Phoenix/1.gif".into() },
        AssetRef { url: "http://a.com/voice.opus".into(), asset_type: "voice".into(), is_default: true, local_path: "defaults/voices/voice_singleblip_1.opus".into() },
    ];
    let (case_specific, shared) = classify_assets(assets);
    let engine_dir = std::path::PathBuf::from("/nonexistent/engine");
    let missing: Vec<_> = shared
        .into_iter()
        .filter(|a| !a.local_path.is_empty() && !engine_dir.join(&a.local_path).exists())
        .collect();
    assert_eq!(case_specific.len(), 1);
    assert_eq!(missing.len(), 2);
}

#[test]
fn test_filter_skips_existing_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let engine_dir = dir.path().to_path_buf();
    let voices_dir = engine_dir.join("defaults/voices");
    std::fs::create_dir_all(&voices_dir).unwrap();
    std::fs::write(voices_dir.join("voice_singleblip_1.opus"), "data").unwrap();

    let shared = vec![
        AssetRef { url: "http://a.com/v1.opus".into(), asset_type: "voice".into(), is_default: true, local_path: "defaults/voices/voice_singleblip_1.opus".into() },
        AssetRef { url: "http://a.com/v2.opus".into(), asset_type: "voice".into(), is_default: true, local_path: "defaults/voices/voice_singleblip_2.opus".into() },
    ];
    let missing: Vec<_> = shared
        .into_iter()
        .filter(|a| !a.local_path.is_empty() && !engine_dir.join(&a.local_path).exists())
        .collect();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].local_path, "defaults/voices/voice_singleblip_2.opus");
}

// --- psyche locks ---

#[test]
fn test_extract_psyche_locks() {
    let data = json!({
        "scenes": [null, {
            "dialogues": [{"locks": {"locks_to_display": 3}}]
        }]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let locks: Vec<_> = assets.iter().filter(|a| a.asset_type == "psyche_lock").collect();
    assert_eq!(locks.len(), 4);
    let names: Vec<&str> = locks.iter().map(|l| l.local_path.as_str()).collect();
    assert!(names.contains(&"defaults/images/psycheLocks/fg_chains_appear.gif"));
    assert!(names.contains(&"defaults/images/psycheLocks/jfa_lock_appears.gif"));
    assert!(names.contains(&"defaults/images/psycheLocks/jfa_lock_explodes.gif"));
    assert!(names.contains(&"defaults/images/psycheLocks/fg_chains_disappear.gif"));
    assert!(locks.iter().all(|l| l.is_default));
}

#[test]
fn test_extract_no_psyche_locks_when_absent() {
    let data = json!({
        "scenes": [null, {
            "dialogues": [{"locks": null}]
        }]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let locks: Vec<_> = assets.iter().filter(|a| a.asset_type == "psyche_lock").collect();
    assert_eq!(locks.len(), 0);
}

// --- foreground objects ---

#[test]
fn test_extract_foreground_objects() {
    let data = json!({
        "places": [null, {
            "background": {"image": "Court", "external": false},
            "background_objects": [],
            "foreground_objects": [{"image": "http://i.imgur.com/fg.gif", "external": true}]
        }]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let fg: Vec<_> = assets.iter().filter(|a| a.asset_type == "foreground_object").collect();
    assert_eq!(fg.len(), 1);
    assert_eq!(fg[0].url, "http://i.imgur.com/fg.gif");
    assert!(fg[0].local_path.is_empty());
}

// --- sanitize_path applied to internal asset local_path ---

#[test]
fn test_internal_music_with_colon_gets_sanitized_local_path() {
    let data = json!({
        "music": [null, {"path": "Ace Attorney Investigations : Miles Edgeworth 2/117 Lamenting People", "external": false}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let music: Vec<_> = assets.iter().filter(|a| a.asset_type == "music").collect();
    assert_eq!(music.len(), 1);
    assert_eq!(
        music[0].local_path,
        "defaults/music/Ace Attorney Investigations _ Miles Edgeworth 2/117 Lamenting People.mp3"
    );
    assert!(music[0].url.contains("Investigations%20:%20Miles"));
}

// --- default sprite paths ---

#[test]
fn test_default_sprite_paths_correct_format() {
    let data = json!({
        "profiles": [null, {"base": "Apollo", "icon": "", "custom_sprites": []}],
        "frames": [null, {"characters": [
            {"profile_id": 1, "sprite_id": -5}
        ]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());

    let talking: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_talking").collect();
    assert_eq!(talking.len(), 1);
    assert_eq!(talking[0].local_path, "defaults/images/chars/Apollo/5.gif");
    assert!(talking[0].is_default);

    let still: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_still").collect();
    assert_eq!(still.len(), 1);
    assert_eq!(still[0].local_path, "defaults/images/charsStill/Apollo/5.gif");
    assert!(still[0].is_default);
}

// --- multiple sprites from same character ---

#[test]
fn test_extract_multiple_sprites_same_character() {
    let data = json!({
        "profiles": [null, {"base": "Phoenix", "icon": "", "custom_sprites": []}],
        "frames": [
            null,
            {"characters": [{"profile_id": 1, "sprite_id": -1}]},
            {"characters": [{"profile_id": 1, "sprite_id": -3}]},
            {"characters": [{"profile_id": 1, "sprite_id": -1}]}
        ]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let talking: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_talking").collect();
    assert_eq!(talking.len(), 2);
    let paths: Vec<&str> = talking.iter().map(|t| t.local_path.as_str()).collect();
    assert!(paths.contains(&"defaults/images/chars/Phoenix/1.gif"));
    assert!(paths.contains(&"defaults/images/chars/Phoenix/3.gif"));
}

// --- sprite extraction edge cases ---

#[test]
fn test_extract_sprites_skips_inconnu_base() {
    let data = json!({
        "profiles": [null, {"base": "Inconnu", "icon": "", "custom_sprites": []}],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -2}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let default_sprites: Vec<_> = assets.iter()
        .filter(|a| a.asset_type.starts_with("default_sprite"))
        .collect();
    assert!(default_sprites.is_empty(),
        "Profiles with base='Inconnu' should produce no default sprites, got {}",
        default_sprites.len());
}

#[test]
fn test_extract_sprites_skips_empty_base() {
    let data = json!({
        "profiles": [null, {"base": "", "icon": "", "custom_sprites": []}],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -1}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let default_sprites: Vec<_> = assets.iter()
        .filter(|a| a.asset_type.starts_with("default_sprite"))
        .collect();
    assert!(default_sprites.is_empty(),
        "Profiles with empty base should produce no default sprites, got {}",
        default_sprites.len());
}

#[test]
fn test_extract_sprites_no_startup_without_data() {
    let dir = tempfile::tempdir().unwrap();
    let data = json!({
        "profiles": [null, {"base": "Phoenix", "icon": "", "custom_sprites": []}],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -1}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), dir.path());
    let startup: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_startup")
        .collect();
    assert!(startup.is_empty(), "Without default_data.js, no startup sprites should be generated");
    let talking: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_talking")
        .collect();
    assert_eq!(talking.len(), 1, "Talking sprites should still be generated");
}

#[test]
fn test_extract_sprites_startup_only_for_matching_keys() {
    let dir = tempfile::tempdir().unwrap();
    let js_dir = dir.path().join("Javascript");
    std::fs::create_dir_all(&js_dir).unwrap();
    std::fs::write(
        js_dir.join("default_data.js"),
        r#"var default_profiles_nb = {"Phoenix": 20};
var default_profiles_startup = {"Phoenix/3": 880};"#,
    ).unwrap();

    let data = json!({
        "profiles": [null, {"base": "Phoenix", "icon": "", "custom_sprites": []}],
        "frames": [null,
            {"characters": [{"profile_id": 1, "sprite_id": -1}]},
            {"characters": [{"profile_id": 1, "sprite_id": -3}]}
        ]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), dir.path());
    let startup: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_startup")
        .collect();
    assert_eq!(startup.len(), 1);
    assert_eq!(startup[0].local_path, "defaults/images/charsStartup/Phoenix/3.gif");
}

#[test]
fn test_extract_no_sprites_for_custom_only() {
    let data = json!({
        "profiles": [null, {"base": "Phoenix", "icon": "", "custom_sprites": [
            {"talking": "http://example.com/t.gif", "still": "http://example.com/s.gif", "startup": ""}
        ]}],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": 1}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let default_sprites: Vec<_> = assets.iter()
        .filter(|a| a.asset_type.starts_with("default_sprite"))
        .collect();
    assert!(default_sprites.is_empty(),
        "Custom sprites (sprite_id >= 0) should not generate default sprites, got {}",
        default_sprites.len());
}

#[test]
fn test_extract_deduplicates_same_sprite_across_frames() {
    let data = json!({
        "profiles": [null, {"base": "Phoenix", "icon": "", "custom_sprites": []}],
        "frames": [
            null,
            {"characters": [{"profile_id": 1, "sprite_id": -2}]},
            {"characters": [{"profile_id": 1, "sprite_id": -2}]},
            {"characters": [{"profile_id": 1, "sprite_id": -2}]}
        ]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let talking: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_talking")
        .collect();
    let still: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_still")
        .collect();
    assert_eq!(talking.len(), 1, "Same sprite across frames should be deduplicated (talking)");
    assert_eq!(still.len(), 1, "Same sprite across frames should be deduplicated (still)");
    assert_eq!(talking[0].local_path, "defaults/images/chars/Phoenix/2.gif");
}

#[test]
fn test_extract_empty_trial_data() {
    let data = json!({});
    let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
    let non_voice: Vec<_> = assets.iter()
        .filter(|a| a.asset_type != "voice")
        .collect();
    assert!(non_voice.is_empty(),
        "Empty trial data should produce only voice assets, got {} non-voice assets: {:?}",
        non_voice.len(),
        non_voice.iter().map(|a| &a.asset_type).collect::<Vec<_>>());
    let voices: Vec<_> = assets.iter().filter(|a| a.asset_type == "voice").collect();
    assert_eq!(voices.len(), 9, "Should still have 3 voice IDs x 3 formats = 9 voice assets");
}

// --- phantom sprite bounds checking ---

#[test]
fn test_extract_sprites_skips_out_of_range_default_sprite() {
    let engine = temp_engine_dir(r#"{"TestChar": 5}"#, "{}");
    let data = json!({
        "profiles": [null, {"base": "TestChar", "icon": "", "custom_sprites": []}],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -8}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), engine.path());
    let talking: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_talking")
        .collect();
    let still: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_still")
        .collect();
    assert_eq!(talking.len(), 0, "Sprite -8 exceeds TestChar max of 5, should be skipped");
    assert_eq!(still.len(), 0, "Sprite -8 exceeds TestChar max of 5, should be skipped");
}

#[test]
fn test_extract_sprites_includes_in_range_default_sprite() {
    let engine = temp_engine_dir(r#"{"TestChar": 5}"#, "{}");
    let data = json!({
        "profiles": [null, {"base": "TestChar", "icon": "", "custom_sprites": []}],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -3}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), engine.path());
    let talking: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_talking")
        .collect();
    let still: Vec<_> = assets.iter()
        .filter(|a| a.asset_type == "default_sprite_still")
        .collect();
    assert_eq!(talking.len(), 1, "Sprite -3 is within TestChar max of 5, should be included");
    assert_eq!(still.len(), 1, "Sprite -3 is within TestChar max of 5, should be included");
}

#[test]
fn test_extract_sprites_skips_unknown_base_not_in_profiles_nb() {
    let engine = temp_engine_dir(r#"{"OtherChar": 10}"#, "{}");
    let data = json!({
        "profiles": [null, {"base": "UnknownChar", "icon": "", "custom_sprites": []}],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -1}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), engine.path());
    let default_sprites: Vec<_> = assets.iter()
        .filter(|a| a.asset_type.starts_with("default_sprite"))
        .collect();
    assert!(default_sprites.is_empty(),
        "Base 'UnknownChar' not in profiles_nb should produce no default sprites, got {}",
        default_sprites.len());
}

// --- custom sprite shadowing default sprite (same URL) ---

#[test]
fn test_custom_sprite_same_url_as_default_gets_default_path() {
    let engine = temp_engine_dir(r#"{"TestChar": 5}"#, "{}");
    let data = json!({
        "profiles": [null, {
            "base": "TestChar",
            "icon": "",
            "custom_sprites": [{
                "id": 1, "name": "pose",
                "talking": "Ressources/Images/persos/TestChar/1.gif",
                "still": "",
                "startup": ""
            }]
        }],
        "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -1}]}]
    });
    let assets = extract_asset_urls(&data, &test_site_paths(), engine.path());
    let talking_url_suffix = "persos/TestChar/1.gif";
    let matching: Vec<_> = assets.iter()
        .filter(|a| a.url.contains(talking_url_suffix))
        .collect();
    assert_eq!(matching.len(), 1, "Should have exactly 1 entry for TestChar/1.gif");
    assert!(!matching[0].local_path.is_empty(),
        "local_path should be upgraded to default path, got empty");
    assert!(matching[0].local_path.starts_with("defaults/"),
        "local_path should start with defaults/, got: {}",
        matching[0].local_path);
}
