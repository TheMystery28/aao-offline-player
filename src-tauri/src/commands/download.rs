use std::fs;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use tauri::ipc::Channel;
use tauri::State;

use crate::app_state::AppState;
use crate::downloader;
use crate::downloader::asset_downloader::DownloadEvent;

/// Check if an asset file truly exists on disk — follows VFS pointers.
/// A VFS pointer whose target is missing counts as "not exists".
fn asset_exists_on_disk(data_dir: &std::path::Path, local_path: &str) -> bool {
    let disk_path = data_dir.join(local_path);
    if !disk_path.exists() {
        return false;
    }
    match downloader::vfs::read_vfs_pointer(&disk_path) {
        Some(target) => data_dir.join(&target).is_file(),
        None => true,
    }
}

/// Cancel the current in-progress download.
#[tauri::command]
pub fn cancel_download(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    s.cancel_flag.store(true, Ordering::Relaxed);
    debug_log!("Download cancellation requested");
    Ok(())
}

/// Lightweight command: fetch case metadata (including sequence info) without downloading assets.
#[tauri::command]
pub async fn fetch_case_info(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
) -> Result<downloader::CaseInfo, String> {
    let client = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.http_client.clone()
    };

    let site_paths = downloader::case_fetcher::fetch_site_paths(&client).await?;
    let _ = site_paths; // Not needed, but fetch_case requires site_paths to exist first

    let (case_info, _trial_data, _info_json, _data_json) =
        downloader::case_fetcher::fetch_case(&client, case_id).await?;

    Ok(case_info)
}

