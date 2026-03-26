use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::asset_downloader::DownloadedAsset;
use super::{AssetRef, SitePaths, AAONLINE_BASE};

/// Offline cfg paths (must match engine/bridge.js cfg).
struct LocalPaths;
impl LocalPaths {
    fn icon() -> &'static str { "defaults/images/chars/" }
    fn talking() -> &'static str { "defaults/images/chars/" }
    fn still() -> &'static str { "defaults/images/charsStill/" }
    fn startup() -> &'static str { "defaults/images/charsStartup/" }
    fn evidence() -> &'static str { "defaults/images/evidence/" }
    fn bg() -> &'static str { "defaults/images/backgrounds/" }
    fn defaultplaces_bg() -> &'static str { "defaults/images/defaultplaces/backgrounds/" }
    fn defaultplaces_fg() -> &'static str { "defaults/images/defaultplaces/foreground_objects/" }
    fn popups() -> &'static str { "defaults/images/popups/" }
    fn locks() -> &'static str { "defaults/images/psycheLocks/" }
    fn music() -> &'static str { "defaults/music/" }
    fn sounds() -> &'static str { "defaults/sounds/" }
    fn voices() -> &'static str { "defaults/voices/" }
}

fn build_url(path: &str) -> String {
    // Use url::Url::join for proper path joining and percent-encoding
    match url::Url::parse(AAONLINE_BASE) {
        Ok(base) => {
            // Ensure base has trailing slash for proper joining
            let base_str = if base.as_str().ends_with('/') {
                base.to_string()
            } else {
                format!("{}/", base)
            };
            match url::Url::parse(&base_str).and_then(|b| b.join(path)) {
                Ok(u) => u.to_string(),
                Err(_) => format!("{}/{}", AAONLINE_BASE, path),
            }
        }
        Err(_) => format!("{}/{}", AAONLINE_BASE, path),
    }
}

/// Sanitize a path for Windows by replacing illegal characters.
/// Must match the sanitization applied in server.rs for path resolution.
pub fn sanitize_path(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Add an asset with a local_path (for internal assets) or empty local_path (for external).
fn add_asset(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    url: String,
    asset_type: &str,
    is_default: bool,
    local_path: String,
) {
    if url.is_empty() {
        return;
    }
    if seen.contains(&url) {
        // Upgrade: if existing entry is external (no local_path) but this one has a default path,
        // update the existing entry so the file gets saved to the correct default location.
        // Same URL = same bytes, so using the default path avoids file duplication.
        if !local_path.is_empty() {
            if let Some(existing) = assets.iter_mut().find(|a| a.url == url && a.local_path.is_empty()) {
                existing.local_path = local_path;
                existing.is_default = is_default;
            }
        }
        return;
    }
    seen.insert(url.clone());
    assets.push(AssetRef {
        url,
        asset_type: asset_type.to_string(),
        is_default,
        local_path,
    });
}

/// Add a non-external asset from AAO's library.
/// `server_dir` = the AAO server path (for building download URL).
/// `local_dir` = the offline bridge.js cfg path (for local save path).
fn add_internal(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    server_dir: &str,
    local_dir: &str,
    name: &str,
    default_ext: &str,
    asset_type: &str,
    is_default: bool,
) {
    if name.is_empty() {
        return;
    }
    let has_ext = name.contains('.');
    let filename = if has_ext {
        name.to_string()
    } else {
        format!("{}.{}", name, default_ext)
    };
    let url = build_url(&format!("{}{}", server_dir, filename));
    let local_path = sanitize_path(&format!("{}{}", local_dir, filename));
    add_asset(assets, seen, url, asset_type, is_default, local_path);
}

/// Add an external asset (full URL or relative path).
/// External assets have no local_path — they get hashed filenames.
fn add_external(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    url_or_path: &str,
    asset_type: &str,
) {
    if url_or_path.is_empty() {
        return;
    }
    let full_url = if url_or_path.starts_with("http://") || url_or_path.starts_with("https://") {
        url_or_path.to_string()
    } else {
        build_url(url_or_path)
    };
    add_asset(assets, seen, full_url, asset_type, false, String::new());
}

fn is_external(val: &Value) -> bool {
    val.as_bool()
        .or_else(|| val.as_i64().map(|n| n != 0))
        .unwrap_or(false)
}

/// Extract all asset URLs from trial data.
pub fn extract_asset_urls(trial_data: &Value, site_paths: &SitePaths, engine_dir: &Path) -> Vec<AssetRef> {
    let mut assets: Vec<AssetRef> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    extract_profiles(&mut assets, &mut seen, trial_data, site_paths);
    extract_evidence(&mut assets, &mut seen, trial_data, site_paths);
    extract_places(&mut assets, &mut seen, trial_data, site_paths);
    extract_music(&mut assets, &mut seen, trial_data, site_paths);
    extract_sounds(&mut assets, &mut seen, trial_data, site_paths);
    extract_popups(&mut assets, &mut seen, trial_data, site_paths);
    extract_sprites_from_frames(&mut assets, &mut seen, trial_data, site_paths, engine_dir);
    extract_voices(&mut assets, &mut seen, site_paths);
    extract_psyche_locks(&mut assets, &mut seen, trial_data, site_paths);

    assets
}

/// Classify assets into case-specific and shared/default.
pub fn classify_assets(assets: Vec<AssetRef>) -> (Vec<AssetRef>, Vec<AssetRef>) {
    let mut case_specific = Vec::new();
    let mut shared = Vec::new();

    for asset in assets {
        if asset.is_default {
            shared.push(asset);
        } else {
            case_specific.push(asset);
        }
    }

    (case_specific, shared)
}

// --- Extraction functions ---

fn extract_profiles(
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

fn extract_evidence(
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

fn extract_places(
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

fn extract_music(
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
                    &paths.music_dir, LocalPaths::music(),
                    path, "mp3", "music", false,
                );
            }
        }
    }
}

