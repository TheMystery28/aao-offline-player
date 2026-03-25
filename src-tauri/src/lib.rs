mod collections;
mod config;
mod downloader;
mod importer;
mod server;
pub mod utils;

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::{Manager, State};
use downloader::asset_downloader::DownloadEvent;

/// Engine files embedded at compile time by build.rs via include_bytes!.
/// Used on Android to extract engine files to the writable filesystem.
/// This bypasses Tauri's fs plugin which corrupts binary data on Android.
include!(concat!(env!("OUT_DIR"), "/engine_embed.rs"));

/// Print only in debug builds.
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            println!($($arg)*);
        }
    };
}

/// Shared state holding the asset server port, engine directory, and user config.
struct AppState {
    server_port: u16,
    /// Static engine files (JS, CSS, HTML, img, Languages). Read-only on mobile.
    engine_dir: PathBuf,
    /// Writable data directory (case/, defaults/, config.json).
    /// On desktop this equals engine_dir. On Android/iOS it's the app's private data dir.
    data_dir: PathBuf,
    config: config::AppConfig,
}

/// Returns the localhost URL for playing a specific case, including language preference.
#[tauri::command]
fn open_game(state: State<'_, Mutex<AppState>>, case_id: u32) -> Result<String, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(format!(
        "http://localhost:{}/player.html?trial_id={}&lang={}",
        state.server_port, case_id, state.config.language
    ))
}

/// Returns the asset server's base URL.
#[tauri::command]
fn get_server_url(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(format!("http://localhost:{}", state.server_port))
}

/// Lightweight command: fetch case metadata (including sequence info) without downloading assets.
#[tauri::command]
async fn fetch_case_info(case_id: u32) -> Result<downloader::CaseInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let site_paths = downloader::case_fetcher::fetch_site_paths(&client).await?;
    let _ = site_paths; // Not needed, but fetch_case requires site_paths to exist first

    let (case_info, _trial_data, _info_json, _data_json) =
        downloader::case_fetcher::fetch_case(&client, case_id).await?;

    Ok(case_info)
}

/// Download all parts of a sequence. Skips cases already downloaded.
/// Emits SequenceProgress events before each part, plus per-part asset download events.
#[tauri::command]
async fn download_sequence(
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
    on_event: Channel<DownloadEvent>,
) -> Result<Vec<downloader::manifest::CaseManifest>, String> {
    let (engine_dir, data_dir, concurrency) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.engine_dir.clone(), s.data_dir.clone(), s.config.concurrent_downloads)
    };

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

        // Run the full download pipeline (same as download_case body)
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

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
            if !existing_urls.contains(&sprite.url) {
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
            if data_dir.join(&a.local_path).exists() {
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

        let result = downloader::asset_downloader::download_assets(
            &client,
            to_download,
            &case_dir,
            &data_dir,
            &on_event,
            concurrency,
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
    });

    Ok(manifests)
}

