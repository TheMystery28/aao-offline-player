//! Logic for creating and reading case manifests (`manifest.json`).
//!
//! The manifest is the single source of truth for a downloaded case,
//! containing metadata, asset mappings, and failure logs.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use super::CaseInfo;
use super::asset_downloader::DownloadedAsset;

/// An asset that failed to download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedAsset {
    pub url: String,
    pub asset_type: String,
    /// What the local path would have been (empty for external assets).
    pub local_path: String,
    pub error: String,
}

/// Case metadata and asset mappings, persisted as `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseManifest {
    pub case_id: u32,
    pub title: String,
    pub author: String,
    pub language: String,
    pub download_date: String,
    pub format: String,
    pub sequence: Option<serde_json::Value>,
    pub assets: AssetSummary,
    /// Maps original asset URL to local relative path (e.g. "assets/filename-hash.ext").
    pub asset_map: HashMap<String, String>,
    /// Assets that failed to download (empty if all succeeded).
    #[serde(default)]
    pub failed_assets: Vec<FailedAsset>,
    /// Whether the case has bundled plugins in case/{id}/plugins/
    #[serde(default)]
    pub has_plugins: bool,
    /// Whether the case has a case_config.json for plugin configuration
    #[serde(default)]
    pub has_case_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSummary {
    pub case_specific: usize,
    pub shared_defaults: usize,
    pub total_downloaded: usize,
    pub total_size_bytes: u64,
}

/// Build a manifest from case info and downloaded assets.
pub fn build_manifest(
    case_info: &CaseInfo,
    downloaded: &[DownloadedAsset],
    failed: Vec<FailedAsset>,
    case_specific_count: usize,
    shared_count: usize,
) -> CaseManifest {
    let total_bytes: u64 = downloaded.iter().map(|a| a.size).sum();

    let mut asset_map = HashMap::new();
    for asset in downloaded {
        asset_map.insert(asset.original_url.clone(), asset.local_path.clone());
    }

    // ISO 8601 timestamp without chrono dependency
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let download_date = format_timestamp(now);

    CaseManifest {
        case_id: case_info.id,
        title: case_info.title.clone(),
        author: case_info.author.clone(),
        language: case_info.language.clone(),
        download_date,
        format: case_info.format.clone(),
        sequence: case_info.sequence.clone(),
        assets: AssetSummary {
            case_specific: case_specific_count,
            shared_defaults: shared_count,
            total_downloaded: downloaded.len(),
            total_size_bytes: total_bytes,
        },
        asset_map,
        failed_assets: failed,
        has_plugins: false,
        has_case_config: false,
    }
}

/// Persist a manifest to disk as pretty-printed JSON.
///
/// # Errors
///
/// Returns an `AppError` if serialization or file writing fails.
pub fn write_manifest(manifest: &CaseManifest, case_dir: &Path) -> Result<(), AppError> {
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    std::fs::write(case_dir.join("manifest.json"), json)
        .map_err(|e| format!("Failed to write manifest.json: {}", e))?;
    Ok(())
}

/// Rewrite asset paths in trial_data.json using the manifest as single source of truth.
///
/// For each entry in `asset_map`, computes the server-resolvable path and replaces
/// occurrences in the trial_data JSON tree. This ensures trial_data always agrees
/// with the manifest — no separate rewrite maps needed.
pub fn rewrite_trial_data_from_manifest(
    trial_data: &mut serde_json::Value,
    case_id: u32,
    manifest: &CaseManifest,
) {
    use crate::downloader::dedup::rewrite_value_recursive;

    for (key, local_path) in &manifest.asset_map {
        // Skip defaults/ entries that map to themselves (default sprites, voices, places)
        if key == local_path && local_path.starts_with("defaults/") {
            continue;
        }

        let server_path = if local_path.starts_with("defaults/") {
            local_path.clone()
        } else if local_path.starts_with("assets/") {
            super::asset_paths::case_relative(case_id, local_path)
        } else {
            local_path.clone()
        };

        // Replace the key (original URL or "assets/filename") with the server path
        rewrite_value_recursive(trial_data, key, &server_path);
    }
}

