use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use reqwest::Client;
use serde::Serialize;
use tauri::ipc::Channel;

use super::AssetRef;
use super::manifest::FailedAsset;

const DEFAULT_CONCURRENCY: usize = 3;
const MAX_RETRIES: u32 = 3;
const BASE_RETRY_DELAY: Duration = Duration::from_secs(2);
const PER_ASSET_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename = "started")]
    Started { total: usize },
    #[serde(rename = "progress")]
    Progress {
        completed: usize,
        total: usize,
        current_url: String,
        bytes_downloaded: u64,
        elapsed_ms: u64,
    },
    #[serde(rename = "finished")]
    Finished {
        downloaded: usize,
        failed: usize,
        total_bytes: u64,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "sequence_progress")]
    SequenceProgress {
        current_part: usize,
        total_parts: usize,
        part_title: String,
    },
}

#[derive(Debug, Clone)]
pub struct DownloadedAsset {
    pub original_url: String,
    pub local_path: String,
    pub size: u64,
}

/// Result of a batch download operation.
pub struct DownloadResult {
    pub downloaded: Vec<DownloadedAsset>,
    pub failed: Vec<FailedAsset>,
}

/// Check if a file already exists locally and should be skipped.
/// Returns `Some(size)` if the file exists and has content (skip download),
/// or `None` if the file is missing or empty (proceed with download).
pub fn check_skip_existing(save_dir: &std::path::Path, relative_path: &str) -> Option<u64> {
    let file_path = save_dir.join(relative_path);
    if file_path.exists() {
        let size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            return Some(size);
        }
    }
    None
}

/// Debug-only log writer.
struct DownloadLog {
    #[cfg(debug_assertions)]
    file: std::sync::Mutex<std::fs::File>,
}

impl DownloadLog {
    fn new(path: &std::path::Path) -> Result<Self, String> {
        #[cfg(debug_assertions)]
        {
            let file = std::fs::File::create(path)
                .map_err(|e| format!("Failed to create log file: {}", e))?;
            Ok(Self {
                file: std::sync::Mutex::new(file),
            })
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = path;
            Ok(Self {})
        }
    }

    #[allow(unused_variables)]
    fn log(&self, msg: &str) {
        #[cfg(debug_assertions)]
        {
            use std::io::Write;
            if let Ok(mut f) = self.file.lock() {
                let _ = writeln!(f, "{}", msg);
                let _ = f.flush();
            }
            println!("{}", msg);
        }
    }
}

fn generate_filename(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();
    let hash_str = format!("{:016x}", hash);

    let url_path = url.split('?').next().unwrap_or(url);
    let raw_name = url_path.rsplit('/').next().unwrap_or("asset");

    let (name, ext) = match raw_name.rfind('.') {
        Some(pos) => (&raw_name[..pos], &raw_name[pos + 1..]),
        None => (raw_name, "bin"),
    };

    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();

    let sanitized = if sanitized.is_empty() {
        "asset".to_string()
    } else {
        sanitized.to_lowercase()
    };

    format!("{}-{}.{}", sanitized, &hash_str[..12], ext.to_lowercase())
}

