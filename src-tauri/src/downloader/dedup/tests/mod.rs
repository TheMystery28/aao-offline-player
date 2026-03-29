use super::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::Value;
use xxhash_rust::xxh3::xxh3_64;

use crate::downloader::manifest::{read_manifest, write_manifest, CaseManifest, AssetSummary};

mod test_helpers;
mod test_index;
mod test_operations;
mod test_optimize;

/// Assert that every asset_map entry in the manifest has a consistent path in trial_data.json.
/// For defaults/ paths: trial_data must contain the defaults path (not case/{id}/assets/...).
/// For assets/ paths: trial_data must contain case/{id}/assets/... form.
pub(super) fn assert_manifest_trial_data_agree(case_dir: &Path, case_id: u32) {
    let manifest = read_manifest(case_dir).expect("Failed to read manifest for consistency check");
    let td_path = case_dir.join("trial_data.json");
    if !td_path.exists() {
        return; // No trial_data to check
    }
    let td_text = fs::read_to_string(&td_path).expect("Failed to read trial_data.json");

    for (key, local_path) in &manifest.asset_map {
        // Skip defaults/ entries that map to themselves (default sprites/voices/places)
        if key == local_path && local_path.starts_with("defaults/") {
            continue;
        }

        let expected_server_path = if local_path.starts_with("defaults/") {
            local_path.clone()
        } else if local_path.starts_with("assets/") {
            format!("case/{}/{}", case_id, local_path)
        } else {
            local_path.clone()
        };

        // The expected path must appear somewhere in trial_data
        assert!(
            td_text.contains(&expected_server_path),
            "Manifest/trial_data disagreement for key '{}': expected '{}' in trial_data but not found.\n\
             Manifest local_path: '{}'\n\
             trial_data snippet: {}",
            key, expected_server_path, local_path,
            &td_text[..td_text.len().min(500)]
        );

        // If the asset was deduped to defaults/, the OLD case-specific path must NOT appear
        if local_path.starts_with("defaults/") && key.starts_with("assets/") {
            let old_case_path = format!("case/{}/{}", case_id, key);
            assert!(
                !td_text.contains(&old_case_path),
                "Manifest/trial_data disagreement: asset '{}' was deduped to '{}' but trial_data \
                 still contains the old path '{}'. The rewrite didn't update trial_data.",
                key, local_path, old_case_path
            );
        }
    }
}

pub(super) fn make_case_with_asset(data_dir: &Path, case_id: u32, filename: &str, content: &[u8]) {
    let case_dir = data_dir.join("case").join(case_id.to_string());
    let assets_dir = case_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join(filename), content).unwrap();

    let mut asset_map = HashMap::new();
    asset_map.insert(
        format!("http://example.com/{}", filename),
        format!("assets/{}", filename),
    );
    let manifest = CaseManifest {
        case_id,
        title: format!("Case {}", case_id),
        author: "Author".to_string(),
        language: "en".to_string(),
        download_date: "2025-01-01".to_string(),
        format: "v6".to_string(),
        sequence: None,
        assets: AssetSummary {
            case_specific: 1,
            shared_defaults: 0,
            total_downloaded: 1,
            total_size_bytes: content.len() as u64,
        },
        asset_map,
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();

    let trial_data = serde_json::json!({
        "profiles": [null, {
            "custom_sprites": [{
                "talking": format!("case/{}/assets/{}", case_id, filename),
                "still": "",
                "startup": ""
            }]
        }]
    });
    fs::write(
        case_dir.join("trial_data.json"),
        serde_json::to_string_pretty(&trial_data).unwrap(),
    ).unwrap();
}
