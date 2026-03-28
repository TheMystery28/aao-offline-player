use super::*;

#[test]
fn test_rewrite_custom_sprites() {
    let mut data = json!({
        "profiles": [null, {
            "base": "Phoenix",
            "icon": "http://i.imgur.com/icon.png",
            "custom_sprites": [{
                "talking": "http://i.imgur.com/talk.gif",
                "still": "http://i.imgur.com/still.gif",
                "startup": ""
            }]
        }]
    });
    let downloaded = vec![
        DownloadedAsset { original_url: "http://i.imgur.com/icon.png".into(), local_path: "assets/icon-abc.png".into(), size: 100, content_hash: 0 },
        DownloadedAsset { original_url: "http://i.imgur.com/talk.gif".into(), local_path: "assets/talk-def.gif".into(), size: 200, content_hash: 0 },
        DownloadedAsset { original_url: "http://i.imgur.com/still.gif".into(), local_path: "assets/still-ghi.gif".into(), size: 300, content_hash: 0 },
    ];
    rewrite_external_urls(&mut data, 123, &downloaded);
    assert_eq!(data["profiles"][1]["icon"], "case/123/assets/icon-abc.png");
    assert_eq!(data["profiles"][1]["custom_sprites"][0]["talking"], "case/123/assets/talk-def.gif");
    assert_eq!(data["profiles"][1]["custom_sprites"][0]["still"], "case/123/assets/still-ghi.gif");
    assert_eq!(data["profiles"][1]["custom_sprites"][0]["startup"], "");
}

#[test]
fn test_rewrite_external_backgrounds() {
    let mut data = json!({
        "places": [null, {
            "background": {"image": "http://i.imgur.com/bg.png", "external": true},
            "background_objects": [{"image": "http://i.imgur.com/obj.gif", "external": true}],
            "foreground_objects": []
        }]
    });
    let downloaded = vec![
        DownloadedAsset { original_url: "http://i.imgur.com/bg.png".into(), local_path: "assets/bg-abc.png".into(), size: 100, content_hash: 0 },
        DownloadedAsset { original_url: "http://i.imgur.com/obj.gif".into(), local_path: "assets/obj-def.gif".into(), size: 200, content_hash: 0 },
    ];
    rewrite_external_urls(&mut data, 42, &downloaded);
    assert_eq!(data["places"][1]["background"]["image"], "case/42/assets/bg-abc.png");
    assert_eq!(data["places"][1]["background_objects"][0]["image"], "case/42/assets/obj-def.gif");
}

#[test]
fn test_rewrite_does_not_touch_internal() {
    let mut data = json!({
        "places": [null, {
            "background": {"image": "Court", "external": false},
            "background_objects": [],
            "foreground_objects": []
        }]
    });
    let downloaded = vec![];
    rewrite_external_urls(&mut data, 1, &downloaded);
    assert_eq!(data["places"][1]["background"]["image"], "Court");
}

#[test]
fn test_rewrite_music_sounds_popups() {
    let mut data = json!({
        "music": [null, {"path": "http://example.com/song.mp3", "external": true}],
        "sounds": [null, {"path": "http://example.com/sfx.mp3", "external": true}],
        "popups": [null, {"path": "http://example.com/popup.gif", "external": true}]
    });
    let downloaded = vec![
        DownloadedAsset { original_url: "http://example.com/song.mp3".into(), local_path: "assets/song-a.mp3".into(), size: 100, content_hash: 0 },
        DownloadedAsset { original_url: "http://example.com/sfx.mp3".into(), local_path: "assets/sfx-b.mp3".into(), size: 200, content_hash: 0 },
        DownloadedAsset { original_url: "http://example.com/popup.gif".into(), local_path: "assets/popup-c.gif".into(), size: 300, content_hash: 0 },
    ];
    rewrite_external_urls(&mut data, 99, &downloaded);
    assert_eq!(data["music"][1]["path"], "case/99/assets/song-a.mp3");
    assert_eq!(data["sounds"][1]["path"], "case/99/assets/sfx-b.mp3");
    assert_eq!(data["popups"][1]["path"], "case/99/assets/popup-c.gif");
}

