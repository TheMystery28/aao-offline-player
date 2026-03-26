use std::collections::HashMap;
use std::path::Path;
use std::fs;

use serde_json::Value;
use xxhash_rust::xxh3::xxh3_64;

use super::manifest::{read_manifest, write_manifest};

/// Key for fast candidate lookup: (file_size, normalized_extension).
type DedupKey = (u64, String);

/// Index mapping (size, ext) → vec of (xxh3_hash, relative_path).
/// Multiple files can have the same (size, ext) but different hashes.
pub struct DedupIndex {
    entries: HashMap<DedupKey, Vec<(u64, String)>>,
}

impl DedupIndex {
    /// Build index from all files under `base_dir/prefix/` recursively.
    pub fn build_from_dir(base_dir: &Path, prefix: &str) -> Self {
        let mut entries: HashMap<DedupKey, Vec<(u64, String)>> = HashMap::new();
        let dir = base_dir.join(prefix);
        if !dir.is_dir() {
            return DedupIndex { entries };
        }
        Self::walk_dir(&dir, base_dir, &mut entries);
        DedupIndex { entries }
    }

    fn walk_dir(
        dir: &Path,
        base_dir: &Path,
        entries: &mut HashMap<DedupKey, Vec<(u64, String)>>,
    ) {
        let read_dir = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return,
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::walk_dir(&path, base_dir, entries);
            } else if path.is_file() {
                let size = match path.metadata() {
                    Ok(m) => m.len(),
                    Err(_) => continue,
                };
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(normalize_ext)
                    .unwrap_or_default();
                let hash = match hash_file(&path) {
                    Ok(h) => h,
                    Err(_) => continue,
                };
                let relative = match path.strip_prefix(base_dir) {
                    Ok(r) => r.to_string_lossy().replace('\\', "/"),
                    Err(_) => continue,
                };
                let key = (size, ext);
                entries.entry(key).or_default().push((hash, relative));
            }
        }
    }

    /// Look up a file: returns the matching default path if identical content found.
    /// 1. Check (size, ext) — O(1) HashMap lookup
    /// 2. If candidates exist, compute xxh3_64 of the file and compare
    pub fn find_duplicate(&self, file_path: &Path, _base_dir: &Path) -> Option<String> {
        let size = file_path.metadata().ok()?.len();
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(normalize_ext)
            .unwrap_or_default();
        let key = (size, ext);
        let candidates = self.entries.get(&key)?;
        let file_hash = hash_file(file_path).ok()?;
        for (candidate_hash, candidate_path) in candidates {
            if *candidate_hash == file_hash {
                return Some(candidate_path.clone());
            }
        }
        None
    }

    /// Add a new entry to the index.
    pub fn insert(&mut self, size: u64, ext: &str, hash: u64, relative_path: String) {
        let key = (size, normalize_ext(ext));
        self.entries.entry(key).or_default().push((hash, relative_path));
    }
}

/// Compute xxh3_64 hash of a file's contents.
pub fn hash_file(path: &Path) -> Result<u64, String> {
    let bytes =
        fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    Ok(xxh3_64(&bytes))
}

/// Normalize file extension for comparison.
pub fn normalize_ext(ext: &str) -> String {
    let lower = ext.to_lowercase();
    match lower.as_str() {
        "jpeg" => "jpg".to_string(),
        "htm" => "html".to_string(),
        "tiff" => "tif".to_string(),
        other => other.to_string(),
    }
}

/// Recursively walk a JSON value and replace all string occurrences of `old` with `new`.
pub fn rewrite_value_recursive(value: &mut Value, old: &str, new: &str) {
    match value {
        Value::String(s) if s == old => {
            *s = new.to_string();
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                rewrite_value_recursive(item, old, new);
            }
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                rewrite_value_recursive(v, old, new);
            }
        }
        _ => {}
    }
}

