use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

use super::super::{AssetRef, SitePaths};
use super::defaults::{parse_default_profiles_nb, parse_default_profiles_startup};
use super::helpers::*;

pub(super) fn extract_profiles(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    data: &Value,
    paths: &SitePaths,
) {
    let profiles = match data["profiles"].as_array() {
        Some(arr) => arr,
        None => return,
    };

    for (i, profile) in profiles.iter().enumerate() {
        if i == 0 || !profile.is_object() {
            continue;
        }

        let base = profile["base"].as_str().unwrap_or("Inconnu");
        let icon = profile["icon"].as_str().unwrap_or("");

        // Profile icon
        if icon.is_empty() {
            add_internal(
                assets, seen,
                &paths.icon_path(), LocalPaths::icon(),
                base, "png", "profile_icon", false,
            );
        } else {
            add_external(assets, seen, icon, "profile_icon");
        }

        // Custom sprites
        if let Some(custom_sprites) = profile["custom_sprites"].as_array() {
            for sprite in custom_sprites {
                if !sprite.is_object() {
                    continue;
                }
                for kind in &["talking", "still", "startup"] {
                    let url = sprite[*kind].as_str().unwrap_or("");
                    if !url.is_empty() {
                        add_external(assets, seen, url, &format!("custom_sprite_{}", kind));
                    }
                }
            }
        }
    }
}

pub(super) fn extract_evidence(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    data: &Value,
    paths: &SitePaths,
) {
    let evidence = match data["evidence"].as_array() {
        Some(arr) => arr,
        None => return,
    };

    for (i, ev) in evidence.iter().enumerate() {
        if i == 0 || !ev.is_object() {
            continue;
        }

        let icon = ev["icon"].as_str().unwrap_or("");
        let external = is_external(&ev["icon_external"]);

        if !icon.is_empty() {
            if external {
                add_external(assets, seen, icon, "evidence_icon");
            } else {
                add_internal(
                    assets, seen,
                    &paths.evidence_path(), LocalPaths::evidence(),
                    icon, "png", "evidence_icon", false,
                );
            }
        }

        // Check button data (images and sounds)
        if let Some(check_data) = ev["check_button_data"].as_array() {
            for item in check_data {
                let item_type = item["type"].as_str().unwrap_or("text");
                if item_type == "text" {
                    continue;
                }
                let content = item["content"].as_str().unwrap_or("");
                if !content.is_empty() {
                    add_external(assets, seen, content, "check_button_data");
                }
            }
        }
    }
}

pub(super) fn extract_places(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    data: &Value,
    paths: &SitePaths,
) {
    let places = match data["places"].as_array() {
        Some(arr) => arr,
        None => return,
    };

    for (i, place) in places.iter().enumerate() {
        if i == 0 || !place.is_object() {
            continue;
        }

        // Background
        if let Some(bg) = place.get("background") {
            let image = bg["image"].as_str().unwrap_or("");
            let external = is_external(&bg["external"]);

            if !image.is_empty() {
                if external {
                    add_external(assets, seen, image, "background");
                } else {
                    add_internal(
                        assets, seen,
                        &paths.bg_path(), LocalPaths::bg(),
                        image, "jpg", "background", false,
                    );
                }
            }
        }

        // Background objects
        if let Some(objects) = place["background_objects"].as_array() {
            for obj in objects {
                let image = obj["image"].as_str().unwrap_or("");
                if !image.is_empty() {
                    if is_external(&obj["external"]) {
                        add_external(assets, seen, image, "background_object");
                    } else {
                        // Internal background objects use the backgrounds directory
                        add_internal(
                            assets, seen,
                            &paths.bg_path(), LocalPaths::bg(),
                            image, "png", "background_object", false,
                        );
                    }
                }
            }
        }

        // Foreground objects
        if let Some(objects) = place["foreground_objects"].as_array() {
            for obj in objects {
                let image = obj["image"].as_str().unwrap_or("");
                if !image.is_empty() {
                    if is_external(&obj["external"]) {
                        add_external(assets, seen, image, "foreground_object");
                    } else {
                        add_internal(
                            assets, seen,
                            &paths.bg_path(), LocalPaths::bg(),
                            image, "png", "foreground_object", false,
                        );
                    }
                }
            }
        }
    }
}

pub(super) fn extract_music(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    data: &Value,
    paths: &SitePaths,
) {
    let music = match data["music"].as_array() {
        Some(arr) => arr,
        None => return,
    };

    for (i, track) in music.iter().enumerate() {
        if i == 0 || !track.is_object() {
            continue;
        }

        let path = track["path"].as_str().unwrap_or("");
        let external = is_external(&track["external"]);

        if !path.is_empty() {
            if external {
                add_external(assets, seen, path, "music");
            } else {
                add_internal(
                    assets, seen,
                    paths.music_path(), LocalPaths::music(),
                    path, "mp3", "music", false,
                );
            }
        }
    }
}

pub(super) fn extract_sounds(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    data: &Value,
    paths: &SitePaths,
) {
    let sounds = match data["sounds"].as_array() {
        Some(arr) => arr,
        None => return,
    };

    for (i, sound) in sounds.iter().enumerate() {
        if i == 0 || !sound.is_object() {
            continue;
        }

        let path = sound["path"].as_str().unwrap_or("");
        let external = is_external(&sound["external"]);

        if !path.is_empty() {
            if external {
                add_external(assets, seen, path, "sound");
            } else {
                add_internal(
                    assets, seen,
                    paths.sounds_path(), LocalPaths::sounds(),
                    path, "mp3", "sound", false,
                );
            }
        }
    }
}