/// Full download pipeline: fetch case → extract assets → download case-specific → generate manifest.
/// Streams progress events to the frontend via Channel.
#[tauri::command]
async fn download_case(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, String> {
    let (engine_dir, data_dir, concurrency) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.engine_dir.clone(), s.data_dir.clone(), s.config.concurrent_downloads)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

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

    // Deduplicate: some assets may already be in the list
    let existing_urls: std::collections::HashSet<String> = assets.iter().map(|a| a.url.clone()).collect();
    let new_count = default_sprites.iter().filter(|s| !existing_urls.contains(&s.url)).count();
    for sprite in default_sprites {
        if !existing_urls.contains(&sprite.url) {
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
        if disk_path.exists() {
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
    let result = downloader::asset_downloader::download_assets(
        &client,
        to_download,
        &case_dir,
        &data_dir,
        &on_event,
        concurrency,
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

    Ok(manifest)
}

/// Retry downloading failed assets for a case.
/// Reads the manifest to find failed assets, re-attempts download, updates manifest.
#[tauri::command]
async fn retry_failed_assets(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, String> {
    let (data_dir, concurrency) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.data_dir.clone(), s.config.concurrent_downloads)
    };

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

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

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

    let result = downloader::asset_downloader::download_assets(
        &client,
        assets_to_retry,
        &case_dir,
        &data_dir,
        &on_event,
        concurrency,
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
async fn update_case(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    redownload_assets: bool,
    on_event: Channel<DownloadEvent>,
) -> Result<downloader::manifest::CaseManifest, String> {
    let (engine_dir, data_dir, concurrency) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.engine_dir.clone(), s.data_dir.clone(), s.config.concurrent_downloads)
    };

    let case_dir = data_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id));
    }

    // Read old manifest to know what we already have
    let old_manifest = downloader::manifest::read_manifest(&case_dir)?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

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
        if !existing_urls.contains(&sprite.url) {
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
            .filter(|a| !a.local_path.is_empty() && !data_dir.join(&a.local_path).exists())
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
            if !asset.local_path.is_empty() && data_dir.join(&asset.local_path).exists() {
                continue;
            }
            new_assets.push(asset);
        }

        // Shared/default: only download if missing from disk
        let missing_defaults: Vec<_> = shared
            .into_iter()
            .filter(|a| !a.local_path.is_empty() && !data_dir.join(&a.local_path).exists())
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
    let result = downloader::asset_downloader::download_assets(
        &client,
        to_download,
        &case_dir,
        &data_dir,
        &on_event,
        concurrency,
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

/// List all downloaded cases by scanning the case directory for manifests.
#[tauri::command]
fn list_cases(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<downloader::manifest::CaseManifest>, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    let cases_dir = data_dir.join("case");
    if !cases_dir.exists() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    let entries =
        fs::read_dir(&cases_dir).map_err(|e| format!("Failed to read cases directory: {}", e))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("manifest.json");
        if manifest_path.exists() {
            match downloader::manifest::read_manifest(&path) {
                Ok(manifest) => cases.push(manifest),
                Err(e) => {
                    debug_log!("Warning: skipping {}: {}", path.display(), e);
                }
            }
        }
    }

    // Sort by title
    cases.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(cases)
}

/// Delete a downloaded case and all its files.
#[tauri::command]
fn delete_case(state: State<'_, Mutex<AppState>>, case_id: u32) -> Result<(), String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    let case_dir = data_dir.join("case").join(case_id.to_string());
    if !case_dir.exists() {
        return Err(format!("Case {} not found", case_id));
    }

    fs::remove_dir_all(&case_dir)
        .map_err(|e| format!("Failed to delete case {}: {}", case_id, e))?;

    debug_log!("Deleted case {} at {}", case_id, case_dir.display());
    Ok(())
}

// ── Collection commands ──

/// Back up game saves to a file in the data directory.
/// Called from JS after reading saves from localStorage via the bridge.
#[tauri::command]
fn backup_saves(
    state: State<'_, Mutex<AppState>>,
    saves: serde_json::Value,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let path = data_dir.join("saves_backup.json");
    let json = serde_json::to_string(&saves)
        .map_err(|e| format!("Failed to serialize saves: {}", e))?;
    fs::write(&path, json)
        .map_err(|e| format!("Failed to write saves backup: {}", e))?;
    Ok(())
}

/// Read backed-up saves from the data directory.
/// Returns the saves JSON or null if no backup exists.
#[tauri::command]
fn load_saves_backup(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<serde_json::Value>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let path = data_dir.join("saves_backup.json");
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read saves backup: {}", e))?;
    let value: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse saves backup: {}", e))?;
    Ok(Some(value))
}

/// Read saves from the backup file, filtered by case IDs.
/// Returns the saves for only the requested cases, or None if no backup or no matching saves.
#[tauri::command]
fn read_saves_for_export(
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
) -> Result<Option<serde_json::Value>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let path = data_dir.join("saves_backup.json");
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read saves backup: {}", e))?;
    let all: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse saves backup: {}", e))?;

    let mut filtered = serde_json::Map::new();
    for id in &case_ids {
        let key = id.to_string();
        if let Some(val) = all.get(&key) {
            filtered.insert(key, val.clone());
        }
    }
    if filtered.is_empty() {
        return Ok(None);
    }
    Ok(Some(serde_json::Value::Object(filtered)))
}

