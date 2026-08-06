//! mtime-validated cache of case manifests for the library listing.
//!
//! `list_cases` previously parsed every `manifest.json` on every call.
//! This module persists a `library_cache.json` holding each manifest minus
//! `asset_map`, validated per case by the manifest file's (mtime, size)
//! fingerprint. Only drifted/new cases are re-parsed.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::downloader::manifest::{read_manifest, CaseManifest};
use crate::error::AppError;

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    /// (mtime_secs, mtime_nanos, file_size)
    fingerprint: (u64, u32, u64),
    manifest: CaseManifest,
}

#[derive(Debug, Serialize, Deserialize)]
struct LibraryCacheFile {
    version: u32,
    cases: HashMap<String, CacheEntry>,
}

fn cache_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("library_cache.json")
}

fn manifest_fingerprint(manifest_path: &Path) -> Option<(u64, u32, u64)> {
    let meta = fs::metadata(manifest_path).ok()?;
    let size = meta.len();
    let mtime = match meta.modified() {
        Ok(t) => t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default(),
        Err(_) => return None,
    };
    Some((mtime.as_secs(), mtime.subsec_nanos(), size))
}

/// List all downloaded cases, using the persistent cache for untouched manifests.
/// Results are sorted by title; `asset_map` is empty (fetch full manifests via
/// `get_case_manifest` when needed).
pub fn list_cases_cached(data_dir: &Path) -> Result<Vec<CaseManifest>, AppError> {
    let cases_dir = data_dir.join("case");
    let cache_file = cache_path(data_dir);

    let mut cached: LibraryCacheFile = if cache_file.exists() {
        fs::read_to_string(&cache_file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .filter(|c: &LibraryCacheFile| c.version == CACHE_VERSION)
            .unwrap_or(LibraryCacheFile { version: CACHE_VERSION, cases: HashMap::new() })
    } else {
        LibraryCacheFile { version: CACHE_VERSION, cases: HashMap::new() }
    };

    let mut cases: Vec<CaseManifest> = Vec::new();
    let mut changed = false;

    let entries = match fs::read_dir(&cases_dir) {
        Ok(e) if cases_dir.exists() => e,
        _ => return Ok(Vec::new()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
            continue;
        };
        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        let fingerprint = manifest_fingerprint(&manifest_path);
        match fingerprint {
            Some(fp) => {
                if cached.cases.get(&id).map(|e| e.fingerprint) == Some(fp) {
                    if let Some(entry) = cached.cases.get(&id) {
                        cases.push(entry.manifest.clone());
                        continue;
                    }
                }
                // Drift or new case — re-parse
                match read_manifest(&path) {
                    Ok(manifest) => {
                        let mut slim = manifest;
                        slim.asset_map = HashMap::new();
                        cached.cases.insert(
                            id,
                            CacheEntry { fingerprint: fp, manifest: slim.clone() },
                        );
                        cases.push(slim);
                        changed = true;
                    }
                    Err(e) => {
                        log::warn!("Skipping {}: {}", path.display(), e);
                    }
                }
            }
            None => {
                // Unreadable metadata — re-parse from disk
                if let Ok(manifest) = read_manifest(&path) {
                    let mut slim = manifest;
                    slim.asset_map = HashMap::new();
                    cases.push(slim);
                }
            }
        }
    }

    // Drop cache entries for deleted cases
    let live_ids: std::collections::HashSet<String> = fs::read_dir(&cases_dir)
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect())
        .unwrap_or_default();
    let before = cached.cases.len();
    cached.cases.retain(|id, _| live_ids.contains(id));
    changed |= cached.cases.len() != before;

    if changed {
        let json = serde_json::to_string(&cached)
            .map_err(|e| format!("Failed to serialize library cache: {}", e))?;
        fs::write(&cache_file, json)
            .map_err(|e| format!("Failed to write library cache: {}", e))?;
    }

    cases.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn write_manifest(dir: &std::path::Path, case_id: u32, title: &str) {
        let case_dir = dir.join("case").join(case_id.to_string());
        std::fs::create_dir_all(&case_dir).unwrap();
        let manifest = crate::downloader::manifest::CaseManifest {
            case_id,
            title: title.to_string(),
            author: "A".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: crate::downloader::manifest::AssetSummary {
                case_specific: 0, shared_defaults: 0, total_downloaded: 0, total_size_bytes: 0,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        crate::downloader::manifest::write_manifest(&manifest, &case_dir).unwrap();
    }

    #[test]
    fn test_list_cases_cached_basic() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), 1, "Zeta");
        write_manifest(dir.path(), 2, "Alpha");

        let cases = list_cases_cached(dir.path()).unwrap();
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].title, "Alpha", "must be sorted by title");
        assert_eq!(cases[1].title, "Zeta");
        assert!(dir.path().join("library_cache.json").exists(), "cache file must be written");
    }

    #[test]
    fn test_list_cases_cached_picks_up_manifest_changes() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), 1, "Old Title");

        let cases = list_cases_cached(dir.path()).unwrap();
        assert_eq!(cases[0].title, "Old Title");

        // Rewrite the manifest with a different title — wait for a distinct mtime
        std::thread::sleep(Duration::from_millis(20));
        write_manifest(dir.path(), 1, "New Title");

        let cases = list_cases_cached(dir.path()).unwrap();
        assert_eq!(cases[0].title, "New Title", "drifted manifest must be re-parsed");
    }

    #[test]
    fn test_list_cases_cached_drops_deleted_cases() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), 1, "Keep");
        write_manifest(dir.path(), 2, "Delete Me");
        list_cases_cached(dir.path()).unwrap();

        std::fs::remove_dir_all(dir.path().join("case/2")).unwrap();
        let cases = list_cases_cached(dir.path()).unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].case_id, 1);
    }

    #[test]
    fn test_list_cases_cached_asset_map_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), 1, "T");
        let cases = list_cases_cached(dir.path()).unwrap();
        assert!(
            cases[0].asset_map.is_empty(),
            "cached manifests must not carry asset_map (frontend uses get_case_manifest)"
        );
    }

    #[test]
    fn test_read_manifest_from_case_dir_after_cache() {
        // The command reads the FULL manifest (with asset_map) from disk.
        let dir = tempfile::tempdir().unwrap();
        let case_dir = dir.path().join("case").join("7");
        std::fs::create_dir_all(case_dir.join("assets")).unwrap();
        let mut manifest = crate::downloader::manifest::CaseManifest {
            case_id: 7,
            title: "Full".to_string(),
            author: "A".to_string(),
            language: "en".to_string(),
            download_date: "2026-01-01T00:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: crate::downloader::manifest::AssetSummary {
                case_specific: 1, shared_defaults: 0, total_downloaded: 1, total_size_bytes: 9,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        manifest.asset_map.insert("http://x/img.png".to_string(), "assets/img-hash.png".to_string());
        crate::downloader::manifest::write_manifest(&manifest, &case_dir).unwrap();

        let full = crate::downloader::manifest::read_manifest(&case_dir).unwrap();
        assert_eq!(full.asset_map.len(), 1, "full manifest must include asset_map");
        assert_eq!(full.asset_map["http://x/img.png"], "assets/img-hash.png");
    }
}