pub(super) fn extract_popups(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    data: &Value,
    paths: &SitePaths,
) {
    let popups = match data["popups"].as_array() {
        Some(arr) => arr,
        None => return,
    };

    for (i, popup) in popups.iter().enumerate() {
        if i == 0 || !popup.is_object() {
            continue;
        }

        let path = popup["path"].as_str().unwrap_or("");
        let external = is_external(&popup["external"]);

        if !path.is_empty() {
            if external {
                add_external(assets, seen, path, "popup");
            } else {
                add_internal(
                    assets, seen,
                    &paths.popups_path(), LocalPaths::popups(),
                    path, "gif", "popup", false,
                );
            }
        }
    }
}

pub(super) fn extract_sprites_from_frames(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    data: &Value,
    paths: &SitePaths,
    engine_dir: &Path,
) {
    let frames = match data["frames"].as_array() {
        Some(arr) => arr,
        None => return,
    };
    let profiles = data["profiles"].as_array();

    // Load sprite count and startup animation lookup to avoid downloading non-existent sprites
    let profiles_nb = parse_default_profiles_nb(engine_dir);
    let profiles_startup = parse_default_profiles_startup(engine_dir);

    let mut used_sprites: HashSet<(i64, i64)> = HashSet::new();

    for (i, frame) in frames.iter().enumerate() {
        if i == 0 || !frame.is_object() {
            continue;
        }
        if let Some(characters) = frame["characters"].as_array() {
            for ch in characters {
                let profile_id = ch["profile_id"].as_i64().unwrap_or(0);
                let sprite_id = ch["sprite_id"].as_i64().unwrap_or(0);
                if profile_id != 0 && sprite_id != 0 {
                    used_sprites.insert((profile_id, sprite_id));
                }
            }
        }
    }

    for (profile_id, sprite_id) in &used_sprites {
        if *sprite_id >= 0 {
            continue; // Custom sprite, handled in extract_profiles
        }

        let base = if let Some(profs) = &profiles {
            let idx = *profile_id as usize;
            if idx < profs.len() {
                profs[idx]["base"].as_str().unwrap_or("Inconnu")
            } else {
                "Inconnu"
            }
        } else {
            "Inconnu"
        };

        // Skip placeholder character -- no real sprites on server
        if base == "Inconnu" || base.is_empty() {
            continue;
        }

        let sprite_num_val = (-sprite_id) as u32;

        // Skip sprites beyond the known count for this character
        // (matches AAO online preloader: j <= default_profiles_nb[profile.base])
        if !profiles_nb.is_empty() {
            match profiles_nb.get(base) {
                Some(&max) if sprite_num_val <= max => {} // in range, proceed
                _ => continue, // out of range or unknown base, skip
            }
        }

        let sprite_num = sprite_num_val.to_string();

        // Talking
        let server_path = format!("{}{}/{}.gif", paths.talking_path(), base, sprite_num);
        let local_path = format!("{}{}/{}.gif", LocalPaths::talking(), base, sprite_num);
        let url = build_url(&server_path);
        add_asset(assets, seen, url, "default_sprite_talking", true, local_path);

        // Still
        let server_path = format!("{}{}/{}.gif", paths.still_path(), base, sprite_num);
        let local_path = format!("{}{}/{}.gif", LocalPaths::still(), base, sprite_num);
        let url = build_url(&server_path);
        add_asset(assets, seen, url, "default_sprite_still", true, local_path);

        // Startup -- only if this base/sprite combo actually has one
        let startup_key = format!("{}/{}", base, sprite_num);
        if profiles_startup.contains(&startup_key) {
            let server_path = format!("{}{}/{}.gif", paths.startup_path(), base, sprite_num);
            let local_path = format!("{}{}/{}.gif", LocalPaths::startup(), base, sprite_num);
            let url = build_url(&server_path);
            add_asset(assets, seen, url, "default_sprite_startup", true, local_path);
        }
    }
}

pub(super) fn extract_voices(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    paths: &SitePaths,
) {
    for voice_id in 1..=3 {
        for ext in &["opus", "wav", "mp3"] {
            let server_path = format!("{}voice_singleblip_{}.{}", paths.voices_path(), voice_id, ext);
            let local_path = format!("{}voice_singleblip_{}.{}", LocalPaths::voices(), voice_id, ext);
            let url = build_url(&server_path);
            add_asset(assets, seen, url, "voice", true, local_path);
        }
    }
}

pub(super) fn extract_psyche_locks(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    data: &Value,
    paths: &SitePaths,
) {
    let scenes = match data["scenes"].as_array() {
        Some(arr) => arr,
        None => return,
    };

    let mut has_locks = false;
    for (i, scene) in scenes.iter().enumerate() {
        if i == 0 || !scene.is_object() {
            continue;
        }
        if let Some(dialogues) = scene["dialogues"].as_array() {
            for dialogue in dialogues {
                if let Some(locks) = dialogue.get("locks") {
                    if !locks.is_null() {
                        has_locks = true;
                        break;
                    }
                }
            }
        }
        if has_locks {
            break;
        }
    }

    if has_locks {
        let lock_names = [
            "fg_chains_appear",
            "jfa_lock_appears",
            "jfa_lock_explodes",
            "fg_chains_disappear",
        ];
        for name in &lock_names {
            let server_path = format!("{}{}.gif", paths.locks_path(), name);
            let local_path = format!("{}{}.gif", LocalPaths::locks(), name);
            let url = build_url(&server_path);
            add_asset(assets, seen, url, "psyche_lock", true, local_path);
        }
    }
}