#[test]
fn test_rewrite_evidence_external() {
    let mut data = json!({
        "evidence": [null, {
            "icon": "http://i.imgur.com/ev.png",
            "icon_external": true,
            "check_button_data": [
                {"type": "text", "content": "hello"},
                {"type": "image", "content": "http://i.imgur.com/check.png"}
            ]
        }]
    });
    let downloaded = vec![
        DownloadedAsset { original_url: "http://i.imgur.com/ev.png".into(), local_path: "assets/ev-a.png".into(), size: 100, content_hash: 0 },
        DownloadedAsset { original_url: "http://i.imgur.com/check.png".into(), local_path: "assets/check-b.png".into(), size: 200, content_hash: 0 },
    ];
    rewrite_external_urls(&mut data, 5, &downloaded);
    assert_eq!(data["evidence"][1]["icon"], "case/5/assets/ev-a.png");
    assert_eq!(data["evidence"][1]["check_button_data"][0]["content"], "hello");
    assert_eq!(data["evidence"][1]["check_button_data"][1]["content"], "case/5/assets/check-b.png");
}

#[test]
fn test_rewrite_skips_internal_assets() {
    let mut data = json!({
        "music": [null, {"path": "Ace Attorney 1/Theme", "external": false}]
    });
    let downloaded = vec![
        DownloadedAsset {
            original_url: "https://aaonline.fr/Ressources/Musiques/Ace Attorney 1/Theme.mp3".into(),
            local_path: "defaults/music/Ace Attorney 1/Theme.mp3".into(),
            size: 500,
            content_hash: 0,
        },
    ];
    rewrite_external_urls(&mut data, 1, &downloaded);
    assert_eq!(data["music"][1]["path"], "Ace Attorney 1/Theme");
}

#[test]
fn test_rewrite_foreground_objects() {
    let mut data = json!({
        "places": [null, {
            "background": {"image": "Court", "external": false},
            "background_objects": [],
            "foreground_objects": [{"image": "http://i.imgur.com/fg.gif", "external": true}]
        }]
    });
    let downloaded = vec![
        DownloadedAsset { original_url: "http://i.imgur.com/fg.gif".into(), local_path: "assets/fg-abc.gif".into(), size: 100, content_hash: 0 },
    ];
    rewrite_external_urls(&mut data, 7, &downloaded);
    assert_eq!(data["places"][1]["foreground_objects"][0]["image"], "case/7/assets/fg-abc.gif");
}

#[test]
fn test_rewrite_url_map_only_includes_external_assets() {
    let downloaded = vec![
        DownloadedAsset {
            original_url: "http://i.imgur.com/ext.png".into(),
            local_path: "assets/ext-hash.png".into(),
            size: 100,
            content_hash: 0,
        },
        DownloadedAsset {
            original_url: "https://aaonline.fr/Ressources/Images/persos/Phoenix.png".into(),
            local_path: "defaults/images/chars/Phoenix.png".into(),
            size: 200,
            content_hash: 0,
        },
    ];
    let mut data = json!({
        "profiles": [null, {
            "base": "Phoenix",
            "icon": "http://i.imgur.com/ext.png",
            "custom_sprites": []
        }]
    });
    rewrite_external_urls(&mut data, 42, &downloaded);
    assert_eq!(data["profiles"][1]["icon"], "case/42/assets/ext-hash.png");
}

#[test]
fn test_rewrite_external_urls_handles_default_path() {
    let mut data = json!({
        "profiles": [null, {
            "base": "Olga",
            "icon": "",
            "custom_sprites": [{
                "talking": "http://example.com/sprite.gif",
                "still": "",
                "startup": ""
            }]
        }]
    });
    let downloaded = vec![
        DownloadedAsset {
            original_url: "http://example.com/sprite.gif".into(),
            local_path: "defaults/images/chars/Olga/1.gif".into(),
            size: 1000,
            content_hash: 0,
        },
    ];
    rewrite_external_urls(&mut data, 99, &downloaded);
    assert_eq!(
        data["profiles"][1]["custom_sprites"][0]["talking"],
        "defaults/images/chars/Olga/1.gif",
        "Custom sprite URL should be rewritten to default path"
    );
}