fn extract_sounds(
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
                    &paths.sounds_dir, LocalPaths::sounds(),
                    path, "mp3", "sound", false,
                );
            }
        }
    }
}

fn extract_popups(
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

fn extract_sprites_from_frames(
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

        // Skip placeholder character — no real sprites on server
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

        // Startup — only if this base/sprite combo actually has one
        let startup_key = format!("{}/{}", base, sprite_num);
        if profiles_startup.contains(&startup_key) {
            let server_path = format!("{}{}/{}.gif", paths.startup_path(), base, sprite_num);
            let local_path = format!("{}{}/{}.gif", LocalPaths::startup(), base, sprite_num);
            let url = build_url(&server_path);
            add_asset(assets, seen, url, "default_sprite_startup", true, local_path);
        }
    }
}

fn extract_voices(
    assets: &mut Vec<AssetRef>,
    seen: &mut HashSet<String>,
    paths: &SitePaths,
) {
    for voice_id in 1..=3 {
        for ext in &["opus", "wav", "mp3"] {
            let server_path = format!("{}voice_singleblip_{}.{}", paths.voices_dir, voice_id, ext);
            let local_path = format!("{}voice_singleblip_{}.{}", LocalPaths::voices(), voice_id, ext);
            let url = build_url(&server_path);
            add_asset(assets, seen, url, "voice", true, local_path);
        }
    }
}