/// Find the latest save across the given case IDs from the disk backup.
/// Returns { partId, saveDate, saveString } or null.
#[tauri::command]
fn find_latest_save(
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
) -> Result<Option<serde_json::Value>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let path = data_dir.join("saves_backup.json");
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read saves backup: {}", e))?;
    let all: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse saves backup: {}", e))?;

    let mut latest_ts: u64 = 0;
    let mut latest_part: Option<u32> = None;
    let mut latest_save: Option<String> = None;

    for &id in &case_ids {
        let key = id.to_string();
        if let Some(saves) = all.get(&key).and_then(|v| v.as_object()) {
            for (ts_str, save_val) in saves {
                if let Ok(ts) = ts_str.parse::<u64>() {
                    if ts > latest_ts {
                        latest_ts = ts;
                        latest_part = Some(id);
                        latest_save = save_val.as_str().map(|s| s.to_string());
                    }
                }
            }
        }
    }

    match (latest_part, latest_save) {
        (Some(part_id), Some(save_string)) => Ok(Some(serde_json::json!({
            "partId": part_id,
            "saveDate": latest_ts,
            "saveString": save_string
        }))),
        _ => Ok(None),
    }
}

/// List all collections.
#[tauri::command]
fn list_collections(state: State<'_, Mutex<AppState>>) -> Result<Vec<collections::Collection>, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let data = collections::load_collections(&data_dir);
    Ok(data.collections)
}

/// Create a new collection with the given title and initial items.
#[tauri::command]
fn create_collection(
    state: State<'_, Mutex<AppState>>,
    title: String,
    items: Vec<collections::CollectionItem>,
) -> Result<collections::Collection, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let mut data = collections::load_collections(&data_dir);
    let collection = collections::Collection {
        id: collections::generate_id(),
        title,
        items,
        created_date: collections::now_iso8601(),
    };
    data.collections.push(collection.clone());
    collections::save_collections(&data_dir, &data)?;
    Ok(collection)
}

/// Update a collection's title and/or items. Only provided fields are changed.
#[tauri::command]
fn update_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
    title: Option<String>,
    items: Option<Vec<collections::CollectionItem>>,
) -> Result<collections::Collection, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let mut data = collections::load_collections(&data_dir);
    let coll = data
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Collection {} not found", id))?;
    if let Some(t) = title {
        coll.title = t;
    }
    if let Some(i) = items {
        coll.items = i;
    }
    let result = coll.clone();
    collections::save_collections(&data_dir, &data)?;
    Ok(result)
}

/// Delete a collection by ID. Does not delete the underlying cases.
#[tauri::command]
fn delete_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let mut data = collections::load_collections(&data_dir);
    let len_before = data.collections.len();
    data.collections.retain(|c| c.id != id);
    if data.collections.len() == len_before {
        return Err(format!("Collection {} not found", id));
    }
    collections::save_collections(&data_dir, &data)?;
    Ok(())
}

/// Get a single collection by ID.
#[tauri::command]
fn get_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<collections::Collection, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let data = collections::load_collections(&data_dir);
    data.collections
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Collection {} not found", id))
}

/// Append items to an existing collection.
#[tauri::command]
fn add_to_collection(
    state: State<'_, Mutex<AppState>>,
    id: String,
    items: Vec<collections::CollectionItem>,
) -> Result<collections::Collection, String> {
    let data_dir = state.lock().map_err(|e| e.to_string())?.data_dir.clone();
    let mut data = collections::load_collections(&data_dir);
    let coll = data
        .collections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Collection {} not found", id))?;
    coll.items.extend(items);
    let result = coll.clone();
    collections::save_collections(&data_dir, &data)?;
    Ok(result)
}

// ── Settings commands ──

/// Return current user settings.
#[tauri::command]
fn get_settings(state: State<'_, Mutex<AppState>>) -> Result<config::AppConfig, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.config.clone())
}

/// Save user settings (validated and clamped).
#[tauri::command]
fn save_settings(
    state: State<'_, Mutex<AppState>>,
    settings: config::AppConfig,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let mut validated = settings;
    config::validate(&mut validated);
    config::save_config(&s.data_dir, &validated)?;
    s.config = validated;
    Ok(())
}