/// Download all parts of a sequence. Skips cases already downloaded.
/// Emits SequenceProgress events before each part, plus per-part asset download events.
#[tauri::command]
pub async fn download_sequence(
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
    on_event: Channel<DownloadEvent>,
) -> Result<Vec<downloader::manifest::CaseManifest>, String> {
    let (engine_dir, data_dir, concurrency, cancel_flag, client) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.engine_dir.clone(), s.data_dir.clone(), s.config.concurrent_downloads, s.cancel_flag.clone(), s.http_client.clone())
    };
    cancel_flag.store(false, Ordering::Relaxed);

    let total_parts = case_ids.len();
    let mut manifests = Vec::new();

    for (idx, &case_id) in case_ids.iter().enumerate() {
        // Check if already downloaded
        let case_dir = data_dir.join("case").join(case_id.to_string());
        if case_dir.join("manifest.json").exists() {
            debug_log!("Sequence: part {}/{} (case {}) already downloaded, skipping", idx + 1, total_parts, case_id);
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

        debug_log!("Sequence: downloading part {}/{} (case {})...", idx + 1, total_parts, case_id);
        let site_paths = downloader::case_fetcher::fetch_site_paths(&client).await?;

        let (case_info, trial_data, info_json, data_json) =
            downloader::case_fetcher::fetch_case(&client, case_id).await?;

        // Update the sequence progress with actual title
        let _ = on_event.send(DownloadEvent::SequenceProgress {
            current_part: idx + 1,
            total_parts,
            part_title: case_info.title.clone(),
        });

        let case_dir = data_dir.join("case").join(case_id.to_string());
        fs::create_dir_all(&case_dir)
            .map_err(|e| format!("Failed to create case directory: {}", e))?;

        let info_pretty: serde_json::Value = serde_json::from_str(&info_json)
            .map_err(|e| format!("Failed to reparse info JSON: {}", e))?;
        fs::write(
            case_dir.join("trial_info.json"),
            serde_json::to_string_pretty(&info_pretty)
                .map_err(|e| format!("Failed to serialize trial_info: {}", e))?,
        )
        .map_err(|e| format!("Failed to write trial_info.json: {}", e))?;

        let mut assets = downloader::asset_resolver::extract_asset_urls(&trial_data, &site_paths, &engine_dir);
        let default_sprites = downloader::asset_resolver::extract_default_sprite_assets(
            &trial_data, &site_paths, &engine_dir,
        );
        let default_places = downloader::asset_resolver::extract_default_place_assets(
            &engine_dir, &site_paths,
        );
        let existing_urls: std::collections::HashSet<String> =
            assets.iter().map(|a| a.url.clone()).collect();
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

        let (case_specific, shared) = downloader::asset_resolver::classify_assets(assets);
        let case_specific_count = case_specific.len();
        let shared_count = shared.len();

        let mut missing_defaults: Vec<downloader::AssetRef> = Vec::new();
        let mut cached_defaults: Vec<(String, String)> = Vec::new();
        for a in shared {
            if a.local_path.is_empty() {
                missing_defaults.push(a);
                continue;
            }
            if asset_exists_on_disk(&data_dir, &a.local_path) {
                cached_defaults.push((a.url.clone(), a.local_path.clone()));
            } else {
                missing_defaults.push(a);
            }
        }

        let mut to_download = case_specific;
        to_download.extend(missing_defaults);

        let aaonline_up = downloader::case_fetcher::is_aaonline_reachable(&client).await;
        if !aaonline_up {
            to_download.retain(|a| !a.url.starts_with(downloader::AAONLINE_BASE));
        }

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

        let mut data_value: serde_json::Value = serde_json::from_str(&data_json)
            .map_err(|e| format!("Failed to reparse data JSON: {}", e))?;
        downloader::asset_resolver::rewrite_external_urls(
            &mut data_value,
            case_id,
            &result.downloaded,
        );
        fs::write(
            case_dir.join("trial_data.json"),
            serde_json::to_string_pretty(&data_value)
                .map_err(|e| format!("Failed to serialize trial_data: {}", e))?,
        )
        .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;

        let mut manifest = downloader::manifest::build_manifest(
            &case_info,
            &result.downloaded,
            result.failed,
            case_specific_count,
            shared_count,
        );
        for (url, local_path) in &cached_defaults {
            manifest.asset_map.insert(url.clone(), local_path.clone());
        }
        manifest.assets.total_downloaded = manifest.asset_map.len();
        downloader::manifest::write_manifest(&manifest, &case_dir)?;

        // Safety net: register assets that were skip-existing (not downloaded, so not registered
        // during download). Assets that were actually downloaded are already registered by download_assets.
        if let Ok(index) = downloader::dedup::DedupIndex::open(&data_dir) {
            for asset in &result.downloaded {
                if !asset.local_path.is_empty() {
                    let reg_path = if asset.local_path.starts_with("defaults/") {
                        asset.local_path.clone()
                    } else {
                        downloader::asset_paths::case_relative(case_id, &asset.local_path)
                    };
                    let _ = index.register(&reg_path, asset.size, asset.content_hash);
                }
            }
        }

        // Post-download finalization: only needed when inline dedup wasn't available
        if dedup_index.is_none() {
            let _ = on_event.send(DownloadEvent::Progress {
                completed: 0, total: 1,
                current_url: "Optimizing storage...".to_string(),
                bytes_downloaded: 0, elapsed_ms: 0,
            });
            let (dedup_count, _) = downloader::dedup::finalize_case_import(case_id, &data_dir);
            if dedup_count > 0 {
                manifest = downloader::manifest::read_manifest(&case_dir)?;
            }
        }

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

/// Full download pipeline: fetch case → extract assets → download case-specific → generate manifest.
/// Streams progress events to the frontend via Channel.
#[tauri::command]
pub async fn download_case(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, String> {
    let (engine_dir, data_dir, concurrency, cancel_flag, client) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.engine_dir.clone(), s.data_dir.clone(), s.config.concurrent_downloads, s.cancel_flag.clone(), s.http_client.clone())
    };
    cancel_flag.store(false, Ordering::Relaxed);

    // 1. Fetch site paths
    debug_log!("Fetching site paths from bridge.js.php...");
    let site_paths = downloader::case_fetcher::fetch_site_paths(&client).await?;

    // 2. Fetch case data
    debug_log!("Fetching case {} from trial.js.php...", case_id);
    let (case_info, trial_data, info_json, data_json) =
        downloader::case_fetcher::fetch_case(&client, case_id).await?;
    debug_log!(
        "Case: \"{}\" by {} ({})",
        case_info.title,
        case_info.author,
        case_info.language
    );

    // 3. Save trial_info.json (trial_data.json saved later after URL rewriting)
    let case_dir = data_dir.join("case").join(case_id.to_string());
    fs::create_dir_all(&case_dir)
        .map_err(|e| format!("Failed to create case directory: {}", e))?;

    let info_pretty: serde_json::Value = serde_json::from_str(&info_json)
        .map_err(|e| format!("Failed to reparse info JSON: {}", e))?;
    fs::write(
        case_dir.join("trial_info.json"),
        serde_json::to_string_pretty(&info_pretty)
            .map_err(|e| format!("Failed to serialize trial_info: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_info.json: {}", e))?;

    debug_log!(
        "Saved trial_info.json to {}",
        case_dir.display()
    );

    // 4. Extract and classify asset URLs
    let mut assets = downloader::asset_resolver::extract_asset_urls(&trial_data, &site_paths, &engine_dir);

    // Also extract default sprite assets (chars/charsStill/charsStartup) based on
    // profiles in trial_data and default_profiles_nb from default_data.js.
    let default_sprites = downloader::asset_resolver::extract_default_sprite_assets(
        &trial_data, &site_paths, &engine_dir,
    );
    debug_log!("Extracted {} default sprite assets from profiles", default_sprites.len());

    // Extract default place assets (courtrooms, lobbies, etc.) from default_data.js.
    let default_places = downloader::asset_resolver::extract_default_place_assets(
        &engine_dir, &site_paths,
    );
    debug_log!("Extracted {} default place assets", default_places.len());

    // Deduplicate: some assets may already be in the list.
    // Upgrade external entries (empty local_path) when a default sprite provides a proper path.
    let existing_urls: std::collections::HashSet<String> = assets.iter().map(|a| a.url.clone()).collect();
    let new_count = default_sprites.iter().filter(|s| !existing_urls.contains(&s.url)).count();
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
    debug_log!("{} new default sprites added (after dedup)", new_count);

    let total_assets = assets.len();
    let (case_specific, shared) = downloader::asset_resolver::classify_assets(assets);
    let case_specific_count = case_specific.len();
    let shared_count = shared.len();

    // Filter shared/default assets to only those missing from disk.
    // Track ALL shared assets (including cached) so exports include them.
    let mut skipped_exists = 0usize;
    let mut missing_defaults: Vec<downloader::AssetRef> = Vec::new();
    let mut cached_defaults: Vec<(String, String)> = Vec::new(); // (url, local_path) for already-cached
    for a in shared {
        if a.local_path.is_empty() {
            missing_defaults.push(a);
            continue;
        }
        let disk_path = data_dir.join(&a.local_path);
        // Check exists — but if it's a VFS pointer, verify the target also exists
        if asset_exists_on_disk(&data_dir, &a.local_path) {
            skipped_exists += 1;
            cached_defaults.push((a.url.clone(), a.local_path.clone()));
            debug_log!(
                "  SKIP default (exists on disk): {} → {}",
                a.local_path,
                disk_path.display()
            );
        } else {
            debug_log!(
                "  NEED default (missing): {} → {}",
                a.local_path,
                disk_path.display()
            );
            missing_defaults.push(a);
        }
    }
    let missing_defaults_count = missing_defaults.len();

    debug_log!(
        "Assets: {} case-specific, {} shared/default ({} already on disk, {} missing), {} total extracted",
        case_specific_count,
        shared_count,
        skipped_exists,
        missing_defaults_count,
        total_assets
    );

    // 5. Download case-specific + missing default assets in parallel
    // - Internal assets (local_path set) → saved to data_dir/{local_path}
    // - External assets (local_path empty) → saved to case_dir/assets/{hash}
    let mut to_download = case_specific;
    to_download.extend(missing_defaults);

    // Check if aaonline.fr is reachable; if not, skip aaonline URLs to avoid noise
    let aaonline_up = downloader::case_fetcher::is_aaonline_reachable(&client).await;
    if !aaonline_up {
        let before = to_download.len();
        to_download.retain(|a| !a.url.starts_with(downloader::AAONLINE_BASE));
        let skipped = before - to_download.len();
        debug_log!("aaonline.fr is unreachable — skipped {} aaonline assets", skipped);
    }

    debug_log!("Downloading {} assets ({} case-specific + {} missing defaults)...",
        to_download.len(), case_specific_count, missing_defaults_count);
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

    let downloaded = result.downloaded;
    let failed = result.failed;

    debug_log!(
        "Downloaded {} assets ({} bytes), {} failed",
        downloaded.len(),
        downloaded.iter().map(|a| a.size).sum::<u64>(),
        failed.len()
    );
    for d in &downloaded {
        debug_log!("  DOWNLOADED: {} → {} ({} bytes)", d.original_url, d.local_path, d.size);
    }
    for f in &failed {
        debug_log!("  FAILED: {} → {} ({})", f.url, f.local_path, f.error);
    }

    // 6. Rewrite external URLs in trial_data to point to local files, then save
    let mut data_value: serde_json::Value = serde_json::from_str(&data_json)
        .map_err(|e| format!("Failed to reparse data JSON: {}", e))?;
    downloader::asset_resolver::rewrite_external_urls(&mut data_value, case_id, &downloaded);
    fs::write(
        case_dir.join("trial_data.json"),
        serde_json::to_string_pretty(&data_value)
            .map_err(|e| format!("Failed to serialize trial_data: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;
    debug_log!("Saved trial_data.json with rewritten URLs to {}", case_dir.display());

    // 7. Generate and save manifest.
    // Include cached (skipped) defaults in asset_map so exports are self-contained.
    let mut manifest = downloader::manifest::build_manifest(
        &case_info,
        &downloaded,
        failed,
        case_specific_count,
        shared_count,
    );
    for (url, local_path) in &cached_defaults {
        manifest.asset_map.insert(url.clone(), local_path.clone());
    }
    manifest.assets.total_downloaded = manifest.asset_map.len();
    downloader::manifest::write_manifest(&manifest, &case_dir)?;
    debug_log!("Saved manifest.json to {} ({} assets incl. {} cached defaults)",
        case_dir.display(), manifest.asset_map.len(), cached_defaults.len());

    // Register ALL downloaded assets in the persistent hash index
    if let Ok(index) = downloader::dedup::DedupIndex::open(&data_dir) {
        for asset in &downloaded {
            if !asset.local_path.is_empty() {
                let reg_path = if asset.local_path.starts_with("defaults/") {
                    asset.local_path.clone()
                } else {
                    downloader::asset_paths::case_relative(case_id, &asset.local_path)
                };
                let _ = index.register(&reg_path, asset.size, asset.content_hash);
            }
        }
    }

    // Post-download finalization: only needed when inline dedup wasn't available
    if dedup_index.is_none() {
        let _ = on_event.send(DownloadEvent::Progress {
            completed: 0, total: 1,
            current_url: "Optimizing storage...".to_string(),
            bytes_downloaded: 0, elapsed_ms: 0,
        });
        let (dedup_count, _dedup_bytes) = downloader::dedup::finalize_case_import(case_id, &data_dir);
        if dedup_count > 0 {
            manifest = downloader::manifest::read_manifest(&case_dir)?;
        }
    }

    Ok(manifest)
}

/// Retry downloading failed assets for a case.
/// Reads the manifest to find failed assets, re-attempts download, updates manifest.
#[tauri::command]
pub async fn retry_failed_assets(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, String> {
    let (data_dir, concurrency, cancel_flag, client) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.data_dir.clone(), s.config.concurrent_downloads, s.cancel_flag.clone(), s.http_client.clone())
    };
    cancel_flag.store(false, Ordering::Relaxed);

    let case_dir = data_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id));
    }

    let mut manifest = downloader::manifest::read_manifest(&case_dir)?;

    if manifest.failed_assets.is_empty() {
        return Ok(manifest);
    }

    debug_log!("Retrying {} failed assets for case {}", manifest.failed_assets.len(), case_id);

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
        debug_log!("aaonline.fr is unreachable — skipped {} aaonline assets", skipped);
        if assets_to_retry.is_empty() {
            return Err("aaonline.fr is currently unreachable. Please try again later.".to_string());
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

    debug_log!(
        "Retry complete: {} newly downloaded, {} still failed",
        result.downloaded.len(),
        manifest.failed_assets.len()
    );

    Ok(manifest)
}

/// Update an existing case by re-fetching case data from AAO.
/// If `redownload_assets` is false, only re-fetches script/dialog data and downloads NEW assets.
/// If `redownload_assets` is true, re-downloads all assets (full update).
#[tauri::command]
pub async fn update_case(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    redownload_assets: bool,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, String> {
    let (engine_dir, data_dir, concurrency, cancel_flag, client) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.engine_dir.clone(), s.data_dir.clone(), s.config.concurrent_downloads, s.cancel_flag.clone(), s.http_client.clone())
    };
    cancel_flag.store(false, Ordering::Relaxed);

    let case_dir = data_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id));
    }

    // Read old manifest to know what we already have
    let old_manifest = downloader::manifest::read_manifest(&case_dir)?;

    // 1. Fetch site paths
    debug_log!("Update: fetching site paths...");
    let site_paths = downloader::case_fetcher::fetch_site_paths(&client).await?;

    // 2. Fetch case data
    debug_log!("Update: fetching case {} data...", case_id);
    let (case_info, trial_data, info_json, data_json) =
        downloader::case_fetcher::fetch_case(&client, case_id).await?;

    // 3. Save updated trial_info.json
    let info_pretty: serde_json::Value = serde_json::from_str(&info_json)
        .map_err(|e| format!("Failed to reparse info JSON: {}", e))?;
    fs::write(
        case_dir.join("trial_info.json"),
        serde_json::to_string_pretty(&info_pretty)
            .map_err(|e| format!("Failed to serialize trial_info: {}", e))?,
    )
    .map_err(|e| format!("Failed to write trial_info.json: {}", e))?;

    // 4. Extract and classify asset URLs from new trial data
    let mut assets = downloader::asset_resolver::extract_asset_urls(&trial_data, &site_paths, &engine_dir);
    let default_sprites = downloader::asset_resolver::extract_default_sprite_assets(
        &trial_data, &site_paths, &engine_dir,
    );
    let default_places = downloader::asset_resolver::extract_default_place_assets(
        &engine_dir, &site_paths,
    );
    let existing_urls: std::collections::HashSet<String> = assets.iter().map(|a| a.url.clone()).collect();
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
    let total_assets = assets.len();
    let (case_specific, shared) = downloader::asset_resolver::classify_assets(assets);
    let case_specific_count = case_specific.len();
    let shared_count = shared.len();

    // 5. Filter what to download based on mode
    let to_download = if redownload_assets {
        // Full update: download all case-specific + missing defaults
        let missing_defaults: Vec<_> = shared
            .into_iter()
            .filter(|a| !a.local_path.is_empty() && !asset_exists_on_disk(&data_dir, &a.local_path))
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
            if !asset.local_path.is_empty() && asset_exists_on_disk(&data_dir, &asset.local_path) {
                continue;
            }
            new_assets.push(asset);
        }

        // Shared/default: only download if missing from disk
        let missing_defaults: Vec<_> = shared
            .into_iter()
            .filter(|a| !a.local_path.is_empty() && !asset_exists_on_disk(&data_dir, &a.local_path))
            .collect();
        new_assets.extend(missing_defaults);
        new_assets
    };

    debug_log!(
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

    debug_log!(
        "Update complete: {} new downloads, {} failed",
        result.downloaded.len(),
        manifest.failed_assets.len()
    );

    Ok(manifest)
}
