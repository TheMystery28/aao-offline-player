use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use reqwest::Client;
use tauri::ipc::Channel;

use crate::error::AppError;
use super::log::DownloadLog;
use super::url_encoding::encode_url;
use super::utils::{check_skip_existing, generate_filename};
use super::{DownloadEvent, DownloadResult, DownloadedAsset};
use crate::downloader::dedup::{DedupIndex, check_and_promote};
use crate::downloader::manifest::FailedAsset;
use crate::downloader::AssetRef;

const DEFAULT_CONCURRENCY: usize = 3;
const MAX_RETRIES: u32 = 3;
const BASE_RETRY_DELAY: Duration = Duration::from_secs(2);
pub(super) const PER_ASSET_TIMEOUT: Duration = Duration::from_secs(15);

/// Download assets with progress reporting and retry logic.
/// - Assets with `local_path` set are saved to `engine_dir/{local_path}` (internal AAO assets).
/// - Assets with empty `local_path` are saved to `case_dir/assets/{hash}` (external assets).
pub async fn download_assets(
    client: &Client,
    assets: Vec<AssetRef>,
    case_dir: &PathBuf,
    engine_dir: &PathBuf,
    dedup_index: Option<&DedupIndex>,
    on_event: &Channel<DownloadEvent>,
    concurrency: usize,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<DownloadResult, AppError> {
    let total = assets.len();
    let concurrency = if concurrency == 0 { DEFAULT_CONCURRENCY } else { concurrency };
    let case_assets_dir = case_dir.join("assets");
    std::fs::create_dir_all(&case_assets_dir)
        .map_err(|e| format!("Failed to create assets directory: {}", e))?;

    let log = Arc::new(DownloadLog::new(&case_dir.join("download_log.txt"))?);
    log.log(&format!(
        "=== Download started: {} assets, concurrency={}, max_retries={} ===",
        total, concurrency, MAX_RETRIES
    ));

    for (i, asset) in assets.iter().enumerate() {
        let save_to = if asset.local_path.is_empty() {
            format!("case/assets/{}", generate_filename(&asset.url))
        } else {
            asset.local_path.clone()
        };
        log.log(&format!(
            "  QUEUED [{}] type={} save={} url={}",
            i, asset.asset_type, save_to, asset.url
        ));
    }

    on_event.send(DownloadEvent::Started { total }).ok();

    let completed = Arc::new(AtomicUsize::new(0));
    let total_bytes = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let start_time = std::time::Instant::now();

    let results: Vec<Result<DownloadedAsset, FailedAsset>> = stream::iter(assets.into_iter())
        .map(|asset| {
            let client = client.clone();
            let case_base = case_dir.clone();
            let engine = engine_dir.clone();
            let completed = completed.clone();
            let total_bytes = total_bytes.clone();
            let failed = failed.clone();
            let on_event = on_event.clone();
            let log = log.clone();
            let cancel_flag = cancel_flag.clone();
            // Move owned AssetRef fields directly — avoids 3 String clones per asset
            let url = asset.url;
            let asset_type = asset.asset_type;
            let local_path = asset.local_path;

            async move {
                // Check cancel before each asset
                if cancel_flag.load(Ordering::Relaxed) {
                    let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    on_event.send(DownloadEvent::Progress {
                        completed: done, total, current_url: url.clone(),
                        bytes_downloaded: total_bytes.load(Ordering::Relaxed),
                        elapsed_ms: start_time.elapsed().as_millis() as u64,
                    }).ok();
                    return Err(FailedAsset {
                        url, asset_type, local_path: String::new(),
                        error: "Cancelled".to_string(),
                    });
                }

                // Determine save path
                let (save_dir, relative_path) = if local_path.is_empty() {
                    // External asset → case_dir/assets/{hash} (save_dir=case_dir, rel=assets/{hash})
                    let filename = generate_filename(&url);
                    (case_base.clone(), format!("assets/{}", filename))
                } else {
                    // Internal asset → engine/{local_path}
                    (engine.clone(), local_path.clone())
                };

                // Skip if file already exists locally (avoids re-downloading defaults)
                if let Some(size) = check_skip_existing(&save_dir, &relative_path) {
                    log.log(&format!(
                        "  SKIP_EXISTS size={} file={} url={}",
                        size, relative_path, url
                    ));
                    let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    on_event
                        .send(DownloadEvent::Progress {
                            completed: done,
                            total,
                            current_url: url.clone(),
                            bytes_downloaded: total_bytes.load(Ordering::Relaxed),
                            elapsed_ms: start_time.elapsed().as_millis() as u64,
                        })
                        .ok();
                    // Compute hash from existing file for accurate dedup index.
                    // If the file is a VFS pointer, resolve to the real target before hashing.
                    let file_path = save_dir.join(&relative_path);
                    let hash_path = crate::downloader::vfs::resolve_path(&file_path, &save_dir, &save_dir);
                    let content_hash = std::fs::read(&hash_path)
                        .map(|bytes| xxhash_rust::xxh3::xxh3_64(&bytes))
                        .unwrap_or(0);
                    return Ok(DownloadedAsset {
                        original_url: url,
                        local_path: relative_path,
                        size,
                        content_hash,
                    });
                }

                match download_with_retry(&client, &url, &save_dir, &relative_path, &log, &asset_type).await {
                    Ok(mut result) => {
                        // Post-download dedup: check if identical content already exists
                        if let Some(idx) = dedup_index {
                            if let Some(existing) = check_and_promote(
                                &engine, result.content_hash, idx, None,
                            ) {
                                if existing != result.local_path {
                                    // If the just-saved file is a canonical default
                                    // (defaults/sounds/, defaults/images/, etc.) and the
                                    // existing match is in defaults/shared/, keep the
                                    // canonical one — the engine references it by fixed path.
                                    // Delete the shared copy instead.
                                    let saved_is_canonical = result.local_path.starts_with("defaults/")
                                        && !result.local_path.starts_with("defaults/shared/");
                                    let existing_is_shared = existing.starts_with("defaults/shared/");

                                    if saved_is_canonical && existing_is_shared {
                                        // Keep canonical, replace shared with VFS pointer.
                                        // Cannot delete shared — other cases may have VFS pointers to it.
                                        let shared_file = save_dir.join(&existing);
                                        let _ = std::fs::remove_file(&shared_file);
                                        let _ = crate::downloader::vfs::write_vfs_pointer(&shared_file, &result.local_path);
                                        let _ = idx.register(
                                            &result.local_path, result.size, result.content_hash,
                                        );
                                        log.log(&format!(
                                            "  DEDUP_KEEP_CANONICAL hash={:016x} keep={} shared_alias={}",
                                            result.content_hash, result.local_path, existing
                                        ));
                                    } else {
                                        // Write VFS pointer instead of deleting — one physical copy
                                        let saved = save_dir.join(&result.local_path);
                                        let _ = std::fs::remove_file(&saved);
                                        let _ = crate::downloader::vfs::write_vfs_pointer(&saved, &existing);
                                        log.log(&format!(
                                            "  DEDUP_VFS_ALIAS hash={:016x} pointer={} -> target={}",
                                            result.content_hash, result.local_path, existing
                                        ));
                                        result.local_path = existing;
                                    }
                                }
                            } else {
                                // No match — register the new file in the index
                                let _ = idx.register(
                                    &result.local_path, result.size, result.content_hash,
                                );
                            }
                        }

                        total_bytes.fetch_add(result.size, Ordering::Relaxed);
                        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                        on_event
                            .send(DownloadEvent::Progress {
                                completed: done,
                                total,
                                current_url: url.clone(),
                                bytes_downloaded: total_bytes.load(Ordering::Relaxed),
                                elapsed_ms: start_time.elapsed().as_millis() as u64,
                            })
                            .ok();
                        Ok(result)
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        failed.fetch_add(1, Ordering::Relaxed);
                        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                        log.log(&format!(
                            "  FINAL_FAIL type={} url={} err={}",
                            asset_type, url, err_msg
                        ));
                        on_event
                            .send(DownloadEvent::Progress {
                                completed: done,
                                total,
                                current_url: url.clone(),
                                bytes_downloaded: total_bytes.load(Ordering::Relaxed),
                                elapsed_ms: start_time.elapsed().as_millis() as u64,
                            })
                            .ok();
                        Err(FailedAsset {
                            url,
                            asset_type,
                            local_path: relative_path,
                            error: err_msg,
                        })
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    let mut downloaded: Vec<DownloadedAsset> = Vec::new();
    let mut failed_assets: Vec<FailedAsset> = Vec::new();
    for result in results {
        match result {
            Ok(asset) => downloaded.push(asset),
            Err(fail) => failed_assets.push(fail),
        }
    }
    let fail_count = failed.load(Ordering::Relaxed);
    let bytes = total_bytes.load(Ordering::Relaxed);

    log.log(&format!(
        "=== Download finished: {} OK, {} FAILED, {} bytes total ===",
        downloaded.len(),
        fail_count,
        bytes
    ));

    on_event
        .send(DownloadEvent::Finished {
            downloaded: downloaded.len(),
            failed: fail_count,
            total_bytes: bytes,
            dedup_saved_bytes: 0,
        })
        .ok();

    Ok(DownloadResult {
        downloaded,
        failed: failed_assets,
    })
}

pub(crate) async fn download_with_retry(
    client: &Client,
    url: &str,
    base_dir: &PathBuf,
    relative_path: &str,
    log: &DownloadLog,
    asset_type: &str,
) -> Result<DownloadedAsset, AppError> {
    let mut last_err = String::new();

    for attempt in 0..MAX_RETRIES {
        log.log(&format!(
            "  ATTEMPT {}/{} type={} url={}",
            attempt + 1, MAX_RETRIES, asset_type, url
        ));

        match download_single_asset(client, url, base_dir, relative_path, log, asset_type).await {
            Ok(result) => {
                log.log(&format!(
                    "  OK size={} file={} url={}",
                    result.size, result.local_path, url
                ));
                return Ok(result);
            }
            Err(e) => {
                last_err = e.to_string();
                log.log(&format!(
                    "  ERR attempt={} err={} url={}",
                    attempt + 1, last_err, url
                ));

                let is_retryable = last_err.contains("429")
                    || last_err.contains("503")
                    || last_err.contains("502")
                    || last_err.contains("timeout")
                    || last_err.contains("connection")
                    || last_err.contains("reset")
                    || last_err.contains("closed");

                if !is_retryable && attempt == 0 {
                    return Err(last_err.into());
                }

                if attempt < MAX_RETRIES - 1 {
                    let delay = BASE_RETRY_DELAY * 2u32.pow(attempt);
                    log.log(&format!("  WAIT {:?} before retry", delay));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_err.into())
}

pub(crate) async fn download_single_asset(
    client: &Client,
    url: &str,
    base_dir: &PathBuf,
    relative_path: &str,
    log: &DownloadLog,
    asset_type: &str,
) -> Result<DownloadedAsset, AppError> {
    let encoded_url = encode_url(url);
    if encoded_url != url {
        log.log(&format!("  URL_ENCODED: {} → {}", url, encoded_url));
    }

    let response = do_request(client, &encoded_url, url, log).await?;

    let status = response.status();
    let status_code = status.as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(none)")
        .to_string();
    let content_length = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(none)")
        .to_string();
    let location = response
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("(none)")
        .to_string();
    let final_url = response.url().to_string();
    log.log(&format!(
        "  HTTP {} content-type={} content-length={} final_url={} location={} url={}",
        status_code, content_type, content_length, final_url, location, url
    ));

    if !status.is_success() {
        if status.is_redirection() {
            return Err(format!("HTTP {} redirect to: {} (reqwest did not follow)", status_code, location).into());
        }
        return Err(format!("HTTP {}", status_code).into());
    }

    // Content-type validation: reject HTML error pages for media assets
    if content_type.contains("text/html") {
        let media_types = ["sprite", "background", "evidence", "music", "sound", "voice", "popup", "lock", "icon", "place"];
        if media_types.iter().any(|t| asset_type.contains(t)) {
            log.log(&format!("  CONTENT_TYPE_MISMATCH: expected media, got text/html for {}", url));
            return Err(format!("Received HTML instead of {} asset (likely a CDN error page)", asset_type).into());
        }
    }

    // Capture content-length for verification after download
    let expected_len = response.content_length();

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read body: {}", e))?;

    if bytes.is_empty() {
        log.log(&format!("  EMPTY response body for {}", url));
        return Err("Empty response body".to_string().into());
    }

    // Content-Length verification: detect truncated downloads
    if let Some(expected) = expected_len {
        if bytes.len() as u64 != expected {
            log.log(&format!(
                "  TRUNCATED: expected {} bytes, got {} for {}",
                expected, bytes.len(), url
            ));
            return Err(format!("Truncated download: expected {} bytes, got {}", expected, bytes.len()).into());
        }
    }

    let content_hash = xxhash_rust::xxh3::xxh3_64(&bytes);

    // Reject imgur's "removed" placeholder (503 bytes, served with HTTP 200 for dead image URLs).
    const IMGUR_REMOVED_HASH: u64 = 0x38da9bd2e10a4bc8;
    if content_hash == IMGUR_REMOVED_HASH {
        log.log(&format!("  IMGUR_REMOVED detected for {}", url));
        return Err("Image has been removed from imgur".to_string().into());
    }

    let file_path = base_dir.join(relative_path);
    log.log(&format!(
        "  SAVING {} bytes → {} (base={}, rel={})",
        bytes.len(),
        file_path.display(),
        base_dir.display(),
        relative_path
    ));

    // Create parent directories
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| {
                log.log(&format!("  MKDIR_ERR {} err={}", parent.display(), e));
                format!("Failed to create directory: {}", e)
            })?;
    }

    tokio::fs::write(&file_path, &bytes)
        .await
        .map_err(|e| {
            log.log(&format!("  WRITE_ERR {} err={}", file_path.display(), e));
            format!("Failed to write file: {}", e)
        })?;

    // Verify the file was actually written
    let verify_exists = file_path.exists();
    let verify_size = if verify_exists {
        std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    log.log(&format!(
        "  VERIFIED exists={} size_on_disk={} path={}",
        verify_exists, verify_size, file_path.display()
    ));

    Ok(DownloadedAsset {
        original_url: url.to_string(),
        local_path: relative_path.to_string(),
        size: bytes.len() as u64,
        content_hash,
    })
}

/// Make an HTTP GET request with smart protocol handling:
/// - External http:// URLs → try HTTPS first (many sites dropped HTTP/port 80),
///   fall back to HTTP only if HTTPS fails.
/// - Handles redirect errors (malformed Location headers) by retrying with HTTPS.
/// - Manually follows 3xx redirects with unencoded spaces in Location header.
pub(crate) async fn do_request(
    client: &Client,
    request_url: &str,
    original_url: &str,
    log: &DownloadLog,
) -> Result<reqwest::Response, AppError> {
    // For external http:// URLs, try HTTPS first to avoid 30s timeout on dead port 80.
    // AAO URLs are already https://, so this only affects third-party hosts.
    if request_url.starts_with("http://") {
        let https_url = encode_url(&request_url.replacen("http://", "https://", 1));
        log.log(&format!(
            "  HTTPS_FIRST: trying https before http: {} → {}",
            request_url, https_url
        ));

        match client
            .get(&https_url)
            .header("User-Agent", "AAO-Offline-Player/0.1")
            .timeout(PER_ASSET_TIMEOUT)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                log.log(&format!("  HTTPS_FIRST: success ({})", resp.status()));
                return Ok(resp);
            }
            Ok(resp) if resp.status().is_redirection() => {
                // Got a redirect over HTTPS — try to follow it manually
                if let Some(location) = resp.headers().get("location") {
                    let loc_str = location.to_str().unwrap_or("").to_string();
                    if !loc_str.is_empty() {
                        let encoded_loc = encode_url(&loc_str);
                        log.log(&format!(
                            "  MANUAL_REDIRECT (from HTTPS): {} → {}",
                            https_url, encoded_loc
                        ));
                        return Ok(client
                            .get(&encoded_loc)
                            .header("User-Agent", "AAO-Offline-Player/0.1")
                            .send()
                            .await
                            .map_err(|e| format!("Failed to follow redirect to {}: {}", encoded_loc, e))?);
                    }
                }
                // Redirect but no Location — return as-is, caller handles error
                return Ok(resp);
            }
            Ok(resp) => {
                log.log(&format!(
                    "  HTTPS_FIRST: got {} — falling back to original http",
                    resp.status()
                ));
                // Fall through to try original HTTP URL
            }
            Err(e) => {
                log.log(&format!(
                    "  HTTPS_FIRST: failed ({}) — falling back to original http",
                    e
                ));
                // Fall through to try original HTTP URL
            }
        }
    }

    // Standard request (or HTTP fallback after HTTPS failure)
    let response = client
        .get(request_url)
        .header("User-Agent", "AAO-Offline-Player/0.1")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!("timeout: {}", e)
            } else if e.is_connect() {
                format!("connection error: {}", e)
            } else if e.is_request() {
                format!("request error: {}", e)
            } else if e.is_redirect() {
                format!("redirect error: {}", e)
            } else {
                format!("HTTP error: {}", e)
            }
        })?;

    // Follow redirect chain manually (up to 3 levels).
    // Handles unencoded spaces in Location headers that reqwest can't parse.
    let mut current_response = response;
    for redirect_level in 0..3 {
        if !current_response.status().is_redirection() {
            return Ok(current_response);
        }
        let location = current_response.headers().get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if location.is_empty() {
            return Ok(current_response);
        }
        // Resolve the redirect URL — handle both absolute and relative Location headers
        let resolved_url = if location.starts_with("http://") || location.starts_with("https://") {
            encode_url(&location)
        } else {
            // Relative redirect — resolve against the current request URL
            match url::Url::parse(request_url) {
                Ok(base) => match base.join(&location) {
                    Ok(u) => u.to_string(),
                    Err(_) => encode_url(&location),
                },
                Err(_) => encode_url(&location),
            }
        };
        log.log(&format!(
            "  MANUAL_REDIRECT [{}]: {} → {}",
            redirect_level + 1, original_url, resolved_url
        ));
        current_response = client
            .get(&resolved_url)
            .header("User-Agent", "AAO-Offline-Player/0.1")
            .timeout(PER_ASSET_TIMEOUT)
            .send()
            .await
            .map_err(|e| format!("Failed to follow redirect to {}: {}", resolved_url, e))?;
    }

    Ok(current_response)
}