/// Return storage usage statistics.
#[tauri::command]
fn get_storage_info(state: State<'_, Mutex<AppState>>) -> Result<config::StorageInfo, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    Ok(config::compute_storage_info(&data_dir))
}

/// Delete all default asset cache files. Returns bytes actually freed.
#[tauri::command]
fn clear_default_cache(state: State<'_, Mutex<AppState>>) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    let defaults_dir = data_dir.join("defaults");
    let size_before = config::dir_size(&defaults_dir);
    if defaults_dir.exists() {
        if let Ok(entries) = fs::read_dir(&defaults_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let _ = fs::remove_dir_all(&path);
                } else {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
    let size_after = config::dir_size(&defaults_dir);
    let freed = size_before.saturating_sub(size_after);
    debug_log!("Cleared default asset cache ({} bytes freed, {} remaining)", freed, size_after);
    Ok(freed)
}

/// Open the data directory in the system file explorer.
#[tauri::command]
fn open_data_dir(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    let path_str = data_dir.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }
    #[cfg(target_os = "android")]
    {
        // Android has no system file explorer for app-internal storage.
        // Return the path so the frontend can display it to the user.
        debug_log!("Data directory (Android): {}", path_str);
    }
    Ok(())
}

/// Open a native folder picker dialog. Returns the selected path or null if cancelled.
/// On Android, folder picking is not supported — returns an error.
#[tauri::command]
async fn pick_folder(_app: tauri::AppHandle) -> Result<Option<String>, String> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let result = _app
            .dialog()
            .file()
            .set_title("Select aaoffline download folder")
            .blocking_pick_folder();
        match result {
            Some(file_path) => {
                let path = file_path
                    .into_path()
                    .map_err(|e| format!("Invalid path: {}", e))?;
                Ok(Some(path.to_string_lossy().to_string()))
            }
            None => Ok(None),
        }
    }
    #[cfg(target_os = "android")]
    {
        Err("Folder picking is not supported on Android. Use file import instead.".to_string())
    }
}

/// Open a native file picker dialog for .aaocase/.zip files. Returns the selected path or null.
///
/// On Android, the dialog returns `content://` URIs instead of filesystem paths.
/// The import_case command handles both formats.
#[tauri::command]
async fn pick_import_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app
        .dialog()
        .file()
        .set_title("Select .aaocase, .aaoplug, or .aaosave file");

    // On Android, the SAF uses MIME types instead of file extensions.
    if cfg!(target_os = "android") {
        builder = builder.add_filter("AAO Files", &["application/zip", "application/octet-stream"]);
    } else {
        builder = builder.add_filter("AAO Files", &["aaocase", "aaoplug", "aaosave", "zip"]);
    }

    let result = builder.blocking_pick_file();
    match result {
        Some(file_path) => {
            // On desktop: into_path() gives a filesystem path.
            // On Android: into_path() fails for content:// URIs.
            // Try path conversion first, fall back to path() for URI.
            if let Some(path) = file_path.as_path() {
                Ok(Some(path.to_string_lossy().to_string()))
            } else {
                // Android content:// URI — convert to string for import_case.
                // import_case will copy it to a temp file via Tauri's fs plugin.
                Ok(Some(file_path.to_string()))
            }
        }
        None => Ok(None),
    }
}