fn extract_psyche_locks(
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

/// Rewrite external asset URLs in trial_data to point to local downloaded files.
/// External assets are saved to `case/{case_id}/assets/{hash}`, so we replace
/// the original URLs in trial_data with server-relative paths.
pub fn rewrite_external_urls(
    trial_data: &mut Value,
    case_id: u32,
    downloaded: &[DownloadedAsset],
) {
    // Build mapping: original_url → server-relative path
    let mut url_map: HashMap<String, String> = HashMap::new();
    for asset in downloaded {
        if asset.local_path.starts_with("assets/") {
            // External asset — build a path the local server can resolve
            let server_path = format!("case/{}/{}", case_id, asset.local_path);
            url_map.insert(asset.original_url.clone(), server_path);
        } else if asset.local_path.starts_with("defaults/") {
            // Custom sprite URL was upgraded to default path — rewrite to use default path directly
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

/// Parse `default_profiles_nb` from engine/Javascript/default_data.js.
/// Returns a map of character base name → number of default sprites.
fn parse_default_profiles_nb(engine_dir: &Path) -> HashMap<String, u32> {
    let path = engine_dir.join("Javascript").join("default_data.js");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    parse_js_object_assignment(&content, "default_profiles_nb")
        .and_then(|json_str| serde_json::from_str::<HashMap<String, u32>>(&json_str).ok())
        .unwrap_or_default()
}

/// Parse `default_profiles_startup` from engine/Javascript/default_data.js.
/// Returns the set of "Base/index" keys that have startup animations.
fn parse_default_profiles_startup(engine_dir: &Path) -> HashSet<String> {
    let path = engine_dir.join("Javascript").join("default_data.js");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashSet::new(),
    };

    parse_js_object_assignment(&content, "default_profiles_startup")
        .and_then(|json_str| serde_json::from_str::<HashMap<String, Value>>(&json_str).ok())
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default()
}

/// Extract the JSON object from a `var name = {...};` assignment in JS source.
fn parse_js_object_assignment(source: &str, var_name: &str) -> Option<String> {
    let prefix = format!("var {} = ", var_name);
    let start = source.find(&prefix)?;
    let rest = &source[start + prefix.len()..];
    // Find the matching closing brace
    let obj_start = rest.find('{')?;
    let mut depth = 0;
    for (i, ch) in rest[obj_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(rest[obj_start..obj_start + i + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract default sprite assets for characters used in the trial.
/// Reads `default_profiles_nb` and `default_profiles_startup` from `default_data.js`
/// to know how many sprites each default character has, then generates download URLs
/// for every default sprite of every character base referenced in `trial_data.profiles`.
pub fn extract_default_sprite_assets(
    trial_data: &Value,
    site_paths: &SitePaths,
    engine_dir: &Path,
) -> Vec<AssetRef> {
    let profiles_nb = parse_default_profiles_nb(engine_dir);
    let profiles_startup = parse_default_profiles_startup(engine_dir);

    if profiles_nb.is_empty() {
        return Vec::new();
    }

    let mut assets: Vec<AssetRef> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Collect unique character bases from trial_data profiles
    let profiles = match trial_data["profiles"].as_array() {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut bases_seen: HashSet<String> = HashSet::new();
    for (i, profile) in profiles.iter().enumerate() {
        if i == 0 || !profile.is_object() {
            continue;
        }
        let base = profile["base"].as_str().unwrap_or("Inconnu");
        if base == "Inconnu" || base.is_empty() {
            continue;
        }
        bases_seen.insert(base.to_string());
    }

    // For each unique base with default sprites, generate download URLs
    for base in &bases_seen {
        let count = match profiles_nb.get(base.as_str()) {
            Some(&n) if n > 0 => n,
            _ => continue,
        };

        for j in 1..=count {
            let sprite_name = format!("{}/{}.gif", base, j);

            // Talking sprite
            add_internal(
                &mut assets, &mut seen,
                &site_paths.talking_path(), LocalPaths::talking(),
                &sprite_name, "gif", "default_sprite_talking", true,
            );

            // Still sprite
            add_internal(
                &mut assets, &mut seen,
                &site_paths.still_path(), LocalPaths::still(),
                &sprite_name, "gif", "default_sprite_still", true,
            );

            // Startup sprite (only if this base/index has one)
            let startup_key = format!("{}/{}", base, j);
            if profiles_startup.contains(&startup_key) {
                add_internal(
                    &mut assets, &mut seen,
                    &site_paths.startup_path(), LocalPaths::startup(),
                    &sprite_name, "gif", "default_sprite_startup", true,
                );
            }
        }
    }

    assets
}

/// Extract default place assets (backgrounds and foreground objects) from `default_data.js`.
///
/// Default places (courtrooms, lobbies, detention centers, etc.) are built-in to the player
/// with negative IDs. Their images are NOT in the trial data — they're defined in
/// `default_data.js` with paths like `defaults/images/defaultplaces/backgrounds/pw_courtroom.jpg`.
/// The downloader must fetch these from the AAO server and store them locally.
pub fn extract_default_place_assets(
    engine_dir: &Path,
    site_paths: &SitePaths,
) -> Vec<AssetRef> {
    let path = engine_dir.join("Javascript").join("default_data.js");
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut assets: Vec<AssetRef> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Extract all image paths from the default_places variable.
    // Paths look like: defaults/images/defaultplaces/backgrounds/pw_courtroom.jpg
    //             or:  defaults/images/defaultplaces/foreground_objects/pw_courtroom_benches.gif
    let re = regex::Regex::new(
        r#"defaults/images/defaultplaces/(backgrounds|foreground_objects)/([^"]+)"#,
    ).unwrap();

    let dp_server_bg = format!("{}backgrounds/", site_paths.defaultplaces_path());
    let dp_server_fg = format!("{}foreground_objects/", site_paths.defaultplaces_path());

    for cap in re.captures_iter(&content) {
        let category = &cap[1]; // "backgrounds" or "foreground_objects"
        let filename = &cap[2]; // e.g., "pw_courtroom.jpg"

        let (server_dir, local_dir, asset_type) = match category {
            "backgrounds" => (&dp_server_bg, LocalPaths::defaultplaces_bg(), "default_place_bg"),
            "foreground_objects" => (&dp_server_fg, LocalPaths::defaultplaces_fg(), "default_place_fg"),
            _ => continue,
        };

        add_internal(
            &mut assets, &mut seen,
            server_dir, local_dir,
            filename, "", asset_type, true,
        );
    }

    assets
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    /// Dummy engine dir with no default_data.js — startup sprites won't be generated.
    fn test_engine_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test_dummy_engine")
    }

    fn test_site_paths() -> SitePaths {
        SitePaths {
            picture_dir: "Ressources/Images/".to_string(),
            icon_subdir: "persos/".to_string(),
            talking_subdir: "persos/".to_string(),
            still_subdir: "persos_static/".to_string(),
            startup_subdir: "persos_startup/".to_string(),
            evidence_subdir: "dossier/".to_string(),
            bg_subdir: "cinematiques/".to_string(),
            defaultplaces_subdir: "lieux/".to_string(),
            popups_subdir: "persos/Cour/".to_string(),
            locks_subdir: "persos/Cour/psyche_locks/".to_string(),
            music_dir: "Ressources/Musiques/".to_string(),
            sounds_dir: "Ressources/Sons/".to_string(),
            voices_dir: "Ressources/Voix/".to_string(),
        }
    }

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

    // --- extract_asset_urls ---

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
        assert!(icons[0].local_path.is_empty()); // external → empty local_path
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
        // Colon should be sanitized to underscore in local_path
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
        // 3 voice IDs × 3 formats = 9
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
        assert_eq!(icons.len(), 1); // Deduplicated
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

    // --- rewrite_external_urls ---

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
            DownloadedAsset { original_url: "http://i.imgur.com/icon.png".into(), local_path: "assets/icon-abc.png".into(), size: 100 },
            DownloadedAsset { original_url: "http://i.imgur.com/talk.gif".into(), local_path: "assets/talk-def.gif".into(), size: 200 },
            DownloadedAsset { original_url: "http://i.imgur.com/still.gif".into(), local_path: "assets/still-ghi.gif".into(), size: 300 },
        ];
        rewrite_external_urls(&mut data, 123, &downloaded);
        assert_eq!(data["profiles"][1]["icon"], "case/123/assets/icon-abc.png");
        assert_eq!(data["profiles"][1]["custom_sprites"][0]["talking"], "case/123/assets/talk-def.gif");
        assert_eq!(data["profiles"][1]["custom_sprites"][0]["still"], "case/123/assets/still-ghi.gif");
        assert_eq!(data["profiles"][1]["custom_sprites"][0]["startup"], ""); // empty stays empty
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
            DownloadedAsset { original_url: "http://i.imgur.com/bg.png".into(), local_path: "assets/bg-abc.png".into(), size: 100 },
            DownloadedAsset { original_url: "http://i.imgur.com/obj.gif".into(), local_path: "assets/obj-def.gif".into(), size: 200 },
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
        // Internal background image untouched
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
            DownloadedAsset { original_url: "http://example.com/song.mp3".into(), local_path: "assets/song-a.mp3".into(), size: 100 },
            DownloadedAsset { original_url: "http://example.com/sfx.mp3".into(), local_path: "assets/sfx-b.mp3".into(), size: 200 },
            DownloadedAsset { original_url: "http://example.com/popup.gif".into(), local_path: "assets/popup-c.gif".into(), size: 300 },
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
            DownloadedAsset { original_url: "http://i.imgur.com/ev.png".into(), local_path: "assets/ev-a.png".into(), size: 100 },
            DownloadedAsset { original_url: "http://i.imgur.com/check.png".into(), local_path: "assets/check-b.png".into(), size: 200 },
        ];
        rewrite_external_urls(&mut data, 5, &downloaded);
        assert_eq!(data["evidence"][1]["icon"], "case/5/assets/ev-a.png");
        // Text content should be untouched
        assert_eq!(data["evidence"][1]["check_button_data"][0]["content"], "hello");
        assert_eq!(data["evidence"][1]["check_button_data"][1]["content"], "case/5/assets/check-b.png");
    }

    #[test]
    fn test_rewrite_skips_internal_assets() {
        let mut data = json!({
            "music": [null, {"path": "Ace Attorney 1/Theme", "external": false}]
        });
        // Downloaded asset has local_path starting with "defaults/" (not "assets/")
        let downloaded = vec![
            DownloadedAsset {
                original_url: "https://aaonline.fr/Ressources/Musiques/Ace Attorney 1/Theme.mp3".into(),
                local_path: "defaults/music/Ace Attorney 1/Theme.mp3".into(),
                size: 500,
            },
        ];
        rewrite_external_urls(&mut data, 1, &downloaded);
        // Internal music path should NOT be rewritten
        assert_eq!(data["music"][1]["path"], "Ace Attorney 1/Theme");
    }

    // --- Regression: psyche lock extraction ---

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

    // --- Regression: foreground object extraction and rewriting ---

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
        assert!(fg[0].local_path.is_empty()); // external
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
            DownloadedAsset { original_url: "http://i.imgur.com/fg.gif".into(), local_path: "assets/fg-abc.gif".into(), size: 100 },
        ];
        rewrite_external_urls(&mut data, 7, &downloaded);
        assert_eq!(data["places"][1]["foreground_objects"][0]["image"], "case/7/assets/fg-abc.gif");
    }

    // --- Regression: sanitize_path applied to internal asset local_path ---

    #[test]
    fn test_internal_music_with_colon_gets_sanitized_local_path() {
        let data = json!({
            "music": [null, {"path": "Ace Attorney Investigations : Miles Edgeworth 2/117 Lamenting People", "external": false}]
        });
        let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
        let music: Vec<_> = assets.iter().filter(|a| a.asset_type == "music").collect();
        assert_eq!(music.len(), 1);
        // local_path must have colon replaced (Windows compat)
        assert_eq!(
            music[0].local_path,
            "defaults/music/Ace Attorney Investigations _ Miles Edgeworth 2/117 Lamenting People.mp3"
        );
        // URL keeps the colon (valid in path segments) but spaces are %20-encoded
        assert!(music[0].url.contains("Investigations%20:%20Miles"));
    }

    // --- Regression: default sprite paths ---

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

    // --- Regression: rewrite only touches assets/ prefix, not defaults/ ---

    #[test]
    fn test_rewrite_url_map_only_includes_external_assets() {
        let downloaded = vec![
            DownloadedAsset {
                original_url: "http://i.imgur.com/ext.png".into(),
                local_path: "assets/ext-hash.png".into(),
                size: 100,
            },
            DownloadedAsset {
                original_url: "https://aaonline.fr/Ressources/Images/persos/Phoenix.png".into(),
                local_path: "defaults/images/chars/Phoenix.png".into(),
                size: 200,
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
        // External icon rewritten
        assert_eq!(data["profiles"][1]["icon"], "case/42/assets/ext-hash.png");
    }

    // --- Regression: default assets should be downloaded only if missing on disk ---

    #[test]
    fn test_classify_then_filter_missing_defaults() {
        let assets = vec![
            AssetRef { url: "http://a.com/bg.jpg".into(), asset_type: "bg".into(), is_default: false, local_path: "defaults/images/backgrounds/Court.jpg".into() },
            AssetRef { url: "http://a.com/sprite.gif".into(), asset_type: "sprite".into(), is_default: true, local_path: "defaults/images/chars/Phoenix/1.gif".into() },
            AssetRef { url: "http://a.com/voice.opus".into(), asset_type: "voice".into(), is_default: true, local_path: "defaults/voices/voice_singleblip_1.opus".into() },
        ];
        let (case_specific, shared) = classify_assets(assets);

        // Simulate the filter from lib.rs: keep only defaults whose file doesn't exist
        let engine_dir = std::path::PathBuf::from("/nonexistent/engine");
        let missing: Vec<_> = shared
            .into_iter()
            .filter(|a| !a.local_path.is_empty() && !engine_dir.join(&a.local_path).exists())
            .collect();

        // Both defaults are "missing" since engine_dir doesn't exist
        assert_eq!(case_specific.len(), 1);
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn test_filter_skips_existing_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path().to_path_buf();

        // Create one default file on disk
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

        // Only v2 is missing; v1 exists on disk
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].local_path, "defaults/voices/voice_singleblip_2.opus");
    }

    // --- Regression: multiple sprites from same character in different frames ---

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
        // Should deduplicate: sprite -1 and -3 = 2 unique talking sprites
        assert_eq!(talking.len(), 2);
        let paths: Vec<&str> = talking.iter().map(|t| t.local_path.as_str()).collect();
        assert!(paths.contains(&"defaults/images/chars/Phoenix/1.gif"));
        assert!(paths.contains(&"defaults/images/chars/Phoenix/3.gif"));
    }

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
        // Create a temp engine dir with a minimal default_data.js
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

        // 3 talking + 3 still + 1 startup = 7
        let talking: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_talking").collect();
        let still: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_still").collect();
        let startup: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_sprite_startup").collect();

        assert_eq!(talking.len(), 3);
        assert_eq!(still.len(), 3);
        assert_eq!(startup.len(), 1); // only Juge2/2 has startup

        // Check paths
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

    // --- New tests: sprite extraction edge cases ---

    /// Frames referencing a profile with base "Inconnu" should produce no default_sprite assets.
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
        assert!(
            default_sprites.is_empty(),
            "Profiles with base='Inconnu' should produce no default sprites, got {}",
            default_sprites.len()
        );
    }

    /// Frames referencing a profile with base "" (empty) should produce no default_sprite assets.
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
        assert!(
            default_sprites.is_empty(),
            "Profiles with empty base should produce no default sprites, got {}",
            default_sprites.len()
        );
    }

    /// When engine_dir has no default_data.js, no startup sprites should be generated from frames.
    #[test]
    fn test_extract_sprites_no_startup_without_data() {
        let dir = tempfile::tempdir().unwrap();
        // engine_dir exists but has no Javascript/default_data.js
        let data = json!({
            "profiles": [null, {"base": "Phoenix", "icon": "", "custom_sprites": []}],
            "frames": [null, {"characters": [{"profile_id": 1, "sprite_id": -1}]}]
        });
        let assets = extract_asset_urls(&data, &test_site_paths(), dir.path());
        let startup: Vec<_> = assets.iter()
            .filter(|a| a.asset_type == "default_sprite_startup")
            .collect();
        assert!(
            startup.is_empty(),
            "Without default_data.js, no startup sprites should be generated"
        );
        // But talking and still should still be generated
        let talking: Vec<_> = assets.iter()
            .filter(|a| a.asset_type == "default_sprite_talking")
            .collect();
        assert_eq!(talking.len(), 1, "Talking sprites should still be generated");
    }

    /// Only matching base/sprite combos should get startup sprites from default_profiles_startup.
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
        // Only Phoenix/3 has a startup entry, not Phoenix/1
        assert_eq!(startup.len(), 1);
        assert_eq!(startup[0].local_path, "defaults/images/charsStartup/Phoenix/3.gif");
    }

    /// When sprite_id >= 0 (custom sprite), no default sprites should be generated.
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
        assert!(
            default_sprites.is_empty(),
            "Custom sprites (sprite_id >= 0) should not generate default sprites, got {}",
            default_sprites.len()
        );
    }

    /// Same character+sprite used in multiple frames should only produce one set of assets.
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

    /// Empty trial data (no profiles, no frames) should produce only voice assets.
    #[test]
    fn test_extract_empty_trial_data() {
        let data = json!({});
        let assets = extract_asset_urls(&data, &test_site_paths(), &test_engine_dir());
        // Only voices should be present (always generated)
        let non_voice: Vec<_> = assets.iter()
            .filter(|a| a.asset_type != "voice")
            .collect();
        assert!(
            non_voice.is_empty(),
            "Empty trial data should produce only voice assets, got {} non-voice assets: {:?}",
            non_voice.len(),
            non_voice.iter().map(|a| &a.asset_type).collect::<Vec<_>>()
        );
        // Verify voices are present
        let voices: Vec<_> = assets.iter().filter(|a| a.asset_type == "voice").collect();
        assert_eq!(voices.len(), 9, "Should still have 3 voice IDs x 3 formats = 9 voice assets");
    }

    #[test]
    fn test_extract_default_place_assets_from_real_data() {
        // Use real engine dir to test with actual default_data.js
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let engine_dir = manifest_dir.parent().unwrap().join("engine");
        if !engine_dir.join("Javascript/default_data.js").exists() {
            return; // Skip if engine dir doesn't exist (CI)
        }

        let paths = test_site_paths();
        let assets = extract_default_place_assets(&engine_dir, &paths);

        // default_data.js has ~20 default places with backgrounds and some foreground objects
        assert!(
            assets.len() >= 20,
            "Expected at least 20 default place assets, got {}",
            assets.len()
        );

        // Check that backgrounds are present
        let bgs: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_place_bg").collect();
        assert!(bgs.len() >= 15, "Expected at least 15 backgrounds, got {}", bgs.len());

        // Check that foreground objects are present
        let fgs: Vec<_> = assets.iter().filter(|a| a.asset_type == "default_place_fg").collect();
        assert!(fgs.len() >= 5, "Expected at least 5 foreground objects, got {}", fgs.len());

        // Verify local paths are correct
        for asset in &assets {
            assert!(
                asset.local_path.starts_with("defaults/images/defaultplaces/"),
                "Local path should start with defaults/images/defaultplaces/, got: {}",
                asset.local_path
            );
            assert!(asset.is_default, "Default place assets should be marked as default");
        }

        // Verify specific well-known assets exist
        let has_courtroom = assets.iter().any(|a| a.local_path.contains("pw_courtroom.jpg"));
        assert!(has_courtroom, "Should include pw_courtroom.jpg");

        let has_benches = assets.iter().any(|a| a.local_path.contains("pw_courtroom_benches.gif"));
        assert!(has_benches, "Should include pw_courtroom_benches.gif foreground object");
    }

    // --- Phantom sprite bounds checking ---

    /// Helper: create a temp engine dir with given default_profiles_nb and default_profiles_startup.
    fn temp_engine_dir(profiles_nb_json: &str, profiles_startup_json: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let js_dir = dir.path().join("Javascript");
        std::fs::create_dir_all(&js_dir).unwrap();
        std::fs::write(
            js_dir.join("default_data.js"),
            format!(
                "var default_profiles_nb = {};\nvar default_profiles_startup = {};",
                profiles_nb_json, profiles_startup_json
            ),
        ).unwrap();
        dir
    }

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
        assert!(
            default_sprites.is_empty(),
            "Base 'UnknownChar' not in profiles_nb should produce no default sprites, got {}",
            default_sprites.len()
        );
    }

    // --- Custom sprite shadowing default sprite (same URL) ---

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

        // Second add: default with proper local_path → should upgrade
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

        // Second add: different local_path → should NOT overwrite
        add_asset(&mut assets, &mut seen, url.clone(), "default_sprite_talking", true, "defaults/other.gif".to_string());
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].local_path, "some/existing/path.gif", "Original local_path should be preserved");
    }

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

        // Find the asset for the talking sprite URL
        let talking_url_suffix = "persos/TestChar/1.gif";
        let matching: Vec<_> = assets.iter()
            .filter(|a| a.url.contains(talking_url_suffix))
            .collect();
        assert_eq!(matching.len(), 1, "Should have exactly 1 entry for TestChar/1.gif");
        assert!(
            !matching[0].local_path.is_empty(),
            "local_path should be upgraded to default path, got empty"
        );
        assert!(
            matching[0].local_path.starts_with("defaults/"),
            "local_path should start with defaults/, got: {}",
            matching[0].local_path
        );
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
            },
        ];
        rewrite_external_urls(&mut data, 99, &downloaded);
        assert_eq!(
            data["profiles"][1]["custom_sprites"][0]["talking"],
            "defaults/images/chars/Olga/1.gif",
            "Custom sprite URL should be rewritten to default path"
        );
    }
}
