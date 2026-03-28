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