/// Import a case from an existing aaoffline download directory or a .aaocase ZIP file.
///
/// - If `source_path` is a directory: expects `index.html` + optional `assets/` (aaoffline format)
/// - If `source_path` is a file: expects a .aaocase or .zip file
/// - If `source_path` is a content:// URI (Android): copies to temp file first via Tauri fs plugin
///
/// Returns `ImportResult` containing the manifest and optionally any game saves.
#[tauri::command]
fn import_case(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    source_path: String,
    on_event: Channel<DownloadEvent>,
) -> Result<importer::ImportResult, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    // On Android, the file picker returns content:// URIs which aren't regular filesystem paths.
    // Copy the file to a temp location using Tauri's fs plugin (handles content URIs).
    let (path, _temp_file) = if source_path.starts_with("content://") {
        use tauri_plugin_fs::FsExt;
        debug_log!("Android content URI detected: {}", source_path);

        let _ = on_event.send(DownloadEvent::Started { total: 1 });
        let _ = on_event.send(DownloadEvent::Progress {
            completed: 0, total: 1,
            current_url: "Reading file...".to_string(),
        });

        let url = reqwest::Url::parse(&source_path)
            .map_err(|e| format!("Failed to parse content URI: {}", e))?;
        let file_path = tauri_plugin_fs::FilePath::from(url);
        let content = app.fs().read(file_path)
            .map_err(|e| format!("Failed to read from content URI: {}", e))?;

        let temp_path = data_dir.join("_import_temp.aaocase");
        fs::write(&temp_path, &content)
            .map_err(|e| format!("Failed to write temp import file: {}", e))?;

        debug_log!("Copied {} bytes from content URI to {}", content.len(), temp_path.display());
        (temp_path.clone(), Some(temp_path)) // _temp_file keeps the path for cleanup
    } else {
        let p = PathBuf::from(&source_path);
        if !p.exists() {
            return Err(format!("Path not found: {}", source_path));
        }
        (p, None)
    };

    let progress_cb = |completed: usize, total: usize| {
        let _ = on_event.send(DownloadEvent::Progress {
            completed,
            total,
            current_url: format!("{}/{}", completed, total),
        });
    };

    let import_result = if path.is_dir() {
        let has_subfolders = !importer::find_aaoffline_subfolders(&path).is_empty();
        if has_subfolders {
            // Parent folder with case subfolders (may also have root index.html — batch handles both)
            let _ = on_event.send(DownloadEvent::Started { total: 0 });
            let case_progress_cb = |current: usize, total: usize, name: &str| {
                let _ = on_event.send(DownloadEvent::SequenceProgress {
                    current_part: current,
                    total_parts: total,
                    part_title: format!("Importing: {}", name),
                });
            };
            importer::import_aaoffline_batch(&path, &data_dir, Some(&case_progress_cb), Some(&progress_cb))?
        } else if path.join("index.html").exists() {
            // Single aaoffline case folder (no subfolders)
            let _ = on_event.send(DownloadEvent::Started { total: 0 });
            let manifest = importer::import_aaoffline(&path, &data_dir, Some(&progress_cb))?;
            importer::ImportResult { manifest, saves: None, missing_defaults: 0, batch_manifests: Vec::new(), batch_errors: Vec::new() }
        } else {
            return Err(format!(
                "No index.html found in {} and no subfolders with cases found either.",
                path.display()
            ));
        }
    } else if path.is_file() {
        let _ = on_event.send(DownloadEvent::Started { total: 0 });
        importer::import_aaocase_zip(&path, &data_dir, Some(&progress_cb))?
    } else {
        return Err(format!("Not a file or directory: {}", source_path));
    };

    // Clean up temp file if we created one
    if let Some(temp) = _temp_file {
        let _ = fs::remove_file(&temp);
    }

    // Sum up totals for batch imports
    let (total_downloaded, total_bytes) = if !import_result.batch_manifests.is_empty() {
        import_result.batch_manifests.iter().fold((0usize, 0u64), |(d, b), m| {
            (d + m.assets.total_downloaded, b + m.assets.total_size_bytes)
        })
    } else {
        (import_result.manifest.assets.total_downloaded, import_result.manifest.assets.total_size_bytes)
    };

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: total_downloaded,
        failed: 0,
        total_bytes,
    });

    debug_log!(
        "Imported case {} \"{}\" ({} assets, {} bytes{})",
        import_result.manifest.case_id,
        import_result.manifest.title,
        import_result.manifest.assets.total_downloaded,
        import_result.manifest.assets.total_size_bytes,
        if import_result.saves.is_some() { ", with saves" } else { "" }
    );

    Ok(import_result)
}

/// Import a .aaoplug plugin file into one or more existing cases.
#[tauri::command]
fn import_plugin(
    state: State<'_, Mutex<AppState>>,
    source_path: String,
    target_case_ids: Vec<u32>,
) -> Result<Vec<u32>, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    let path = std::path::PathBuf::from(&source_path);
    importer::import_aaoplug(&path, &target_case_ids, &data_dir)
}

