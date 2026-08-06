use std::collections::HashSet;

use serde_json::Value;

use super::super::default_data::DefaultData;
use super::super::{AssetRef, SitePaths};
use super::helpers::*;

/// Extract default sprite assets for characters used in the trial.
/// Reads `default_profiles_nb` and `default_profiles_startup` from `default_data.js`
/// to know how many sprites each default character has, then generates download URLs
/// for every default sprite of every character base referenced in `trial_data.profiles`.
pub fn extract_default_sprite_assets(
    trial_data: &Value,
    site_paths: &SitePaths,
    default_data: &DefaultData,
) -> Vec<AssetRef> {
    let profiles_nb = &default_data.profiles_nb;
    let profiles_startup = &default_data.profiles_startup;
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
    default_data: &DefaultData,
    site_paths: &SitePaths,
) -> Vec<AssetRef> {
    let mut assets: Vec<AssetRef> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Extract all image paths from the default_places variable.
    // Paths look like: defaults/images/defaultplaces/backgrounds/pw_courtroom.jpg
    //             or:  defaults/images/defaultplaces/foreground_objects/pw_courtroom_benches.gif
    let dp_server_bg = format!("{}backgrounds/", site_paths.defaultplaces_path());
    let dp_server_fg = format!("{}foreground_objects/", site_paths.defaultplaces_path());

    for (category, filename) in &default_data.place_files {
        let (server_dir, local_dir, asset_type) = match category.as_str() {
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
