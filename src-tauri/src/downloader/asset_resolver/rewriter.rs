use serde_json::Value;
use std::collections::HashMap;

use super::super::asset_downloader::DownloadedAsset;
use super::super::AAONLINE_BASE;
use super::helpers::is_external;

/// Rewrite external asset URLs in trial_data to point to local downloaded files.
/// External assets are saved to `case/{case_id}/assets/{hash}`, so we replace
/// the original URLs in trial_data with server-relative paths.
pub fn rewrite_external_urls(
    trial_data: &mut Value,
    case_id: u32,
    downloaded: &[DownloadedAsset],
) {
    // Build mapping: original_url -> server-relative path
    let mut url_map: HashMap<String, String> = HashMap::new();
    for asset in downloaded {
        if asset.local_path.starts_with("assets/") {
            // External asset -- build a path the local server can resolve
            let server_path = crate::downloader::asset_paths::case_relative(case_id, &asset.local_path);
            url_map.insert(asset.original_url.clone(), server_path);
        } else if asset.local_path.starts_with("defaults/") {
            // Custom sprite URL was upgraded to default path -- rewrite to use default path directly
            url_map.insert(asset.original_url.clone(), asset.local_path.clone());
        }
    }

    if url_map.is_empty() {
        return;
    }

    // Lookup helper: try direct match, then try prepending base URL for relative paths
    let lookup = |val: &str| -> Option<String> {
        if let Some(local) = url_map.get(val) {
            return Some(local.clone());
        }
        if !val.starts_with("http://") && !val.starts_with("https://") {
            let full = format!("{}/{}", AAONLINE_BASE, val);
            if let Some(local) = url_map.get(&full) {
                return Some(local.clone());
            }
        }
        None
    };

    // Rewrite profiles
    if let Some(profiles) = trial_data["profiles"].as_array_mut() {
        for (i, profile) in profiles.iter_mut().enumerate() {
            if i == 0 { continue; }
            // Profile icon (non-empty icon = external URL)
            if let Some(icon) = profile.get("icon").and_then(|v| v.as_str()).map(String::from) {
                if let Some(local) = lookup(&icon) {
                    profile["icon"] = Value::String(local);
                }
            }
            // Custom sprites
            if let Some(sprites) = profile.get_mut("custom_sprites").and_then(|v| v.as_array_mut()) {
                for sprite in sprites.iter_mut() {
                    for field in &["talking", "still", "startup"] {
                        if let Some(url) = sprite.get(*field).and_then(|v| v.as_str()).map(String::from) {
                            if let Some(local) = lookup(&url) {
                                sprite[*field] = Value::String(local);
                            }
                        }
                    }
                }
            }
        }
    }

    // Rewrite evidence
    if let Some(evidence) = trial_data["evidence"].as_array_mut() {
        for (i, ev) in evidence.iter_mut().enumerate() {
            if i == 0 { continue; }
            if is_external(&ev["icon_external"]) {
                if let Some(icon) = ev.get("icon").and_then(|v| v.as_str()).map(String::from) {
                    if let Some(local) = lookup(&icon) {
                        ev["icon"] = Value::String(local);
                    }
                }
            }
            // Check button data
            if let Some(items) = ev.get_mut("check_button_data").and_then(|v| v.as_array_mut()) {
                for item in items.iter_mut() {
                    let item_type = item["type"].as_str().unwrap_or("text");
                    if item_type != "text" {
                        if let Some(content) = item.get("content").and_then(|v| v.as_str()).map(String::from) {
                            if let Some(local) = lookup(&content) {
                                item["content"] = Value::String(local);
                            }
                        }
                    }
                }
            }
        }
    }

    // Rewrite places (backgrounds, background_objects, foreground_objects)
    if let Some(places) = trial_data["places"].as_array_mut() {
        for (i, place) in places.iter_mut().enumerate() {
            if i == 0 { continue; }
            if let Some(bg) = place.get_mut("background") {
                if is_external(&bg["external"]) {
                    if let Some(image) = bg.get("image").and_then(|v| v.as_str()).map(String::from) {
                        if let Some(local) = lookup(&image) {
                            bg["image"] = Value::String(local);
                        }
                    }
                }
            }
            for obj_key in &["background_objects", "foreground_objects"] {
                if let Some(objects) = place.get_mut(*obj_key).and_then(|v| v.as_array_mut()) {
                    for obj in objects.iter_mut() {
                        if is_external(&obj["external"]) {
                            if let Some(image) = obj.get("image").and_then(|v| v.as_str()).map(String::from) {
                                if let Some(local) = lookup(&image) {
                                    obj["image"] = Value::String(local);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Rewrite music
    if let Some(music) = trial_data["music"].as_array_mut() {
        for (i, track) in music.iter_mut().enumerate() {
            if i == 0 { continue; }
            if is_external(&track["external"]) {
                if let Some(path) = track.get("path").and_then(|v| v.as_str()).map(String::from) {
                    if let Some(local) = lookup(&path) {
                        track["path"] = Value::String(local);
                    }
                }
            }
        }
    }

    // Rewrite sounds
    if let Some(sounds) = trial_data["sounds"].as_array_mut() {
        for (i, sound) in sounds.iter_mut().enumerate() {
            if i == 0 { continue; }
            if is_external(&sound["external"]) {
                if let Some(path) = sound.get("path").and_then(|v| v.as_str()).map(String::from) {
                    if let Some(local) = lookup(&path) {
                        sound["path"] = Value::String(local);
                    }
                }
            }
        }
    }

    // Rewrite popups
    if let Some(popups) = trial_data["popups"].as_array_mut() {
        for (i, popup) in popups.iter_mut().enumerate() {
            if i == 0 { continue; }
            if is_external(&popup["external"]) {
                if let Some(path) = popup.get("path").and_then(|v| v.as_str()).map(String::from) {
                    if let Some(local) = lookup(&path) {
                        popup["path"] = Value::String(local);
                    }
                }
            }
        }
    }
}
