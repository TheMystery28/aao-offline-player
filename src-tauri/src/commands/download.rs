//! Commands for downloading cases and assets from AAO.
//!
//! This module handles the high-level download pipeline, including fetching
//! case metadata, managing download sequences, and retrying failed assets.
//! It uses a channel-based event system to stream progress updates to the frontend.

use std::fs;
use std::sync::atomic::Ordering;
use tauri::ipc::Channel;
use tauri::State;

use crate::app_state::{AppPaths, MutableConfig};
use crate::downloader;
use crate::downloader::asset_downloader::DownloadEvent;
use crate::error::AppError;

/// Cancel the current in-progress download or sequence.
///
/// Sets a shared atomic flag that the download pipeline checks periodically.
#[tauri::command]
pub fn cancel_download(paths: State<'_, AppPaths>) -> Result<(), AppError> {
    paths.cancel_flag.store(true, Ordering::Relaxed);
    log::debug!("Download cancellation requested");
    Ok(())
}

/// Fetch case metadata and sequence information without downloading any assets.
///
/// This is used by the frontend to display case details and "what's inside"
/// before the user commits to a full download.
///
/// # Arguments
///
/// * `case_id` - The AAO case ID to fetch.
///
/// # Returns
///
/// A `CaseInfo` object containing title, author, and sequence parts.
#[tauri::command]
pub async fn fetch_case_info(
    paths: State<'_, AppPaths>,
    case_id: u32,
) -> Result<downloader::CaseInfo, AppError> {
    let client = paths.http_client.clone();

    let (case_info, _trial_data, _info_json, _data_json) =
        downloader::case_fetcher::fetch_case(&client, case_id).await?;

    Ok(case_info)
}

/// Download a sequence of cases in order.
///
/// Cases already present on disk are skipped. Progress events are emitted
/// for each part of the sequence and for individual asset downloads.
///
/// # Arguments
///
/// * `case_ids` - A list of AAO case IDs in the sequence.
/// * `on_event` - A Tauri IPC channel to stream `DownloadEvent`s to the UI.
///
/// # Returns
///
/// A list of `CaseManifest`s for all successfully downloaded cases in the sequence.
#[tauri::command]
pub async fn download_sequence(
    paths: State<'_, AppPaths>,
    config: State<'_, MutableConfig>,
    case_ids: Vec<u32>,
    on_event: Channel<DownloadEvent>,
) -> Result<Vec<downloader::manifest::CaseManifest>, AppError> {
    let engine_dir = paths.engine_dir.clone();
    let data_dir = paths.data_dir.clone();
    let cancel_flag = paths.cancel_flag.clone();
    let client = paths.http_client.clone();
    let concurrency = config.0.lock().map_err(|e| e.to_string())?.concurrent_downloads;
    cancel_flag.store(false, Ordering::Relaxed);

    let total_parts = case_ids.len();
    let mut manifests = Vec::new();

    // Fetch site paths once (static config, same for all parts)
    let site_paths = downloader::case_fetcher::fetch_site_paths(&client).await?;

    // Open dedup index once for the whole sequence. Reopening per-part causes
    // concurrent redb::Database instances on the same file, which corrupts mmap on Android.
    let dedup_index = downloader::dedup::DedupIndex::open(&data_dir).ok();

    for (idx, &case_id) in case_ids.iter().enumerate() {
        // Check if already downloaded
        let case_dir = data_dir.join("case").join(case_id.to_string());
        if case_dir.join("manifest.json").exists() {
            log::debug!("Sequence: part {}/{} (case {}) already downloaded, skipping", idx + 1, total_parts, case_id);
            match downloader::manifest::read_manifest(&case_dir) {
                Ok(manifest) => {
                    let _ = on_event.send(DownloadEvent::SequenceProgress {
                        current_part: idx + 1,
                        total_parts,
                        part_title: format!("{} (already downloaded)", manifest.title),
                    });
                    manifests.push(manifest);
                    continue;
                }
                Err(_) => {
                    // Manifest unreadable, re-download
                }
            }
        }

        // Emit sequence progress
        let _ = on_event.send(DownloadEvent::SequenceProgress {
            current_part: idx + 1,
            total_parts,
            part_title: format!("Part {}", idx + 1),
        });

        let manifest = downloader::pipeline::download_single_case(
            case_id, &client, &site_paths, &engine_dir, &data_dir,
            dedup_index.as_ref(), &on_event, concurrency, cancel_flag.clone(),
        ).await?;

        // Update sequence progress with actual title
        let _ = on_event.send(DownloadEvent::SequenceProgress {
            current_part: idx + 1,
            total_parts,
            part_title: manifest.title.clone(),
        });

        manifests.push(manifest);
    }

    // Send final finished event for the whole sequence
    let total_downloaded: usize = manifests.iter().map(|m| m.assets.total_downloaded).sum();
    let total_bytes: u64 = manifests.iter().map(|m| m.assets.total_size_bytes).sum();
    let total_failed: usize = manifests.iter().map(|m| m.failed_assets.len()).sum();
    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: total_downloaded,
        failed: total_failed,
        total_bytes,
        dedup_saved_bytes: 0,
    });

    Ok(manifests)
}