/// Attach raw plugin JS code to one or more existing cases.
#[tauri::command]
fn attach_plugin_code(
    state: State<'_, Mutex<AppState>>,
    code: String,
    filename: String,
    target_case_ids: Vec<u32>,
) -> Result<Vec<u32>, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    importer::attach_plugin_code(&code, &filename, &target_case_ids, &data_dir)
}

/// List plugins installed for a given case.
#[tauri::command]
fn list_plugins(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
) -> Result<serde_json::Value, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    importer::list_plugins(case_id, &data_dir)
}

/// Remove a plugin from a case by filename.
#[tauri::command]
fn remove_plugin(
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    filename: String,
) -> Result<(), String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    importer::remove_plugin(case_id, &filename, &data_dir)
}

/// Export saves as a .aaosave file.
#[tauri::command]
fn export_save(
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
    saves: serde_json::Value,
    include_plugins: bool,
    dest_path: String,
) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    let path = std::path::PathBuf::from(&dest_path);
    importer::export_aaosave(&case_ids, &saves, include_plugins, &path, &data_dir)
}

/// Import saves from a .aaosave file.
#[tauri::command]
fn import_save(
    state: State<'_, Mutex<AppState>>,
    source_path: String,
) -> Result<importer::ImportSaveResult, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    let path = std::path::PathBuf::from(&source_path);
    importer::import_aaosave(&path, &data_dir)
}

/// Open a native "Save As" dialog for exporting a .aaosave file.
#[tauri::command]
async fn pick_export_save_file(app: tauri::AppHandle, default_name: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app
        .dialog()
        .file()
        .set_title("Export saves as .aaosave")
        .set_file_name(&default_name);

    if !cfg!(target_os = "android") {
        builder = builder.add_filter("AAO Save", &["aaosave"]);
    }

    let result = builder.blocking_save_file();
    match result {
        Some(file_path) => {
            if let Some(path) = file_path.as_path() {
                Ok(Some(path.to_string_lossy().to_string()))
            } else {
                Ok(Some(file_path.to_string()))
            }
        }
        None => Ok(None),
    }
}

/// Open a native "Save As" dialog for exporting a .aaocase file.
/// `default_name` is the suggested filename (e.g. "My Case.aaocase").
#[tauri::command]
async fn pick_export_file(app: tauri::AppHandle, default_name: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app
        .dialog()
        .file()
        .set_title("Export case as .aaocase")
        .set_file_name(&default_name);

    // On Android, extension filters don't work — use MIME type
    if !cfg!(target_os = "android") {
        builder = builder.add_filter("AAO Case", &["aaocase"]);
    }

    let result = builder.blocking_save_file();
    match result {
        Some(file_path) => {
            // On desktop: filesystem path. On Android: content:// URI.
            if let Some(path) = file_path.as_path() {
                Ok(Some(path.to_string_lossy().to_string()))
            } else {
                Ok(Some(file_path.to_string()))
            }
        }
        None => Ok(None),
    }
}

/// Export an entire sequence as a single .aaocase ZIP file.
/// If `saves` is provided, includes it as saves.json in the ZIP.
/// On Android, `dest_path` may be a content:// URI — exports to temp then copies.
#[tauri::command]
fn export_sequence(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
    sequence_title: String,
    sequence_list: serde_json::Value,
    dest_path: String,
    saves: Option<serde_json::Value>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    let (export_path, content_uri) = if dest_path.starts_with("content://") {
        let temp = data_dir.join("_export_temp.aaocase");
        (temp, Some(dest_path.clone()))
    } else {
        (PathBuf::from(&dest_path), None)
    };

    let _ = on_event.send(DownloadEvent::Started { total: 0 });

    let progress_cb = |completed: usize, total: usize| {
        let _ = on_event.send(DownloadEvent::Progress {
            completed,
            total,
            current_url: format!("{}/{}", completed, total),
        });
    };

    let size = importer::export_sequence(
        &case_ids,
        &sequence_title,
        &sequence_list,
        &data_dir,
        &export_path,
        Some(&progress_cb),
        saves.as_ref(),
    )?;

    if let Some(uri) = content_uri {
        use tauri_plugin_fs::FsExt;
        use std::io::Write;
        let data = fs::read(&export_path)
            .map_err(|e| format!("Failed to read temp export: {}", e))?;
        let dest_url = reqwest::Url::parse(&uri)
            .map_err(|e| format!("Failed to parse content URI: {}", e))?;
        let dest_fp = tauri_plugin_fs::FilePath::from(dest_url);
        let opts = tauri_plugin_fs::OpenOptions::new().write(true).create(true).clone();
        let mut file = app.fs().open(dest_fp, opts)
            .map_err(|e| format!("Failed to open content URI for writing: {}", e))?;
        file.write_all(&data)
            .map_err(|e| format!("Failed to write to content URI: {}", e))?;
        let _ = fs::remove_file(&export_path);
    }

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
    });

    Ok(size)
}

