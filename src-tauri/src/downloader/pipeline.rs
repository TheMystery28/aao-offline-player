use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::ipc::Channel;

use super::{AssetRef, SitePaths, AAONLINE_BASE};
use super::asset_downloader::DownloadEvent;
use super::manifest::CaseManifest;

/// Assets extracted, deduplicated, classified, and filtered — ready for download.
pub struct PreparedAssets {
    pub to_download: Vec<AssetRef>,
    pub cached_defaults: Vec<(String, String)>, // (url, local_path)
    pub case_specific_count: usize,
    pub shared_count: usize,
}

/// Extract all assets from trial data, including default sprites and places,
/// with deduplication. Returns the raw list before classification.
/// Shared by all download commands.
pub fn extract_all_assets(
    trial_data: &serde_json::Value,
    site_paths: &SitePaths,
    engine_dir: &Path,
) -> Vec<AssetRef> {
    let mut assets = super::asset_resolver::extract_asset_urls(trial_data, site_paths, engine_dir);

    let default_sprites = super::asset_resolver::extract_default_sprite_assets(
        trial_data, site_paths, engine_dir,
    );
    let default_places = super::asset_resolver::extract_default_place_assets(engine_dir, site_paths);

    let existing_urls: HashSet<String> = assets.iter().map(|a| a.url.clone()).collect();
    for sprite in default_sprites {
        if existing_urls.contains(&sprite.url) {
            if !sprite.local_path.is_empty() {
                if let Some(existing) = assets.iter_mut().find(|a| a.url == sprite.url && a.local_path.is_empty()) {
                    existing.local_path = sprite.local_path.clone();
                    existing.is_default = true;
                }
            }
        } else {
            assets.push(sprite);
        }
    }
    for place in default_places {
        if !existing_urls.contains(&place.url) {
            assets.push(place);
        }
    }

    assets
}

/// Extract, deduplicate, classify, and filter assets for a case.
/// Builds on extract_all_assets, adding classify + missing/cached filtering.
/// Shared by download_case and download_sequence.
pub fn extract_and_prepare_assets(
    trial_data: &serde_json::Value,
    site_paths: &SitePaths,
    engine_dir: &Path,
    data_dir: &Path,
) -> PreparedAssets {
    let assets = extract_all_assets(trial_data, site_paths, engine_dir);
    let (case_specific, shared) = super::asset_resolver::classify_assets(assets);
    let case_specific_count = case_specific.len();
    let shared_count = shared.len();

    let mut missing_defaults: Vec<AssetRef> = Vec::new();
    let mut cached_defaults: Vec<(String, String)> = Vec::new();
    for a in shared {
        if a.local_path.is_empty() {
            missing_defaults.push(a);
            continue;
        }
        if super::vfs::asset_exists(data_dir, &a.local_path) {
            cached_defaults.push((a.url.clone(), a.local_path.clone()));
        } else {
            missing_defaults.push(a);
        }
    }

    let mut to_download = case_specific;
    to_download.extend(missing_defaults);

    PreparedAssets {
        to_download,
        cached_defaults,
        case_specific_count,
        shared_count,
    }
}

/// Save trial_info.json to a case directory.
pub fn save_trial_info(case_dir: &Path, info_json: &str) -> Result<(), String> {
    let info_pretty: serde_json::Value = serde_json::from_str(info_json)
        .map_err(|e| format!("Failed to reparse info JSON: {}", e))?;
    fs::write(
        case_dir.join("trial_info.json"),
        serde_json::to_string_pretty(&info_pretty)
            .map_err(|e| format!("Failed to serialize trial_info: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_info.json: {}", e))
}

/// Full single-case download pipeline: fetch → extract → download → rewrite → manifest.
/// Shared by download_case and download_sequence.
pub(crate) async fn download_single_case(
    case_id: u32,
    client: &reqwest::Client,
    site_paths: &SitePaths,
    engine_dir: &Path,
    data_dir: &Path,
    dedup_index: Option<&super::dedup::DedupIndex>,
    on_event: &Channel<DownloadEvent>,
    concurrency: usize,
    cancel_flag: Arc<AtomicBool>,
) -> Result<CaseManifest, String> {
    // 1. Fetch case data
    let (case_info, trial_data, info_json, data_json) =
        super::case_fetcher::fetch_case(client, case_id).await?;

    // 2. Save trial_info.json
    let case_dir = data_dir.join("case").join(case_id.to_string());
    fs::create_dir_all(&case_dir)
        .map_err(|e| format!("Failed to create case directory: {}", e))?;
    save_trial_info(&case_dir, &info_json)?;

    // 3. Extract and prepare assets
    let prepared = extract_and_prepare_assets(&trial_data, site_paths, engine_dir, data_dir);
    let mut to_download = prepared.to_download;

    // 4. aaonline reachability check
    let aaonline_up = super::case_fetcher::is_aaonline_reachable(client).await;
    if !aaonline_up {
        to_download.retain(|a| !a.url.starts_with(AAONLINE_BASE));
    }

    // 5. Download assets
    let case_dir_buf = PathBuf::from(&case_dir);
    let data_dir_buf = PathBuf::from(data_dir);
    let result = super::asset_downloader::download_assets(
        client,
        to_download,
        &case_dir_buf,
        &data_dir_buf,
        dedup_index,
        on_event,
        concurrency,
        cancel_flag,
    )
    .await?;

    // 6. Rewrite external URLs in trial_data and save
    let mut data_value: serde_json::Value = serde_json::from_str(&data_json)
        .map_err(|e| format!("Failed to reparse data JSON: {}", e))?;
    super::asset_resolver::rewrite_external_urls(&mut data_value, case_id, &result.downloaded);
    fs::write(
        case_dir.join("trial_data.json"),
        serde_json::to_string_pretty(&data_value)
            .map_err(|e| format!("Failed to serialize trial_data: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;

    // 7. Build and save manifest
    let mut manifest = super::manifest::build_manifest(
        &case_info,
        &result.downloaded,
        result.failed,
        prepared.case_specific_count,
        prepared.shared_count,
    );
    for (url, local_path) in &prepared.cached_defaults {
        manifest.asset_map.insert(url.clone(), local_path.clone());
    }
    manifest.assets.total_downloaded = manifest.asset_map.len();
    super::manifest::write_manifest(&manifest, &case_dir)?;

    // 8. Register downloaded assets in dedup index
    if let Some(ref index) = dedup_index {
        for asset in &result.downloaded {
            if !asset.local_path.is_empty() {
                let reg_path = if asset.local_path.starts_with("defaults/") {
                    asset.local_path.clone()
                } else {
                    super::asset_paths::case_relative(case_id, &asset.local_path)
                };
                let _ = index.register(&reg_path, asset.size, asset.content_hash);
            }
        }
    }

    // 9. Post-download finalization (only when inline dedup wasn't available)
    if dedup_index.is_none() {
        let _ = on_event.send(DownloadEvent::Progress {
            completed: 0,
            total: 1,
            current_url: "Optimizing storage...".to_string(),
            bytes_downloaded: 0,
            elapsed_ms: 0,
        });
        let (dedup_count, _) = super::dedup::finalize_case_import(case_id, data_dir);
        if dedup_count > 0 {
            return super::manifest::read_manifest(&case_dir);
        }
    }

    Ok(manifest)
}

