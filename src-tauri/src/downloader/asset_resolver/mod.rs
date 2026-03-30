mod helpers;
mod defaults;
mod extractors;
mod rewriter;

#[cfg(test)]
use helpers::*;
use extractors::*;
pub use defaults::{extract_default_sprite_assets, extract_default_place_assets};
pub use rewriter::rewrite_external_urls;

use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

use super::{AssetRef, SitePaths};

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

#[cfg(test)]
mod tests;