/// Dedup a single case's assets against the shared defaults pool.
/// Returns the number of files deduplicated and bytes saved.
pub fn dedup_case_assets(case_id: u32, data_dir: &Path) -> Result<(usize, u64), String> {
    let case_dir = data_dir.join("case").join(case_id.to_string());
    let assets_dir = case_dir.join("assets");
    if !assets_dir.is_dir() {
        return Ok((0, 0));
    }

    // Build index from all defaults/ files
    let defaults_dir = data_dir.join("defaults");
    if !defaults_dir.is_dir() {
        return Ok((0, 0));
    }
    let index = DedupIndex::build_from_dir(data_dir, "defaults");

    // Read manifest
    let mut manifest = read_manifest(&case_dir)?;

    // Read trial_data.json for URL rewriting
    let trial_data_path = case_dir.join("trial_data.json");
    let mut trial_data: Option<Value> = if trial_data_path.exists() {
        let text = fs::read_to_string(&trial_data_path)
            .map_err(|e| format!("Failed to read trial_data.json: {}", e))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse trial_data.json: {}", e))
            .ok()
    } else {
        None
    };

    let mut deduped_count = 0usize;
    let mut bytes_saved = 0u64;
    let mut trial_data_modified = false;

    // Collect asset files first to avoid borrow issues with fs::remove_file during iteration
    let asset_files: Vec<_> = match fs::read_dir(&assets_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect(),
        Err(_) => return Ok((0, 0)),
    };

    for entry in &asset_files {
        let file_path = entry.path();
        let file_size = match file_path.metadata() {
            Ok(m) => m.len(),
            Err(_) => continue,
        };

        // Check if this file has a duplicate in defaults/
        if let Some(default_relative_path) = index.find_duplicate(&file_path, data_dir) {
            let asset_filename = match file_path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let old_local_path = format!("assets/{}", asset_filename);

            // Find the URL(s) in asset_map that point to this assets/ path
            let urls_to_update: Vec<String> = manifest
                .asset_map
                .iter()
                .filter(|(_, v)| **v == old_local_path)
                .map(|(k, _)| k.clone())
                .collect();

            if urls_to_update.is_empty() {
                continue; // No manifest entry points here, skip
            }

            // Verify the default file actually exists on disk before deleting the case copy
            let default_full_path = data_dir.join(&default_relative_path);
            if !default_full_path.is_file() {
                continue;
            }

            // Update manifest
            for url in &urls_to_update {
                manifest
                    .asset_map
                    .insert(url.clone(), default_relative_path.clone());
            }

            // Rewrite references in trial_data.json
            if let Some(ref mut td) = trial_data {
                let old_server_path = format!("case/{}/{}", case_id, old_local_path);
                rewrite_value_recursive(td, &old_server_path, &default_relative_path);
                trial_data_modified = true;
            }

            // Delete the duplicate file
            if fs::remove_file(&file_path).is_ok() {
                deduped_count += 1;
                bytes_saved += file_size;
            }
        }
    }

    if deduped_count > 0 {
        // Save updated manifest
        manifest.assets.total_downloaded = manifest.asset_map.len();
        write_manifest(&manifest, &case_dir)?;

        // Save updated trial_data
        if trial_data_modified {
            if let Some(td) = &trial_data {
                let json = serde_json::to_string_pretty(td)
                    .map_err(|e| format!("Failed to serialize trial_data: {}", e))?;
                fs::write(&trial_data_path, json)
                    .map_err(|e| format!("Failed to write trial_data.json: {}", e))?;
            }
        }
    }

    Ok((deduped_count, bytes_saved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_normalize_ext() {
        assert_eq!(normalize_ext("GIF"), "gif");
        assert_eq!(normalize_ext("jpeg"), "jpg");
        assert_eq!(normalize_ext("JPEG"), "jpg");
        assert_eq!(normalize_ext("PNG"), "png");
        assert_eq!(normalize_ext("htm"), "html");
        assert_eq!(normalize_ext("HTM"), "html");
        assert_eq!(normalize_ext("tiff"), "tif");
        assert_eq!(normalize_ext("TIFF"), "tif");
        assert_eq!(normalize_ext("mp3"), "mp3");
        assert_eq!(normalize_ext("jpg"), "jpg");
    }

    #[test]
    fn test_hash_file_consistent() {
        let dir = tempfile::tempdir().unwrap();
        let file_a = dir.path().join("a.bin");
        let file_b = dir.path().join("b.bin");
        let file_c = dir.path().join("c.bin");

        fs::write(&file_a, b"hello world").unwrap();
        fs::write(&file_b, b"hello world").unwrap(); // same content
        fs::write(&file_c, b"different content").unwrap();

        let hash_a = hash_file(&file_a).unwrap();
        let hash_b = hash_file(&file_b).unwrap();
        let hash_c = hash_file(&file_c).unwrap();

        assert_eq!(hash_a, hash_b, "Same content should produce same hash");
        assert_ne!(hash_a, hash_c, "Different content should produce different hash");
    }

    #[test]
    fn test_dedup_index_build_and_lookup() {
        let dir = tempfile::tempdir().unwrap();

        // Create defaults/ with a known file
        let defaults_dir = dir.path().join("defaults").join("images");
        fs::create_dir_all(&defaults_dir).unwrap();
        fs::write(defaults_dir.join("sprite.gif"), b"sprite content").unwrap();

        // Build index
        let index = DedupIndex::build_from_dir(dir.path(), "defaults");

        // Create a candidate file with same content
        let candidate = dir.path().join("candidate.gif");
        fs::write(&candidate, b"sprite content").unwrap();
        let result = index.find_duplicate(&candidate, dir.path());
        assert!(result.is_some(), "Should find duplicate");
        assert!(
            result.unwrap().contains("sprite.gif"),
            "Should return the defaults path"
        );

        // Create a candidate with different content
        let different = dir.path().join("different.gif");
        fs::write(&different, b"different content here").unwrap();
        let result = index.find_duplicate(&different, dir.path());
        assert!(result.is_none(), "Different content should not match");
    }

    #[test]
    fn test_dedup_index_size_mismatch_skips_hash() {
        let dir = tempfile::tempdir().unwrap();

        // Create defaults/ with a known file
        let defaults_dir = dir.path().join("defaults");
        fs::create_dir_all(&defaults_dir).unwrap();
        fs::write(defaults_dir.join("small.gif"), b"small").unwrap();

        let index = DedupIndex::build_from_dir(dir.path(), "defaults");

        // Different size, same extension → no match (hash never computed for candidate)
        let candidate = dir.path().join("candidate.gif");
        fs::write(&candidate, b"this is a much larger file with different size").unwrap();
        let result = index.find_duplicate(&candidate, dir.path());
        assert!(
            result.is_none(),
            "Different file size should not match even with same extension"
        );
    }

    #[test]
    fn test_rewrite_value_recursive() {
        let mut data = serde_json::json!({
            "profiles": [null, {
                "custom_sprites": [{
                    "talking": "case/99/assets/sprite-abc.gif",
                    "still": "case/99/assets/sprite-abc.gif",
                    "startup": ""
                }]
            }],
            "nested": {
                "deep": "case/99/assets/sprite-abc.gif"
            },
            "unrelated": "keep this"
        });

        rewrite_value_recursive(
            &mut data,
            "case/99/assets/sprite-abc.gif",
            "defaults/images/chars/Olga/1.gif",
        );

        assert_eq!(
            data["profiles"][1]["custom_sprites"][0]["talking"],
            "defaults/images/chars/Olga/1.gif"
        );
        assert_eq!(
            data["profiles"][1]["custom_sprites"][0]["still"],
            "defaults/images/chars/Olga/1.gif"
        );
        assert_eq!(
            data["profiles"][1]["custom_sprites"][0]["startup"],
            ""
        );
        assert_eq!(data["nested"]["deep"], "defaults/images/chars/Olga/1.gif");
        assert_eq!(data["unrelated"], "keep this");
    }

    #[test]
    fn test_dedup_case_assets_removes_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();

        // Create defaults/ with a known file
        let defaults_chars = data_dir.join("defaults").join("images").join("chars").join("Olga");
        fs::create_dir_all(&defaults_chars).unwrap();
        fs::write(defaults_chars.join("1.gif"), b"sprite bytes").unwrap();

        // Create case with assets/ containing identical file
        let case_dir = data_dir.join("case").join("99");
        let assets_dir = case_dir.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("sprite-abc123.gif"), b"sprite bytes").unwrap();

        // Create manifest
        let mut asset_map = HashMap::new();
        asset_map.insert(
            "http://example.com/sprite.gif".to_string(),
            "assets/sprite-abc123.gif".to_string(),
        );
        let manifest = super::super::manifest::CaseManifest {
            case_id: 99,
            title: "Test".to_string(),
            author: "Author".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: super::super::manifest::AssetSummary {
                case_specific: 1,
                shared_defaults: 0,
                total_downloaded: 1,
                total_size_bytes: 12,
            },
            asset_map,
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        // Create trial_data.json with reference to the asset
        let trial_data = serde_json::json!({
            "profiles": [null, {
                "custom_sprites": [{
                    "talking": "case/99/assets/sprite-abc123.gif",
                    "still": "",
                    "startup": ""
                }]
            }]
        });
        fs::write(
            case_dir.join("trial_data.json"),
            serde_json::to_string_pretty(&trial_data).unwrap(),
        ).unwrap();

        // Run dedup
        let (count, bytes) = dedup_case_assets(99, data_dir).unwrap();
        assert_eq!(count, 1, "Should dedup 1 file");
        assert_eq!(bytes, 12, "Should save 12 bytes");

        // Verify file deleted from assets/
        assert!(!assets_dir.join("sprite-abc123.gif").exists());

        // Verify manifest updated
        let updated = read_manifest(&case_dir).unwrap();
        assert_eq!(
            updated.asset_map["http://example.com/sprite.gif"],
            "defaults/images/chars/Olga/1.gif"
        );

        // Verify trial_data rewritten
        let td_str = fs::read_to_string(case_dir.join("trial_data.json")).unwrap();
        let td: Value = serde_json::from_str(&td_str).unwrap();
        assert_eq!(
            td["profiles"][1]["custom_sprites"][0]["talking"],
            "defaults/images/chars/Olga/1.gif"
        );
    }

    #[test]
    fn test_dedup_case_assets_preserves_unique() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();

        // Create defaults/ with a known file
        let defaults_dir = data_dir.join("defaults");
        fs::create_dir_all(&defaults_dir).unwrap();
        fs::write(defaults_dir.join("other.gif"), b"other content").unwrap();

        // Create case with a UNIQUE asset (different content)
        let case_dir = data_dir.join("case").join("50");
        let assets_dir = case_dir.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("unique-abc.gif"), b"unique content").unwrap();

        let mut asset_map = HashMap::new();
        asset_map.insert(
            "http://example.com/unique.gif".to_string(),
            "assets/unique-abc.gif".to_string(),
        );
        let manifest = super::super::manifest::CaseManifest {
            case_id: 50,
            title: "Test".to_string(),
            author: "Author".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: super::super::manifest::AssetSummary {
                case_specific: 1,
                shared_defaults: 0,
                total_downloaded: 1,
                total_size_bytes: 14,
            },
            asset_map,
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        let (count, bytes) = dedup_case_assets(50, data_dir).unwrap();
        assert_eq!(count, 0, "Unique asset should not be deduped");
        assert_eq!(bytes, 0);
        assert!(assets_dir.join("unique-abc.gif").exists(), "File should still exist");
    }

    #[test]
    fn test_dedup_case_assets_no_defaults_dir() {
        let dir = tempfile::tempdir().unwrap();
        // No defaults/ dir exists
        let case_dir = dir.path().join("case").join("1");
        let assets_dir = case_dir.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("file.gif"), b"data").unwrap();

        let (count, _) = dedup_case_assets(1, dir.path()).unwrap();
        assert_eq!(count, 0, "No defaults dir → no dedup");
    }

    #[test]
    fn test_dedup_case_assets_no_assets_dir() {
        let dir = tempfile::tempdir().unwrap();
        let case_dir = dir.path().join("case").join("2");
        fs::create_dir_all(&case_dir).unwrap();
        // No assets/ dir

        let (count, _) = dedup_case_assets(2, dir.path()).unwrap();
        assert_eq!(count, 0, "No assets dir → no dedup");
    }
}