/// Download a single case and all its required assets.
///
/// This executes the full pipeline: fetch metadata → extract asset URLs →
/// download assets (with de-duplication) → generate manifest.
///
/// # Arguments
///
/// * `case_id` - The AAO case ID to download.
/// * `on_event` - A Tauri IPC channel for real-time progress updates.
///
/// # Returns
///
/// The `CaseManifest` for the newly downloaded case.
#[tauri::command]
pub async fn download_case(
    paths: State<'_, AppPaths>,
    config: State<'_, MutableConfig>,
    case_id: u32,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, AppError> {
    let engine_dir = paths.engine_dir.clone();
    let data_dir = paths.data_dir.clone();
    let cancel_flag = paths.cancel_flag.clone();
    let client = paths.http_client.clone();
    let concurrency = config.0.lock().map_err(|e| e.to_string())?.concurrent_downloads;
    cancel_flag.store(false, Ordering::Relaxed);

    let site_paths = downloader::case_fetcher::fetch_site_paths(&client).await?;
    let dedup_index = downloader::dedup::DedupIndex::open(&data_dir).ok();

    Ok(downloader::pipeline::download_single_case(
        case_id, &client, &site_paths, &engine_dir, &data_dir,
        dedup_index.as_ref(), &on_event, concurrency, cancel_flag,
    ).await?)
}

/// Re-attempt downloading any assets that failed during a previous download attempt.
///
/// Reads the case's manifest to identify failed assets, then runs the download
/// pipeline for those specific assets. If successful, the manifest is updated.
///
/// # Arguments
///
/// * `case_id` - The ID of the case to fix.
/// * `on_event` - A Tauri IPC channel for progress updates.
///
/// # Errors
///
/// Returns an error if the case is not found or if the AAO website is unreachable.
#[tauri::command]
pub async fn retry_failed_assets(
    paths: State<'_, AppPaths>,
    config: State<'_, MutableConfig>,
    case_id: u32,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, AppError> {
    let data_dir = paths.data_dir.clone();
    let cancel_flag = paths.cancel_flag.clone();
    let client = paths.http_client.clone();
    let concurrency = config.0.lock().map_err(|e| e.to_string())?.concurrent_downloads;
    cancel_flag.store(false, Ordering::Relaxed);

    let case_dir = data_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id).into());
    }

    let mut manifest = downloader::manifest::read_manifest(&case_dir)?;

    if manifest.failed_assets.is_empty() {
        return Ok(manifest);
    }

    log::info!("Retrying {} failed assets for case {}", manifest.failed_assets.len(), case_id);

    // Convert failed assets back to AssetRef for re-download
    let assets_to_retry: Vec<downloader::AssetRef> = manifest
        .failed_assets
        .iter()
        .map(|f| downloader::AssetRef {
            url: f.url.clone(),
            asset_type: f.asset_type.clone(),
            is_default: f.local_path.starts_with("defaults/"),
            local_path: if f.local_path.starts_with("defaults/") {
                f.local_path.clone()
            } else {
                String::new() // External assets get rehashed
            },
        })
        .collect();

    // Check if aaonline.fr is reachable; if not, skip aaonline URLs to avoid noise
    let mut assets_to_retry = assets_to_retry;
    let aaonline_up = downloader::case_fetcher::is_aaonline_reachable(&client).await;
    if !aaonline_up {
        let before = assets_to_retry.len();
        assets_to_retry.retain(|a| !a.url.starts_with(downloader::AAONLINE_BASE));
        let skipped = before - assets_to_retry.len();
        log::warn!("aaonline.fr is unreachable — skipped {} aaonline assets", skipped);
        if assets_to_retry.is_empty() {
            return Err("aaonline.fr is currently unreachable. Please try again later.".to_string().into());
        }
    }

    let dedup_index = downloader::dedup::DedupIndex::open(&data_dir).ok();
    let result = downloader::asset_downloader::download_assets(
        &client,
        assets_to_retry,
        &case_dir,
        &data_dir,
        dedup_index.as_ref(),
        &on_event,
        concurrency,
        cancel_flag.clone(),
    )
    .await?;

    // Merge newly downloaded assets into manifest
    for asset in &result.downloaded {
        manifest.asset_map.insert(asset.original_url.clone(), asset.local_path.clone());
    }

    // Update failed list to only those that still failed
    manifest.failed_assets = result.failed;

    // Update asset counts
    manifest.assets.total_downloaded = manifest.asset_map.len();
    manifest.assets.total_size_bytes += result.downloaded.iter().map(|a| a.size).sum::<u64>();

    // Rewrite external URLs in trial_data for newly successful external downloads
    let new_externals: Vec<_> = result.downloaded.iter()
        .filter(|a| a.local_path.starts_with("assets/"))
        .cloned()
        .collect();
    if !new_externals.is_empty() {
        let data_path = case_dir.join("trial_data.json");
        if data_path.exists() {
            let data_str = fs::read_to_string(&data_path)
                .map_err(|e| format!("Failed to read trial_data.json: {}", e))?;
            let mut data_value: serde_json::Value = serde_json::from_str(&data_str)
                .map_err(|e| format!("Failed to parse trial_data.json: {}", e))?;
            downloader::asset_resolver::rewrite_external_urls(&mut data_value, case_id, &new_externals);
            fs::write(
                &data_path,
                serde_json::to_string_pretty(&data_value)
                    .map_err(|e| format!("Failed to serialize trial_data: {}", e))?,
            )
            .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;
        }
    }

    // Save updated manifest
    downloader::manifest::write_manifest(&manifest, &case_dir)?;

    log::info!(
        "Retry complete: {} newly downloaded, {} still failed",
        result.downloaded.len(),
        manifest.failed_assets.len()
    );

    Ok(manifest)
}