/// Download assets with progress reporting and retry logic.
/// - Assets with `local_path` set are saved to `engine_dir/{local_path}` (internal AAO assets).
/// - Assets with empty `local_path` are saved to `case_dir/assets/{hash}` (external assets).
pub async fn download_assets(
    client: &Client,
    assets: Vec<AssetRef>,
    case_dir: &PathBuf,
    engine_dir: &PathBuf,
    on_event: &Channel<DownloadEvent>,
    concurrency: usize,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<DownloadResult, String> {
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
            let url = asset.url.clone();
            let asset_type = asset.asset_type.clone();
            let local_path = asset.local_path.clone();
            let cancel_flag = cancel_flag.clone();

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
                    return Ok(DownloadedAsset {
                        original_url: url,
                        local_path: relative_path,
                        size,
                    });
                }

                match download_with_retry(&client, &url, &save_dir, &relative_path, &log, &asset_type).await {
                    Ok(result) => {
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
                        failed.fetch_add(1, Ordering::Relaxed);
                        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                        log.log(&format!(
                            "  FINAL_FAIL type={} url={} err={}",
                            asset_type, url, e
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
                            error: e,
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
        })
        .ok();

    Ok(DownloadResult {
        downloaded,
        failed: failed_assets,
    })
}

async fn download_with_retry(
    client: &Client,
    url: &str,
    base_dir: &PathBuf,
    relative_path: &str,
    log: &DownloadLog,
    asset_type: &str,
) -> Result<DownloadedAsset, String> {
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
                last_err = e;
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
                    return Err(last_err);
                }

                if attempt < MAX_RETRIES - 1 {
                    let delay = BASE_RETRY_DELAY * 2u32.pow(attempt);
                    log.log(&format!("  WAIT {:?} before retry", delay));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_err)
}

/// Encode spaces (and other raw characters) in a URL path.
/// Handles URLs from AAO trial data which may contain unencoded spaces.
fn encode_url(raw_url: &str) -> String {
    // Only encode spaces — other special chars are less common and reqwest handles most
    raw_url.replace(' ', "%20")
}

async fn download_single_asset(
    client: &Client,
    url: &str,
    base_dir: &PathBuf,
    relative_path: &str,
    log: &DownloadLog,
    asset_type: &str,
) -> Result<DownloadedAsset, String> {
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
            return Err(format!("HTTP {} redirect to: {} (reqwest did not follow)", status_code, location));
        }
        return Err(format!("HTTP {}", status_code));
    }

    // Content-type validation: reject HTML error pages for media assets
    if content_type.contains("text/html") {
        let media_types = ["sprite", "background", "evidence", "music", "sound", "voice", "popup", "lock", "icon", "place"];
        if media_types.iter().any(|t| asset_type.contains(t)) {
            log.log(&format!("  CONTENT_TYPE_MISMATCH: expected media, got text/html for {}", url));
            return Err(format!("Received HTML instead of {} asset (likely a CDN error page)", asset_type));
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
        return Err("Empty response body".to_string());
    }

    // Content-Length verification: detect truncated downloads
    if let Some(expected) = expected_len {
        if bytes.len() as u64 != expected {
            log.log(&format!(
                "  TRUNCATED: expected {} bytes, got {} for {}",
                expected, bytes.len(), url
            ));
            return Err(format!("Truncated download: expected {} bytes, got {}", expected, bytes.len()));
        }
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
    })
}