/// Load a manifest from the given case directory.
///
/// # Errors
///
/// Returns an `AppError` if the file is missing or contains invalid JSON.
pub fn read_manifest(case_dir: &Path) -> Result<CaseManifest, AppError> {
    let data = std::fs::read_to_string(case_dir.join("manifest.json"))
        .map_err(|e| format!("Failed to read manifest.json: {}", e))?;
    Ok(serde_json::from_str(&data)
        .map_err(|e| format!("Failed to parse manifest.json: {}", e))?)
}

use crate::utils::format_timestamp;

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::CaseInfo;

    fn test_case_info() -> CaseInfo {
        CaseInfo {
            id: 69063,
            title: "Test Case".to_string(),
            author: "TestAuthor".to_string(),
            language: "en".to_string(),
            last_edit_date: 0,
            format: "v6".to_string(),
            sequence: None,
        }
    }

    #[test]
    fn test_build_manifest_basic() {
        let info = test_case_info();
        let downloaded = vec![
            DownloadedAsset { original_url: "http://a.com/1.png".into(), local_path: "assets/img-abc.png".into(), size: 1000, content_hash: 0 },
            DownloadedAsset { original_url: "http://b.com/2.mp3".into(), local_path: "defaults/music/song.mp3".into(), size: 2000, content_hash: 0 },
        ];
        let manifest = build_manifest(&info, &downloaded, Vec::new(), 5, 3);

        assert_eq!(manifest.case_id, 69063);
        assert_eq!(manifest.title, "Test Case");
        assert_eq!(manifest.author, "TestAuthor");
        assert_eq!(manifest.assets.case_specific, 5);
        assert_eq!(manifest.assets.shared_defaults, 3);
        assert_eq!(manifest.assets.total_downloaded, 2);
        assert_eq!(manifest.assets.total_size_bytes, 3000);
        assert_eq!(manifest.asset_map.len(), 2);
        assert_eq!(manifest.asset_map["http://a.com/1.png"], "assets/img-abc.png");
        assert!(manifest.failed_assets.is_empty());
    }

    #[test]
    fn test_manifest_write_read_roundtrip() {
        let info = test_case_info();
        let downloaded = vec![
            DownloadedAsset { original_url: "http://x.com/bg.jpg".into(), local_path: "assets/bg-hash.jpg".into(), size: 500, content_hash: 0 },
        ];
        let failed = vec![FailedAsset {
            url: "http://dead.com/sound.mp3".into(),
            asset_type: "sound".into(),
            local_path: "assets/sound-hash.mp3".into(),
            error: "timeout".into(),
        }];
        let manifest = build_manifest(&info, &downloaded, failed, 1, 0);

        let dir = tempfile::tempdir().unwrap();
        write_manifest(&manifest, dir.path()).unwrap();
        let loaded = read_manifest(dir.path()).unwrap();

        assert_eq!(loaded.case_id, manifest.case_id);
        assert_eq!(loaded.title, manifest.title);
        assert_eq!(loaded.assets.total_size_bytes, manifest.assets.total_size_bytes);
        assert_eq!(loaded.asset_map, manifest.asset_map);
        assert_eq!(loaded.failed_assets.len(), 1);
        assert_eq!(loaded.failed_assets[0].url, "http://dead.com/sound.mp3");
    }

    #[test]
    fn test_format_timestamp_unix_epoch() {
        assert_eq!(format_timestamp(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_format_timestamp_known_date() {
        // 2025-01-01T00:00:00Z = 1735689600
        assert_eq!(format_timestamp(1735689600), "2025-01-01T00:00:00Z");
    }

    #[test]
    fn test_format_timestamp_iso8601_format() {
        let ts = format_timestamp(1000000000);
        // Must match YYYY-MM-DDTHH:MM:SSZ pattern
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
    }

    // --- New tests ---

    /// Manifest with sequence: None roundtrips through write/read.
    #[test]
    fn test_manifest_without_sequence() {
        let info = test_case_info();
        let manifest = build_manifest(&info, &[], Vec::new(), 0, 0);
        assert!(manifest.sequence.is_none());

        let dir = tempfile::tempdir().unwrap();
        write_manifest(&manifest, dir.path()).unwrap();
        let loaded = read_manifest(dir.path()).unwrap();

        assert!(loaded.sequence.is_none());
        assert_eq!(loaded.case_id, manifest.case_id);
        assert_eq!(loaded.title, manifest.title);
    }

    /// Manifest with failed_assets roundtrips correctly.
    #[test]
    fn test_manifest_with_failed_assets_roundtrip() {
        let info = test_case_info();
        let downloaded = vec![
            DownloadedAsset { original_url: "http://ok.com/1.png".into(), local_path: "assets/1-hash.png".into(), size: 100, content_hash: 0 },
        ];
        let failed = vec![
            FailedAsset { url: "http://fail.com/a.mp3".into(), asset_type: "music".into(), local_path: "assets/a-hash.mp3".into(), error: "HTTP 404".into() },
            FailedAsset { url: "http://fail.com/b.gif".into(), asset_type: "sprite".into(), local_path: "assets/b-hash.gif".into(), error: "timeout".into() },
        ];
        let manifest = build_manifest(&info, &downloaded, failed, 1, 0);

        let dir = tempfile::tempdir().unwrap();
        write_manifest(&manifest, dir.path()).unwrap();
        let loaded = read_manifest(dir.path()).unwrap();

        assert_eq!(loaded.failed_assets.len(), 2);
        assert_eq!(loaded.failed_assets[0].url, "http://fail.com/a.mp3");
        assert_eq!(loaded.failed_assets[0].error, "HTTP 404");
        assert_eq!(loaded.failed_assets[1].url, "http://fail.com/b.gif");
        assert_eq!(loaded.failed_assets[1].asset_type, "sprite");
    }

    /// Manifest with many entries in asset_map roundtrips correctly.
    #[test]
    fn test_manifest_large_asset_map() {
        let info = test_case_info();
        let mut downloaded: Vec<DownloadedAsset> = Vec::new();
        for i in 0..100 {
            downloaded.push(DownloadedAsset {
                original_url: format!("http://example.com/asset_{}.png", i),
                local_path: format!("assets/asset_{}-hash.png", i),
                size: i as u64 * 10,
                content_hash: 0,
            });
        }
        let manifest = build_manifest(&info, &downloaded, Vec::new(), 100, 0);
        assert_eq!(manifest.asset_map.len(), 100);

        let dir = tempfile::tempdir().unwrap();
        write_manifest(&manifest, dir.path()).unwrap();
        let loaded = read_manifest(dir.path()).unwrap();

        assert_eq!(loaded.asset_map.len(), 100);
        assert_eq!(loaded.assets.total_downloaded, 100);
        // Verify a specific entry roundtripped
        assert_eq!(
            loaded.asset_map["http://example.com/asset_42.png"],
            "assets/asset_42-hash.png"
        );
    }

    /// Manifest with empty asset_map works correctly.
    #[test]
    fn test_manifest_empty_asset_map() {
        let info = test_case_info();
        let manifest = build_manifest(&info, &[], Vec::new(), 0, 0);
        assert!(manifest.asset_map.is_empty());
        assert_eq!(manifest.assets.total_downloaded, 0);
        assert_eq!(manifest.assets.total_size_bytes, 0);

        let dir = tempfile::tempdir().unwrap();
        write_manifest(&manifest, dir.path()).unwrap();
        let loaded = read_manifest(dir.path()).unwrap();

        assert!(loaded.asset_map.is_empty());
        assert_eq!(loaded.assets.total_downloaded, 0);
        assert_eq!(loaded.assets.total_size_bytes, 0);
    }
}