/// Export a collection as a .aaocase ZIP file.
#[tauri::command]
fn export_collection(
    state: State<'_, Mutex<AppState>>,
    collection_id: String,
    dest_path: String,
    saves: Option<serde_json::Value>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    let coll_data = collections::load_collections(&data_dir);
    let collection = coll_data.collections.iter()
        .find(|c| c.id == collection_id)
        .ok_or_else(|| format!("Collection {} not found", collection_id))?
        .clone();

    let export_path = PathBuf::from(&dest_path);
    let progress_cb = |completed: usize, total: usize| {
        let _ = on_event.send(DownloadEvent::Progress {
            completed, total,
            current_url: format!("{}/{}", completed, total),
        });
    };

    let size = importer::export_collection(&collection, &data_dir, &export_path, Some(&progress_cb), saves.as_ref())?;

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
    });

    Ok(size)
}

/// Export a case as a .aaocase ZIP file.
/// If `saves` is provided, includes it as saves.json in the ZIP.
/// On Android, `dest_path` may be a content:// URI — exports to temp then copies.
#[tauri::command]
fn export_case(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    dest_path: String,
    saves: Option<serde_json::Value>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    // On Android, dest_path is a content:// URI. Export to temp, then copy.
    let (export_path, content_uri) = if dest_path.starts_with("content://") {
        let temp = data_dir.join("_export_temp.aaocase");
        (temp, Some(dest_path.clone()))
    } else {
        (PathBuf::from(&dest_path), None)
    };

    let _ = on_event.send(DownloadEvent::Started { total: 0 });

    let progress_cb = |completed: usize, total: usize| {
        let _ = on_event.send(DownloadEvent::Progress {
            completed,
            total,
            current_url: format!("{}/{}", completed, total),
        });
    };

    let size = importer::export_aaocase(case_id, &data_dir, &export_path, Some(&progress_cb), saves.as_ref())?;

    // Copy temp file to content URI on Android
    if let Some(uri) = content_uri {
        use tauri_plugin_fs::FsExt;
        use std::io::Write;
        let data = fs::read(&export_path)
            .map_err(|e| format!("Failed to read temp export: {}", e))?;
        let dest_url = reqwest::Url::parse(&uri)
            .map_err(|e| format!("Failed to parse content URI: {}", e))?;
        let dest_fp = tauri_plugin_fs::FilePath::from(dest_url);
        let opts = tauri_plugin_fs::OpenOptions::new().write(true).create(true).clone();
        let mut file = app.fs().open(dest_fp, opts)
            .map_err(|e| format!("Failed to open content URI for writing: {}", e))?;
        file.write_all(&data)
            .map_err(|e| format!("Failed to write to content URI: {}", e))?;
        let _ = fs::remove_file(&export_path);
    }

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
    });

    debug_log!(
        "Exported case {} to {} ({} bytes)",
        case_id,
        dest_path,
        size
    );

    Ok(size)
}