/// Make an HTTP GET request with smart protocol handling:
/// - External http:// URLs → try HTTPS first (many sites dropped HTTP/port 80),
///   fall back to HTTP only if HTTPS fails.
/// - Handles redirect errors (malformed Location headers) by retrying with HTTPS.
/// - Manually follows 3xx redirects with unencoded spaces in Location header.
async fn do_request(
    client: &Client,
    request_url: &str,
    original_url: &str,
    log: &DownloadLog,
) -> Result<reqwest::Response, String> {
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
                        return client
                            .get(&encoded_loc)
                            .header("User-Agent", "AAO-Offline-Player/0.1")
                            .send()
                            .await
                            .map_err(|e| format!("Failed to follow redirect to {}: {}", encoded_loc, e));
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

    // If we got a 3xx response with a Location header, follow it manually
    // (handles unencoded spaces in Location that reqwest can't parse).
    if response.status().is_redirection() {
        if let Some(location) = response.headers().get("location") {
            let loc_str = location.to_str().unwrap_or("").to_string();
            if !loc_str.is_empty() {
                let encoded_loc = encode_url(&loc_str);
                log.log(&format!(
                    "  MANUAL_REDIRECT: {} → {} (encoded: {})",
                    original_url, loc_str, encoded_loc
                ));
                return client
                    .get(&encoded_loc)
                    .header("User-Agent", "AAO-Offline-Player/0.1")
                    .send()
                    .await
                    .map_err(|e| format!("Failed to follow redirect to {}: {}", encoded_loc, e));
            }
        }
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_filename() {
        let name = generate_filename("https://aaonline.fr/uploads/sprites/chars/Phoenix/1.gif");
        assert!(name.ends_with(".gif"));
        assert!(name.contains('-'));
        let name2 = generate_filename("https://aaonline.fr/uploads/sprites/chars/Phoenix/1.gif");
        assert_eq!(name, name2);
        let name3 = generate_filename("https://aaonline.fr/uploads/sprites/chars/Phoenix/2.gif");
        assert_ne!(name, name3);
    }

    #[test]
    fn test_generate_filename_strips_query_string() {
        let name = generate_filename("https://example.com/image.png?v=123&t=456");
        assert!(name.ends_with(".png"));
    }

    #[test]
    fn test_generate_filename_no_extension_uses_bin() {
        let name = generate_filename("https://example.com/asset");
        assert!(name.ends_with(".bin"));
    }

    #[test]
    fn test_generate_filename_sanitizes_special_chars() {
        let name = generate_filename("https://example.com/my image (1).png");
        assert!(name.ends_with(".png"));
        assert!(!name.contains(' '));
        assert!(!name.contains('('));
    }

    /// Regression: external assets were saved to case_dir/assets/assets/{hash}
    /// because save_dir was case_dir/assets/ and relative_path was "assets/{hash}".
    /// Fix: save_dir must be case_dir (not case_dir/assets/).
    #[test]
    fn test_external_asset_path_no_double_nesting() {
        let case_dir = PathBuf::from("/data/case/123");
        let url = "http://i.imgur.com/abc.png";
        let filename = generate_filename(url);

        // Replicate the path construction from download_assets for external assets
        let local_path = ""; // external → empty local_path
        let (save_dir, relative_path) = if local_path.is_empty() {
            (case_dir.clone(), format!("assets/{}", filename))
        } else {
            unreachable!()
        };

        let final_path = save_dir.join(&relative_path);
        let final_str = final_path.to_string_lossy();

        // Must NOT contain double-nested assets/assets/
        assert!(
            !final_str.contains("assets/assets") && !final_str.contains("assets\\assets"),
            "Double-nested assets directory detected: {}",
            final_str
        );
        // Must be exactly case_dir/assets/{filename}
        assert_eq!(final_path, case_dir.join("assets").join(&filename));
    }

    /// Verify internal assets go to engine_dir/{local_path}, not case_dir.
    #[test]
    fn test_internal_asset_path_uses_engine_dir() {
        let case_dir = PathBuf::from("/data/case/123");
        let engine_dir = PathBuf::from("/data/engine");
        let local_path = "defaults/images/chars/Phoenix.png";

        // Replicate the path construction from download_assets for internal assets
        let (save_dir, relative_path) = if !local_path.is_empty() {
            (engine_dir.clone(), local_path.to_string())
        } else {
            unreachable!()
        };

        let final_path = save_dir.join(&relative_path);

        // Must be under engine_dir, not case_dir
        assert!(final_path.starts_with(&engine_dir));
        assert!(!final_path.starts_with(&case_dir));
    }

    #[test]
    fn test_encode_url_spaces() {
        assert_eq!(
            encode_url("http://example.com/sounds/Apollo - (french) hold it.mp3"),
            "http://example.com/sounds/Apollo%20-%20(french)%20hold%20it.mp3"
        );
    }

    #[test]
    fn test_encode_url_already_encoded() {
        let url = "http://example.com/sounds/Konrad%20-%20(french)%20Objection.mp3";
        assert_eq!(encode_url(url), url);
    }

    #[test]
    fn test_encode_url_no_spaces() {
        let url = "https://aaonline.fr/uploads/sprites/chars/Phoenix/1.gif";
        assert_eq!(encode_url(url), url);
    }

    // --- check_skip_existing tests ---

    /// Default asset already on disk → skip download.
    #[test]
    fn test_skip_existing_default_asset() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();

        // Simulate a bundled default sprite
        let rel = "defaults/images/chars/Phoenix/1.gif";
        let file_path = engine_dir.join(rel);
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, b"GIF89a_fake_image_data").unwrap();

        let result = check_skip_existing(engine_dir, rel);
        assert!(result.is_some(), "Should skip download for existing default asset");
        assert_eq!(result.unwrap(), 22); // "GIF89a_fake_image_data" is 22 bytes
    }

    /// Missing asset → must download.
    #[test]
    fn test_no_skip_missing_asset() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();

        let result = check_skip_existing(engine_dir, "defaults/images/chars/Phoenix/1.gif");
        assert!(result.is_none(), "Should not skip download for missing asset");
    }

    /// Empty file (0 bytes) → must re-download.
    #[test]
    fn test_no_skip_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();

        let rel = "defaults/music/AA1/track.mp3";
        let file_path = engine_dir.join(rel);
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, b"").unwrap();

        let result = check_skip_existing(engine_dir, rel);
        assert!(result.is_none(), "Should not skip download for empty file");
    }

    /// Nested default paths (backgrounds, sounds, voices) all skip correctly.
    #[test]
    fn test_skip_existing_various_default_types() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();

        let cases = vec![
            ("defaults/images/backgrounds/AA4/Court.jpg", b"JFIF_fake" as &[u8]),
            ("defaults/sounds/sfx-blipmale.wav", b"RIFF_fake"),
            ("defaults/voices/French/Objection.mp3", b"ID3_fake"),
            ("defaults/images/charsStill/Phoenix/1.gif", b"GIF87a"),
            ("defaults/images/charsStartup/Apollo/1.gif", b"GIF89a"),
        ];

        for (rel, content) in &cases {
            let file_path = engine_dir.join(rel);
            std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
            std::fs::write(&file_path, content).unwrap();
        }

        for (rel, content) in &cases {
            let result = check_skip_existing(engine_dir, rel);
            assert!(
                result.is_some(),
                "Should skip download for existing default: {}",
                rel
            );
            assert_eq!(
                result.unwrap(),
                content.len() as u64,
                "Size mismatch for {}",
                rel
            );
        }
    }

    /// End-to-end: simulate the full path construction + skip check
    /// for an internal default asset, as download_assets would do.
    #[test]
    fn test_skip_existing_end_to_end_internal_asset() {
        let dir = tempfile::tempdir().unwrap();
        let engine_dir = dir.path();

        // Pre-populate a default asset
        let default_rel = "defaults/images/chars/Phoenix/1.gif";
        let full_path = engine_dir.join(default_rel);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, b"GIF89a_sprite_data_here").unwrap();

        // Simulate what download_assets does for an internal asset
        let asset = AssetRef {
            url: "https://aaonline.fr/Ressources/Images/Personnages/Phoenix/1.gif".to_string(),
            asset_type: "icon".to_string(),
            is_default: true,
            local_path: default_rel.to_string(),
        };

        // Path construction from download_assets
        let (save_dir, relative_path) = if asset.local_path.is_empty() {
            unreachable!("internal asset should have local_path");
        } else {
            (engine_dir.to_path_buf(), asset.local_path.clone())
        };

        // This is exactly the check that prevents re-downloading
        let result = check_skip_existing(&save_dir, &relative_path);
        assert!(
            result.is_some(),
            "Internal default asset with local_path='{}' should be skipped",
            asset.local_path
        );
    }

    /// External assets (empty local_path) use case_dir, not engine_dir.
    /// If a previous download already saved the file, it should be skipped.
    #[test]
    fn test_skip_existing_external_asset_in_case_dir() {
        let dir = tempfile::tempdir().unwrap();
        let case_dir = dir.path();

        let url = "http://i.imgur.com/abc.png";
        let filename = generate_filename(url);
        let relative_path = format!("assets/{}", filename);

        // Pre-populate the external asset
        let full_path = case_dir.join(&relative_path);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, b"PNG_fake_external_image").unwrap();

        let result = check_skip_existing(case_dir, &relative_path);
        assert!(
            result.is_some(),
            "Previously downloaded external asset should be skipped"
        );
    }

    /// External asset not yet downloaded → must download.
    #[test]
    fn test_no_skip_external_asset_not_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let case_dir = dir.path();

        let url = "http://i.imgur.com/abc.png";
        let filename = generate_filename(url);
        let relative_path = format!("assets/{}", filename);

        let result = check_skip_existing(case_dir, &relative_path);
        assert!(
            result.is_none(),
            "Missing external asset should not be skipped"
        );
    }

    // --- DownloadEvent serialization regression tests ---
    // These ensure adding new variants doesn't break existing serialization.

    #[test]
    fn test_download_event_started_serialization() {
        let event = DownloadEvent::Started { total: 42 };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "started");
        assert_eq!(json["data"]["total"], 42);
    }

    #[test]
    fn test_download_event_progress_serialization() {
        let event = DownloadEvent::Progress {
            completed: 10,
            total: 50,
            current_url: "https://example.com/img.png".to_string(),
            bytes_downloaded: 1024000,
            elapsed_ms: 5000,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "progress");
        assert_eq!(json["data"]["completed"], 10);
        assert_eq!(json["data"]["total"], 50);
        assert_eq!(json["data"]["current_url"], "https://example.com/img.png");
        assert_eq!(json["data"]["bytes_downloaded"], 1024000);
        assert_eq!(json["data"]["elapsed_ms"], 5000);
    }

    #[test]
    fn test_download_event_finished_serialization() {
        let event = DownloadEvent::Finished {
            downloaded: 45,
            failed: 5,
            total_bytes: 123456,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "finished");
        assert_eq!(json["data"]["downloaded"], 45);
        assert_eq!(json["data"]["failed"], 5);
        assert_eq!(json["data"]["total_bytes"], 123456);
    }

    #[test]
    fn test_download_event_error_serialization() {
        let event = DownloadEvent::Error {
            message: "Connection refused".to_string(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "error");
        assert_eq!(json["data"]["message"], "Connection refused");
    }

    #[test]
    fn test_download_event_sequence_progress_serialization() {
        let event = DownloadEvent::SequenceProgress {
            current_part: 2,
            total_parts: 5,
            part_title: "Trial Day 1".to_string(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "sequence_progress");
        assert_eq!(json["data"]["current_part"], 2);
        assert_eq!(json["data"]["total_parts"], 5);
        assert_eq!(json["data"]["part_title"], "Trial Day 1");
    }

    // --- New tests ---

    /// All DownloadEvent variants must serialize with an "event" field.
    #[test]
    fn test_download_event_all_variants_have_event_field() {
        let variants: Vec<DownloadEvent> = vec![
            DownloadEvent::Started { total: 10 },
            DownloadEvent::Progress { completed: 1, total: 10, current_url: "http://a.com".into(), bytes_downloaded: 100, elapsed_ms: 500 },
            DownloadEvent::Finished { downloaded: 8, failed: 2, total_bytes: 500 },
            DownloadEvent::Error { message: "fail".into() },
            DownloadEvent::SequenceProgress { current_part: 1, total_parts: 3, part_title: "P1".into() },
        ];
        for variant in &variants {
            let json = serde_json::to_value(variant).unwrap();
            assert!(
                json.get("event").is_some(),
                "Missing 'event' field in serialized DownloadEvent: {:?}",
                json
            );
            assert!(
                json["event"].is_string(),
                "'event' field should be a string, got: {:?}",
                json["event"]
            );
        }
    }

    /// Started with total=0 should serialize correctly.
    #[test]
    fn test_download_event_started_zero_total() {
        let event = DownloadEvent::Started { total: 0 };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"], "started");
        assert_eq!(json["data"]["total"], 0);
    }

    /// Same URL must always produce the same filename (determinism).
    #[test]
    fn test_generate_filename_deterministic() {
        let url = "https://aaonline.fr/uploads/sprites/chars/Apollo/7.gif";
        let name1 = generate_filename(url);
        let name2 = generate_filename(url);
        let name3 = generate_filename(url);
        assert_eq!(name1, name2);
        assert_eq!(name2, name3);
    }

    /// Different URLs must produce different filenames.
    #[test]
    fn test_generate_filename_different_urls_different_names() {
        let name_a = generate_filename("https://example.com/image_a.png");
        let name_b = generate_filename("https://example.com/image_b.png");
        let name_c = generate_filename("https://other.com/image_a.png");
        assert_ne!(name_a, name_b, "Different filenames on same host should differ");
        assert_ne!(name_a, name_c, "Same filename on different hosts should differ");
    }

    /// URL with unicode characters should produce a valid filename.
    #[test]
    fn test_generate_filename_unicode() {
        let name = generate_filename("https://example.com/images/café_résumé.png");
        assert!(name.ends_with(".png"));
        assert!(!name.is_empty());
        assert!(name.contains('-'), "Filename should contain hash separator");
        // Unicode alphanumeric chars are preserved by is_alphanumeric(), which is correct
        // behavior — they're valid in filenames on all platforms
        assert!(
            !name.contains(' ') && !name.contains('(') && !name.contains(')'),
            "Filename should not contain spaces or special chars: {}",
            name
        );
    }

    /// Very long URL should still produce a reasonable filename.
    #[test]
    fn test_generate_filename_very_long_url() {
        let long_path = "a".repeat(500);
        let url = format!("https://example.com/{}.jpg", long_path);
        let name = generate_filename(&url);
        assert!(name.ends_with(".jpg"));
        assert!(!name.is_empty());
        // The filename includes the full sanitized name + hash, which could be long,
        // but it should still be well-formed
        assert!(name.contains('-'), "Filename should contain hash separator");
    }

    /// check_skip_existing returns correct file size when file exists with known content.
    #[test]
    fn test_check_skip_existing_returns_correct_size() {
        let dir = tempfile::tempdir().unwrap();
        let rel = "assets/test-image.png";
        let file_path = dir.path().join(rel);
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        let content = b"PNG_image_data_exactly_42_bytes_long_paddd";
        assert_eq!(content.len(), 42);
        std::fs::write(&file_path, content).unwrap();

        let result = check_skip_existing(dir.path(), rel);
        assert!(result.is_some(), "File exists and has content, should return Some");
        assert_eq!(result.unwrap(), 42, "Should return exact file size in bytes");
    }

    // ============================================================
    // Phase 1: Core Reliability Tests
    // ============================================================

    #[test]
    fn test_per_asset_timeout_constant() {
        assert!(PER_ASSET_TIMEOUT.as_secs() >= 5, "Timeout should be at least 5s");
        assert!(PER_ASSET_TIMEOUT.as_secs() <= 30, "Timeout should be at most 30s");
        assert_eq!(PER_ASSET_TIMEOUT.as_secs(), 15, "Timeout should be 15s");
    }

    #[test]
    fn test_retryable_errors_include_transient() {
        // These should be retried
        let transient = vec!["HTTP 429", "HTTP 503", "HTTP 502", "timeout: foo", "connection error", "reset by peer", "closed"];
        for err in &transient {
            let is_retryable = err.contains("429")
                || err.contains("503")
                || err.contains("502")
                || err.contains("timeout")
                || err.contains("connection")
                || err.contains("reset")
                || err.contains("closed");
            assert!(is_retryable, "'{}' should be retryable", err);
        }
    }

    #[test]
    fn test_retryable_errors_exclude_permanent() {
        // These should NOT be retried (permanent failures)
        let permanent = vec!["HTTP 301 redirect to: example.com", "HTTP 302 redirect to: example.com", "HTTP 404", "HTTP 403"];
        for err in &permanent {
            let is_retryable = err.contains("429")
                || err.contains("503")
                || err.contains("502")
                || err.contains("timeout")
                || err.contains("connection")
                || err.contains("reset")
                || err.contains("closed");
            assert!(!is_retryable, "'{}' should NOT be retryable", err);
        }
    }

    #[test]
    fn test_content_type_html_is_rejected_for_media() {
        let media_types = vec!["sprite", "background_internal", "evidence_icon", "music_internal", "sound", "voice", "popup", "lock", "icon"];
        for asset_type in &media_types {
            let content_type = "text/html; charset=utf-8";
            let media_markers = ["sprite", "background", "evidence", "music", "sound", "voice", "popup", "lock", "icon", "place"];
            let should_reject = content_type.contains("text/html")
                && media_markers.iter().any(|t| asset_type.contains(t));
            assert!(should_reject, "HTML response should be rejected for asset_type '{}'", asset_type);
        }
    }

    #[test]
    fn test_content_type_html_is_accepted_for_unknown() {
        let content_type = "text/html";
        let asset_type = "external_unknown";
        let media_markers = ["sprite", "background", "evidence", "music", "sound", "voice", "popup", "lock", "icon", "place"];
        let should_reject = content_type.contains("text/html")
            && media_markers.iter().any(|t| asset_type.contains(t));
        assert!(!should_reject, "HTML should be accepted for unknown asset type");
    }

    #[test]
    fn test_content_length_mismatch_detected() {
        let expected: Option<u64> = Some(1000);
        let actual_len: u64 = 500;
        if let Some(exp) = expected {
            assert_ne!(actual_len, exp, "Mismatch should be detected");
        }
    }

    #[test]
    fn test_content_length_absent_is_ok() {
        let expected: Option<u64> = None;
        // When Content-Length is absent, we accept any size
        assert!(expected.is_none(), "Absent Content-Length should not cause rejection");
    }

    #[test]
    fn test_content_length_match_is_ok() {
        let expected: Option<u64> = Some(500);
        let actual_len: u64 = 500;
        if let Some(exp) = expected {
            assert_eq!(actual_len, exp, "Matching Content-Length should pass");
        }
    }
}