/// Update an existing case by re-fetching its data from AAO.
///
/// This is used to sync local cases with updates made on the AAO website.
///
/// # Arguments
///
/// * `case_id` - The ID of the case to update.
/// * `redownload_assets` - If true, all assets are re-downloaded even if they exist.
///   If false, only newly added assets in the updated trial data are downloaded.
/// * `on_event` - A Tauri IPC channel for progress updates.
///
/// # Returns
///
/// The updated `CaseManifest`.
#[tauri::command]
pub async fn update_case(
    paths: State<'_, AppPaths>,
    config: State<'_, MutableConfig>,
    case_id: u32,
    redownload_assets: bool,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, AppError> {
    let engine_dir = paths.engine_dir.clone();
    let data_dir = paths.data_dir.clone();
    let cancel_flag = paths.cancel_flag.clone();
    let client = paths.http_client.clone();
    let concurrency = config.0.lock().map_err(|e| e.to_string())?.concurrent_downloads;
    cancel_flag.store(false, Ordering::Relaxed);

    let case_dir = data_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id).into());
    }

    // Read old manifest to know what we already have
    let old_manifest = downloader::manifest::read_manifest(&case_dir)?;

    // 1. Fetch site paths
    log::debug!("Update: fetching site paths...");
    let site_paths = downloader::case_fetcher::fetch_site_paths(&client).await?;

    // 2. Fetch case data
    log::debug!("Update: fetching case {} data...", case_id);
    let (case_info, trial_data, info_json, data_json) =
        downloader::case_fetcher::fetch_case(&client, case_id).await?;

    // 3. Save updated trial_info.json
    downloader::pipeline::save_trial_info(&case_dir, &info_json)?;

    // 4. Extract and classify asset URLs from new trial data
    let assets = downloader::pipeline::extract_all_assets(&trial_data, &site_paths, &engine_dir);
    let total_assets = assets.len();
    let (case_specific, shared) = downloader::asset_resolver::classify_assets(assets);
    let case_specific_count = case_specific.len();
    let shared_count = shared.len();

    // 5. Filter what to download based on mode
    let to_download = if redownload_assets {
        // Full update: download all case-specific + missing defaults
        let missing_defaults: Vec<_> = shared
            .into_iter()
            .filter(|a| !a.local_path.is_empty() && !downloader::vfs::asset_exists(&data_dir, &a.local_path))
            .collect();
        let mut all = case_specific;
        all.extend(missing_defaults);
        all
    } else {
        // Script-only update: download only NEW assets not already on disk or in manifest
        let mut new_assets = Vec::new();

        for asset in case_specific {
            // Skip if URL is already in the old asset_map (already downloaded before)
            if old_manifest.asset_map.contains_key(&asset.url) {
                continue;
            }
            // For internal assets with a local_path, also check if file exists on disk
            if !asset.local_path.is_empty() && downloader::vfs::asset_exists(&data_dir, &asset.local_path) {
                continue;
            }
            new_assets.push(asset);
        }

        // Shared/default: only download if missing from disk
        let missing_defaults: Vec<_> = shared
            .into_iter()
            .filter(|a| !a.local_path.is_empty() && !downloader::vfs::asset_exists(&data_dir, &a.local_path))
            .collect();
        new_assets.extend(missing_defaults);
        new_assets
    };

    log::debug!(
        "Update (redownload_assets={}): {} total extracted, downloading {}",
        redownload_assets,
        total_assets,
        to_download.len()
    );

    // 6. Download assets
    let dedup_index = downloader::dedup::DedupIndex::open(&data_dir).ok();
    let result = downloader::asset_downloader::download_assets(
        &client,
        to_download,
        &case_dir,
        &data_dir,
        dedup_index.as_ref(),
        &on_event,
        concurrency,
        cancel_flag.clone(),
    )
    .await?;

    // 7. Build updated asset_map: start from old map, add/overwrite new downloads
    let mut asset_map = if redownload_assets {
        // Full update: start fresh
        std::collections::HashMap::new()
    } else {
        // Script-only: keep existing mappings
        old_manifest.asset_map.clone()
    };
    for asset in &result.downloaded {
        asset_map.insert(asset.original_url.clone(), asset.local_path.clone());
    }

    // 8. Rewrite external URLs in trial_data then save
    let mut data_value: serde_json::Value = serde_json::from_str(&data_json)
        .map_err(|e| format!("Failed to reparse data JSON: {}", e))?;
    // Rewrite using ALL known asset mappings (old + new)
    let all_downloaded: Vec<downloader::asset_downloader::DownloadedAsset> = asset_map
        .iter()
        .filter(|(_, v)| v.starts_with("assets/"))
        .map(|(url, path)| downloader::asset_downloader::DownloadedAsset {
            original_url: url.clone(),
            local_path: path.clone(),
            size: 0,
            content_hash: 0,
        })
        .collect();
    downloader::asset_resolver::rewrite_external_urls(&mut data_value, case_id, &all_downloaded);
    fs::write(
        case_dir.join("trial_data.json"),
        serde_json::to_string_pretty(&data_value)
            .map_err(|e| format!("Failed to serialize trial_data: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;

    // 9. Build and save manifest
    let total_bytes: u64 = if redownload_assets {
        result.downloaded.iter().map(|a| a.size).sum()
    } else {
        old_manifest.assets.total_size_bytes + result.downloaded.iter().map(|a| a.size).sum::<u64>()
    };

    let manifest = downloader::manifest::CaseManifest {
        case_id: case_info.id,
        title: case_info.title,
        author: case_info.author,
        language: case_info.language,
        download_date: old_manifest.download_date.clone(),
        format: case_info.format,
        sequence: case_info.sequence,
        assets: downloader::manifest::AssetSummary {
            case_specific: case_specific_count,
            shared_defaults: shared_count,
            total_downloaded: asset_map.len(),
            total_size_bytes: total_bytes,
        },
        asset_map,
        failed_assets: result.failed,
        has_plugins: old_manifest.has_plugins || case_dir.join("plugins").is_dir(),
        has_case_config: old_manifest.has_case_config || case_dir.join("case_config.json").is_file(),
    };
    downloader::manifest::write_manifest(&manifest, &case_dir)?;

    log::info!(
        "Update complete: {} new downloads, {} failed",
        result.downloaded.len(),
        manifest.failed_assets.len()
    );

    Ok(manifest)
}
