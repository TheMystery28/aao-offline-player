use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::super::{AssetRef, SitePaths};
use super::helpers::*;

/// Parse `default_profiles_nb` from engine/Javascript/default_data.js.
/// Returns a map of character base name -> number of default sprites.
pub(super) fn parse_default_profiles_nb(engine_dir: &Path) -> HashMap<String, u32> {
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
pub(super) fn parse_default_profiles_startup(engine_dir: &Path) -> HashSet<String> {
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
pub(super) fn parse_js_object_assignment(source: &str, var_name: &str) -> Option<String> {
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
/// with negative IDs. Their images are NOT in the trial data -- they're defined in
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