/// Extract engine files from the embedded binary data to the writable filesystem.
///
/// Engine files are embedded at compile time via `include_bytes!` in build.rs.
/// This avoids Tauri's `app.fs().read()` which corrupts binary data (GIFs, fonts)
/// when reading from APK assets on Android. The embedded data is byte-identical
/// to the original files from the build machine.
fn extract_engine_files(dest: &std::path::Path) -> Result<(), String> {
    debug_log!(
        "Extracting {} engine files to {}...",
        EMBEDDED_ENGINE_FILES.len(),
        dest.display()
    );

    for (name, data) in EMBEDDED_ENGINE_FILES {
        let dest_path = dest.join(name);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory for '{}': {}", name, e))?;
        }
        fs::write(&dest_path, data)
            .map_err(|e| format!("Failed to write '{}': {}", name, e))?;
    }

    debug_log!("Engine files extracted successfully");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Determine engine_dir and data_dir based on platform.
            //
            // Desktop (Windows/macOS/Linux):
            //   engine_dir = resource_dir/engine (installed) or source engine/ (dev mode)
            //   data_dir = engine_dir (everything in one writable directory)
            //
            // Mobile (Android/iOS):
            //   data_dir = app_data_dir/engine (writable private storage)
            //   Engine files are bundled inside the APK — not on the filesystem.
            //   On first launch, extract them from APK assets to data_dir.
            //   engine_dir = data_dir (both point to the same writable directory)
            let (engine_dir, data_dir) = if cfg!(target_os = "android") || cfg!(target_os = "ios") {
                let dir = app.path().app_data_dir()
                    .expect("failed to resolve app data dir")
                    .join("engine");
                fs::create_dir_all(&dir)
                    .expect("failed to create data directory");

                // Extract bundled engine files from APK on first launch.
                // On Android, bundle.resources are inside the APK (not on filesystem).
                // We use Tauri's fs plugin to read them and write to the writable dir.
                if !dir.join("player.html").exists() {
                    extract_engine_files(&dir)
                        .expect("failed to extract engine files");
                }

                // On mobile, both dirs point to the same writable location
                (dir.clone(), dir)
            } else {
                // Desktop: in dev mode, serve directly from source engine/ so edits
                // are reflected immediately without manual copy to target/debug/engine/.
                // In release, use resource_dir/engine (bundled by installer).
                let engine_dir = if cfg!(debug_assertions) {
                    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                    manifest_dir.parent().unwrap().join("engine")
                } else {
                    app.path()
                        .resource_dir()
                        .ok()
                        .map(|d| d.join("engine"))
                        .filter(|d| d.exists())
                        .unwrap_or_else(|| {
                            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                            manifest_dir.parent().unwrap().join("engine")
                        })
                };
                // In dev mode, data_dir stays in target/debug/engine for runtime
                // data (cases, defaults, config). In release, same as engine_dir.
                let data_dir = if cfg!(debug_assertions) {
                    app.path()
                        .resource_dir()
                        .ok()
                        .map(|d| d.join("engine"))
                        .filter(|d| d.exists())
                        .unwrap_or_else(|| engine_dir.clone())
                } else {
                    engine_dir.clone()
                };
                (engine_dir, data_dir)
            };

            // Load user config from writable data dir
            let app_config = config::load_config(&data_dir);
            debug_log!("Loaded config: {:?}", app_config);

            // Start the custom asset server
            let port = server::start_server(server::ServerConfig {
                engine_dir: engine_dir.clone(),
                data_dir: data_dir.clone(),
            });

            debug_log!("Asset server started on http://localhost:{}", port);
            debug_log!("Engine directory: {}", engine_dir.display());
            debug_log!("Data directory: {}", data_dir.display());

            // Write port file so external scripts (e.g. test runner) can find the server
            let port_file = data_dir.join(".server_port");
            let _ = fs::write(&port_file, port.to_string());

            // Store state for commands
            app.manage(Mutex::new(AppState {
                server_port: port,
                engine_dir,
                data_dir,
                config: app_config,
            }));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_game,
            get_server_url,
            fetch_case_info,
            download_case,
            download_sequence,
            update_case,
            retry_failed_assets,
            list_cases,
            delete_case,
            backup_saves,
            load_saves_backup,
            read_saves_for_export,
            find_latest_save,
            list_collections,
            create_collection,
            update_collection,
            delete_collection,
            get_collection,
            add_to_collection,
            export_collection,
            get_settings,
            save_settings,
            get_storage_info,
            clear_default_cache,
            open_data_dir,
            pick_folder,
            pick_import_file,
            import_case,
            import_plugin,
            attach_plugin_code,
            list_plugins,
            remove_plugin,
            export_save,
            import_save,
            pick_export_save_file,
            pick_export_file,
            export_case,
            export_sequence
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
