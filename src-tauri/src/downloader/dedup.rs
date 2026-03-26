use std::collections::HashMap;
use std::path::Path;
use std::fs;

use redb::{Database, MultimapTableDefinition, ReadableMultimapTable, TableDefinition};
use serde_json::Value;
use xxhash_rust::xxh3::xxh3_64;

use super::manifest::{read_manifest, write_manifest};

/// Primary index: relative_path → (file_size, xxh3_hash)
const HASH_BY_PATH: TableDefinition<&str, (u64, u64)> =
    TableDefinition::new("hash_by_path");

/// Secondary lookup: "{size}:{normalized_ext}" → relative_path (multimap)
const PATHS_BY_SIZE_EXT: MultimapTableDefinition<&str, &str> =
    MultimapTableDefinition::new("paths_by_size_ext");

/// Persistent dedup index backed by redb.
/// Stores xxh3 hashes of default/shared assets for O(log n) duplicate lookups
/// without re-reading files from disk.
pub struct DedupIndex {
    db: Database,
}

impl DedupIndex {
    /// Open or create the index database at `data_dir/dedup_index.redb`.
    /// If the db file is corrupt, deletes and recreates it.
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        let db_path = data_dir.join("dedup_index.redb");
        match Database::create(&db_path) {
            Ok(db) => Ok(DedupIndex { db }),
            Err(_) => {
                // Corrupt db — delete and retry
                let _ = fs::remove_file(&db_path);
                let db = Database::create(&db_path)
                    .map_err(|e| format!("Failed to create dedup index: {}", e))?;
                Ok(DedupIndex { db })
            }
        }
    }

    /// Register a file in the index.
    /// Inserts into both hash_by_path and paths_by_size_ext in one transaction.
    pub fn register(&self, relative_path: &str, size: u64, hash: u64) -> Result<(), String> {
        let ext = Path::new(relative_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(normalize_ext)
            .unwrap_or_default();
        let size_ext_key = format!("{}:{}", size, ext);

        let txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write: {}", e))?;
        {
            let mut hash_table = txn.open_table(HASH_BY_PATH)
                .map_err(|e| format!("Failed to open hash table: {}", e))?;
            hash_table.insert(relative_path, (size, hash))
                .map_err(|e| format!("Failed to insert hash: {}", e))?;

            let mut lookup_table = txn.open_multimap_table(PATHS_BY_SIZE_EXT)
                .map_err(|e| format!("Failed to open lookup table: {}", e))?;
            lookup_table.insert(&*size_ext_key, relative_path)
                .map_err(|e| format!("Failed to insert lookup: {}", e))?;
        }
        txn.commit().map_err(|e| format!("Failed to commit: {}", e))?;
        Ok(())
    }

    /// Remove a file from the index.
    /// Reads old entry to reconstruct the size+ext key, then removes from both tables.
    pub fn unregister(&self, relative_path: &str) -> Result<(), String> {
        // Read old entry to get size for the secondary key
        let old_entry = {
            let txn = self.db.begin_read()
                .map_err(|e| format!("Failed to begin read: {}", e))?;
            match txn.open_table(HASH_BY_PATH) {
                Ok(table) => table.get(relative_path)
                    .map_err(|e| format!("Failed to read: {}", e))?
                    .map(|v| v.value()),
                Err(_) => None,
            }
        };

        if let Some((size, _hash)) = old_entry {
            let ext = Path::new(relative_path)
                .extension()
                .and_then(|e| e.to_str())
                .map(normalize_ext)
                .unwrap_or_default();
            let size_ext_key = format!("{}:{}", size, ext);

            let txn = self.db.begin_write()
                .map_err(|e| format!("Failed to begin write: {}", e))?;
            {
                let mut hash_table = txn.open_table(HASH_BY_PATH)
                    .map_err(|e| format!("Failed to open hash table: {}", e))?;
                let _ = hash_table.remove(relative_path);

                let mut lookup_table = txn.open_multimap_table(PATHS_BY_SIZE_EXT)
                    .map_err(|e| format!("Failed to open lookup table: {}", e))?;
                let _ = lookup_table.remove(&*size_ext_key, relative_path);
            }
            txn.commit().map_err(|e| format!("Failed to commit: {}", e))?;
        }
        Ok(())
    }

    /// Find a duplicate in the index for the given file.
    /// 1. Compute "{size}:{ext}" key from the candidate file
    /// 2. Look up candidates in paths_by_size_ext (B-tree, O(log n))
    /// 3. For each candidate, compare xxh3 hashes
    /// Returns the matching default's relative path if identical content found.
    pub fn find_duplicate(&self, file_path: &Path, _base_dir: &Path) -> Option<String> {
        let size = file_path.metadata().ok()?.len();
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(normalize_ext)
            .unwrap_or_default();
        let size_ext_key = format!("{}:{}", size, ext);

        let txn = self.db.begin_read().ok()?;
        let lookup_table = txn.open_multimap_table(PATHS_BY_SIZE_EXT).ok()?;
        let candidates = lookup_table.get(&*size_ext_key).ok()?;

        let hash_table = txn.open_table(HASH_BY_PATH).ok()?;
        let file_hash = hash_file(file_path).ok()?;

        for candidate in candidates.flatten() {
            let candidate_path = candidate.value().to_string();
            if let Ok(Some(entry)) = hash_table.get(&*candidate_path) {
                let (_size, candidate_hash) = entry.value();
                if candidate_hash == file_hash {
                    return Some(candidate_path);
                }
            }
        }
        None
    }

    /// Scan a directory and register all files not already in the db.
    /// Used on first run or when the index is out of date.
    /// Returns the count of newly registered files.
    pub fn scan_and_register(&self, data_dir: &Path, prefix: &str) -> Result<usize, String> {
        let dir = data_dir.join(prefix);
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut count = 0;
        Self::walk_and_register(&dir, data_dir, &self.db, &mut count)?;
        Ok(count)
    }

    fn walk_and_register(
        dir: &Path,
        base_dir: &Path,
        db: &Database,
        count: &mut usize,
    ) -> Result<(), String> {
        let read_dir = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return Ok(()),
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::walk_and_register(&path, base_dir, db, count)?;
            } else if path.is_file() {
                let relative = match path.strip_prefix(base_dir) {
                    Ok(r) => r.to_string_lossy().replace('\\', "/"),
                    Err(_) => continue,
                };

                // Check if already registered
                let already_exists = {
                    let txn = db.begin_read()
                        .map_err(|e| format!("Failed to begin read: {}", e))?;
                    match txn.open_table(HASH_BY_PATH) {
                        Ok(table) => table.get(&*relative)
                            .map_err(|e| format!("Failed to read: {}", e))?
                            .is_some(),
                        Err(_) => false,
                    }
                };

                if already_exists {
                    continue;
                }

                let size = match path.metadata() {
                    Ok(m) => m.len(),
                    Err(_) => continue,
                };
                let hash = match hash_file(&path) {
                    Ok(h) => h,
                    Err(_) => continue,
                };
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(normalize_ext)
                    .unwrap_or_default();
                let size_ext_key = format!("{}:{}", size, ext);

                let txn = db.begin_write()
                    .map_err(|e| format!("Failed to begin write: {}", e))?;
                {
                    let mut hash_table = txn.open_table(HASH_BY_PATH)
                        .map_err(|e| format!("Failed to open hash table: {}", e))?;
                    hash_table.insert(&*relative, (size, hash))
                        .map_err(|e| format!("Failed to insert: {}", e))?;

                    let mut lookup_table = txn.open_multimap_table(PATHS_BY_SIZE_EXT)
                        .map_err(|e| format!("Failed to open lookup table: {}", e))?;
                    lookup_table.insert(&*size_ext_key, &*relative)
                        .map_err(|e| format!("Failed to insert lookup: {}", e))?;
                }
                txn.commit().map_err(|e| format!("Failed to commit: {}", e))?;
                *count += 1;
            }
        }
        Ok(())
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

    // Open persistent index and ensure defaults/ are registered
    let defaults_dir = data_dir.join("defaults");
    if !defaults_dir.is_dir() {
        return Ok((0, 0));
    }
    let index = DedupIndex::open(data_dir)?;
    index.scan_and_register(data_dir, "defaults")?;

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

/// Clear default assets that are not referenced by any case manifest.
/// Scans all manifests to build a set of used defaults/ paths, then deletes the rest.
/// Returns (files_deleted, bytes_freed).
pub fn clear_unused_defaults(data_dir: &Path) -> Result<(usize, u64), String> {
    // Collect all defaults/ paths referenced by any case manifest
    let mut used_defaults: std::collections::HashSet<String> = std::collections::HashSet::new();
    let cases_dir = data_dir.join("case");
    if cases_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&cases_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Ok(manifest) = read_manifest(&path) {
                    for local_path in manifest.asset_map.values() {
                        if local_path.starts_with("defaults/") {
                            used_defaults.insert(local_path.replace('\\', "/"));
                        }
                    }
                }
            }
        }
    }

    // Walk defaults/ and delete files not in the used set
    let defaults_dir = data_dir.join("defaults");
    if !defaults_dir.is_dir() {
        return Ok((0, 0));
    }

    // Open index to unregister deleted files
    let index = DedupIndex::open(data_dir).ok();

    let mut deleted_count = 0usize;
    let mut bytes_freed = 0u64;

    fn walk_and_clean(
        dir: &std::path::Path,
        base_dir: &std::path::Path,
        used: &std::collections::HashSet<String>,
        index: Option<&DedupIndex>,
        deleted: &mut usize,
        freed: &mut u64,
    ) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_and_clean(&path, base_dir, used, index, deleted, freed);
                // Remove empty directories
                let _ = fs::remove_dir(&path);
            } else if path.is_file() {
                let relative = match path.strip_prefix(base_dir) {
                    Ok(r) => r.to_string_lossy().replace('\\', "/"),
                    Err(_) => continue,
                };
                if !used.contains(&relative) {
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    if fs::remove_file(&path).is_ok() {
                        // Unregister from persistent index
                        if let Some(idx) = index {
                            let _ = idx.unregister(&relative);
                        }
                        *deleted += 1;
                        *freed += size;
                    }
                }
            }
        }
    }

    walk_and_clean(&defaults_dir, data_dir, &used_defaults, index.as_ref(), &mut deleted_count, &mut bytes_freed);

    Ok((deleted_count, bytes_freed))
}

/// List all case directories under `data_dir/case/` with parseable numeric IDs.
pub fn list_case_dirs(data_dir: &Path) -> Result<Vec<(u32, std::path::PathBuf)>, String> {
    let cases_dir = data_dir.join("case");
    if !cases_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let entries = fs::read_dir(&cases_dir)
        .map_err(|e| format!("Failed to read case directory: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if let Ok(id) = name.parse::<u32>() {
                result.push((id, path));
            }
        }
    }
    result.sort_by_key(|(id, _)| *id);
    Ok(result)
}

/// Global optimization: find assets shared across multiple cases, promote to defaults/shared/.
/// Then run single-case dedup for each case against the full defaults pool.
/// Returns total files deduplicated and bytes saved.
pub fn optimize_all_cases(
    data_dir: &Path,
    on_progress: Option<&dyn Fn(usize, usize)>,
) -> Result<(usize, u64), String> {
    let case_dirs = list_case_dirs(data_dir)?;
    if case_dirs.is_empty() {
        return Ok((0, 0));
    }

    let total_phases = case_dirs.len() * 2; // scan + dedup for each case
    let mut progress = 0usize;

    // Phase 1: Build content map of ALL case assets across ALL cases
    //   (size, normalized_ext, hash) → [(case_id, filename)]
    type ContentKey = (u64, String, u64);
    let mut content_map: HashMap<ContentKey, Vec<(u32, String)>> = HashMap::new();

    for (case_id, case_dir) in &case_dirs {
        let assets_dir = case_dir.join("assets");
        if assets_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&assets_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
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
                    let filename = match path.file_name().and_then(|n| n.to_str()) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };
                    let key = (size, ext, hash);
                    content_map.entry(key).or_default().push((*case_id, filename));
                }
            }
        }
        progress += 1;
        if let Some(cb) = &on_progress {
            cb(progress, total_phases);
        }
    }

    let mut total_deduped = 0usize;
    let mut total_saved = 0u64;

    // Phase 2: Promote entries with 2+ occurrences to defaults/shared/
    let shared_dir = data_dir.join("defaults").join("shared");
    for ((size, ext, hash), entries) in &content_map {
        if entries.len() < 2 {
            continue;
        }

        // Determine shared path
        let shared_relative = format!("defaults/shared/{:016x}.{}", hash, ext);
        let shared_full = data_dir.join(&shared_relative);

        // Copy first available source to shared location (if not already there)
        let already_shared = shared_full.is_file();
        if !already_shared {
            let mut copied = false;
            for (case_id, filename) in entries {
                let src = data_dir
                    .join("case")
                    .join(case_id.to_string())
                    .join("assets")
                    .join(filename);
                if src.is_file() {
                    if let Err(_) = fs::create_dir_all(&shared_dir) {
                        continue;
                    }
                    if fs::copy(&src, &shared_full).is_ok() {
                        copied = true;
                        break;
                    }
                }
            }
            if !copied {
                continue; // Could not create shared copy, skip this group
            }

            // Register the new shared asset in the persistent index
            if let Ok(idx) = DedupIndex::open(data_dir) {
                let _ = idx.register(&shared_relative, *size, *hash);
            }
        }

        // Track how many copies we delete for this group
        let mut group_deleted = 0u32;

        // Update all cases referencing this content
        for (case_id, filename) in entries {
            let case_dir = data_dir.join("case").join(case_id.to_string());
            let asset_path = case_dir.join("assets").join(filename);
            if !asset_path.is_file() {
                continue; // Already removed by a previous pass
            }

            let old_local_path = format!("assets/{}", filename);

            // Update manifest
            if let Ok(mut manifest) = read_manifest(&case_dir) {
                let urls_to_update: Vec<String> = manifest
                    .asset_map
                    .iter()
                    .filter(|(_, v)| **v == old_local_path)
                    .map(|(k, _)| k.clone())
                    .collect();

                if urls_to_update.is_empty() {
                    continue;
                }

                for url in &urls_to_update {
                    manifest
                        .asset_map
                        .insert(url.clone(), shared_relative.clone());
                }
                manifest.assets.total_downloaded = manifest.asset_map.len();
                let _ = write_manifest(&manifest, &case_dir);
            }

            // Rewrite trial_data
            let trial_data_path = case_dir.join("trial_data.json");
            if trial_data_path.exists() {
                if let Ok(text) = fs::read_to_string(&trial_data_path) {
                    if let Ok(mut td) = serde_json::from_str::<Value>(&text) {
                        let old_server_path =
                            format!("case/{}/{}", case_id, old_local_path);
                        rewrite_value_recursive(&mut td, &old_server_path, &shared_relative);
                        if let Ok(json) = serde_json::to_string_pretty(&td) {
                            let _ = fs::write(&trial_data_path, json);
                        }
                    }
                }
            }

            // Delete the case-specific copy
            if fs::remove_file(&asset_path).is_ok() {
                total_deduped += 1;
                group_deleted += 1;
            }
        }

        // Net savings: we deleted N copies but created 1 shared copy (if it didn't already exist).
        // So net bytes saved = (deleted * size) - (size if we created the shared copy).
        if group_deleted > 0 {
            let created_cost = if already_shared { 0 } else { *size };
            total_saved += (group_deleted as u64) * size - created_cost;
        }
    }

    // Phase 3: Run single-case dedup for each case against the full defaults pool
    // (catches assets matching existing defaults that weren't cross-case duplicates)
    for (case_id, _) in &case_dirs {
        let (n, b) = dedup_case_assets(*case_id, data_dir).unwrap_or((0, 0));
        total_deduped += n;
        total_saved += b;

        progress += 1;
        if let Some(cb) = &on_progress {
            cb(progress, total_phases);
        }
    }

    Ok((total_deduped, total_saved))
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
    fn test_dedup_index_scan_register_and_find() {
        let dir = tempfile::tempdir().unwrap();

        // Create defaults/ with a known file
        let defaults_dir = dir.path().join("defaults").join("images");
        fs::create_dir_all(&defaults_dir).unwrap();
        fs::write(defaults_dir.join("sprite.gif"), b"sprite content").unwrap();

        // Open index and scan
        let index = DedupIndex::open(dir.path()).unwrap();
        let count = index.scan_and_register(dir.path(), "defaults").unwrap();
        assert_eq!(count, 1, "Should register 1 file");

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

        let index = DedupIndex::open(dir.path()).unwrap();
        index.scan_and_register(dir.path(), "defaults").unwrap();

        // Different size, same extension → no match
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

    // --- optimize_all_cases ---

    fn make_case_with_asset(data_dir: &Path, case_id: u32, filename: &str, content: &[u8]) {
        let case_dir = data_dir.join("case").join(case_id.to_string());
        let assets_dir = case_dir.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join(filename), content).unwrap();

        let mut asset_map = HashMap::new();
        asset_map.insert(
            format!("http://example.com/{}", filename),
            format!("assets/{}", filename),
        );
        let manifest = super::super::manifest::CaseManifest {
            case_id,
            title: format!("Case {}", case_id),
            author: "Author".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: super::super::manifest::AssetSummary {
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

    #[test]
    fn test_optimize_all_cases_promotes_shared() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();
        let content = b"shared sprite data for testing";

        // Two cases with identical assets (different filenames)
        make_case_with_asset(data_dir, 100, "bg-aaa.jpg", content);
        make_case_with_asset(data_dir, 200, "bg-bbb.jpg", content);

        let (count, bytes) = optimize_all_cases(data_dir, None).unwrap();
        assert!(count >= 2, "Should dedup at least 2 files, got {}", count);
        // Net savings: deleted 2 case copies, created 1 shared copy → net = 1x file size
        assert_eq!(bytes, content.len() as u64, "Net savings should be 1x file size (2 deleted - 1 created)");

        // Verify shared file exists in defaults/shared/
        let shared_dir = data_dir.join("defaults").join("shared");
        assert!(shared_dir.is_dir(), "defaults/shared/ should exist");
        let shared_files: Vec<_> = fs::read_dir(&shared_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();
        assert_eq!(shared_files.len(), 1, "Should have exactly 1 shared file");

        // Verify original assets/ files deleted
        assert!(!data_dir.join("case/100/assets/bg-aaa.jpg").exists());
        assert!(!data_dir.join("case/200/assets/bg-bbb.jpg").exists());

        // Verify manifests updated to shared path
        let m100 = read_manifest(&data_dir.join("case/100")).unwrap();
        let path100 = &m100.asset_map["http://example.com/bg-aaa.jpg"];
        assert!(path100.starts_with("defaults/shared/"), "Manifest should point to shared, got: {}", path100);

        let m200 = read_manifest(&data_dir.join("case/200")).unwrap();
        let path200 = &m200.asset_map["http://example.com/bg-bbb.jpg"];
        assert!(path200.starts_with("defaults/shared/"), "Manifest should point to shared, got: {}", path200);
    }

    #[test]
    fn test_optimize_all_cases_skips_singletons() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();

        // One case with unique asset (no duplicate anywhere)
        make_case_with_asset(data_dir, 300, "unique-xyz.gif", b"unique content");

        let (count, _) = optimize_all_cases(data_dir, None).unwrap();
        assert_eq!(count, 0, "Singleton should not be promoted or deduped");
        assert!(data_dir.join("case/300/assets/unique-xyz.gif").exists(), "File should still exist");
    }

    #[test]
    fn test_optimize_all_cases_empty_no_crash() {
        let dir = tempfile::tempdir().unwrap();
        // No case/ dir at all
        let (count, bytes) = optimize_all_cases(dir.path(), None).unwrap();
        assert_eq!(count, 0);
        assert_eq!(bytes, 0);
    }

    #[test]
    fn test_optimize_all_cases_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();

        make_case_with_asset(data_dir, 400, "sprite-a.gif", b"identical content");
        make_case_with_asset(data_dir, 500, "sprite-b.gif", b"identical content");

        let (count1, _) = optimize_all_cases(data_dir, None).unwrap();
        assert!(count1 >= 2);

        // Run again — should do nothing
        let (count2, bytes2) = optimize_all_cases(data_dir, None).unwrap();
        assert_eq!(count2, 0, "Second run should find nothing to dedup");
        assert_eq!(bytes2, 0);
    }

    #[test]
    fn test_export_after_dedup_includes_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();

        // Create defaults/ with a known file
        let defaults_chars = data_dir.join("defaults").join("images").join("chars").join("Olga");
        fs::create_dir_all(&defaults_chars).unwrap();
        fs::write(defaults_chars.join("1.gif"), b"olga sprite content").unwrap();

        // Create case with identical asset in assets/
        let case_dir = data_dir.join("case").join("77");
        let assets_dir = case_dir.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("sprite-olga.gif"), b"olga sprite content").unwrap();

        // Create manifest and trial_data
        let mut asset_map = HashMap::new();
        asset_map.insert(
            "http://example.com/olga.gif".to_string(),
            "assets/sprite-olga.gif".to_string(),
        );
        let manifest = super::super::manifest::CaseManifest {
            case_id: 77,
            title: "Export Dedup Test".to_string(),
            author: "Author".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-01".to_string(),
            format: "v6".to_string(),
            sequence: None,
            assets: super::super::manifest::AssetSummary {
                case_specific: 1, shared_defaults: 0,
                total_downloaded: 1, total_size_bytes: 19,
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
                    "talking": "case/77/assets/sprite-olga.gif",
                    "still": "", "startup": ""
                }]
            }]
        });
        fs::write(
            case_dir.join("trial_data.json"),
            serde_json::to_string_pretty(&trial_data).unwrap(),
        ).unwrap();

        // Run dedup — asset should be deduped to default path
        let (count, _) = dedup_case_assets(77, data_dir).unwrap();
        assert_eq!(count, 1, "Should dedup 1 file");
        assert!(!assets_dir.join("sprite-olga.gif").exists(), "Original should be deleted");

        // Verify manifest points to defaults/
        let updated_manifest = read_manifest(&case_dir).unwrap();
        let path = &updated_manifest.asset_map["http://example.com/olga.gif"];
        assert!(path.starts_with("defaults/"), "Manifest should point to defaults/, got: {}", path);

        // Export the case
        let export_path = dir.path().join("test.aaocase");
        crate::importer::export_aaocase(77, data_dir, &export_path, None, None, true).unwrap();
        assert!(export_path.exists(), "ZIP should exist");

        // Verify the ZIP contains the defaults/ file
        let file = fs::File::open(&export_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut found_default = false;
        let mut found_manifest = false;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).unwrap();
            let name = entry.name().to_string();
            if name.contains("defaults/images/chars/Olga/1.gif") {
                found_default = true;
            }
            if name == "manifest.json" {
                found_manifest = true;
            }
        }
        assert!(found_default, "ZIP should contain the defaults/ sprite file");
        assert!(found_manifest, "ZIP should contain manifest.json");

        // Verify the exported manifest has the correct path
        let manifest_text = {
            let mut entry = archive.by_name("manifest.json").unwrap();
            let mut s = String::new();
            std::io::Read::read_to_string(&mut entry, &mut s).unwrap();
            s
        };
        let exported_manifest: super::super::manifest::CaseManifest =
            serde_json::from_str(&manifest_text).unwrap();
        let exported_path = &exported_manifest.asset_map["http://example.com/olga.gif"];
        assert!(
            exported_path.starts_with("defaults/"),
            "Exported manifest should point to defaults/, got: {}",
            exported_path
        );
    }

    // --- clear_unused_defaults ---

    #[test]
    fn test_clear_unused_defaults_removes_only_unreferenced() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();

        // Create defaults/ with 3 files: 2 used by a case, 1 unused
        let chars_dir = data_dir.join("defaults").join("images").join("chars").join("Olga");
        fs::create_dir_all(&chars_dir).unwrap();
        fs::write(chars_dir.join("1.gif"), b"used sprite").unwrap();
        fs::write(chars_dir.join("2.gif"), b"also used").unwrap();
        let unused_dir = data_dir.join("defaults").join("music");
        fs::create_dir_all(&unused_dir).unwrap();
        fs::write(unused_dir.join("old_track.mp3"), b"unused music file").unwrap();

        // Create a case whose manifest references only the 2 used sprites
        let case_dir = data_dir.join("case").join("10");
        fs::create_dir_all(&case_dir).unwrap();
        let mut asset_map = HashMap::new();
        asset_map.insert("http://a.com/1".into(), "defaults/images/chars/Olga/1.gif".into());
        asset_map.insert("http://a.com/2".into(), "defaults/images/chars/Olga/2.gif".into());
        let manifest = super::super::manifest::CaseManifest {
            case_id: 10,
            title: "Test".into(), author: "A".into(), language: "en".into(),
            download_date: "2025-01-01".into(), format: "v6".into(),
            sequence: None,
            assets: super::super::manifest::AssetSummary {
                case_specific: 0, shared_defaults: 2, total_downloaded: 2, total_size_bytes: 20,
            },
            asset_map,
            failed_assets: vec![], has_plugins: false, has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        // Run clear
        let (deleted, bytes) = clear_unused_defaults(data_dir).unwrap();
        assert_eq!(deleted, 1, "Should delete only the unused music file");
        assert_eq!(bytes, b"unused music file".len() as u64);

        // Verify used files still exist
        assert!(chars_dir.join("1.gif").exists(), "Used sprite should remain");
        assert!(chars_dir.join("2.gif").exists(), "Used sprite should remain");
        // Verify unused file is gone
        assert!(!unused_dir.join("old_track.mp3").exists(), "Unused file should be deleted");
    }

    #[test]
    fn test_clear_unused_defaults_no_cases_clears_everything() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();

        // Create defaults/ with files but NO cases
        let defaults_dir = data_dir.join("defaults").join("sounds");
        fs::create_dir_all(&defaults_dir).unwrap();
        fs::write(defaults_dir.join("sound.mp3"), b"orphaned").unwrap();

        let (deleted, _) = clear_unused_defaults(data_dir).unwrap();
        assert_eq!(deleted, 1, "All files should be cleared when no cases reference them");
        assert!(!defaults_dir.join("sound.mp3").exists());
    }

    #[test]
    fn test_clear_unused_defaults_no_defaults_dir() {
        let dir = tempfile::tempdir().unwrap();
        let (deleted, bytes) = clear_unused_defaults(dir.path()).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(bytes, 0);
    }

    // --- Persistent index tests ---

    #[test]
    fn test_dedup_index_register_and_find() {
        let dir = tempfile::tempdir().unwrap();
        let index = DedupIndex::open(dir.path()).unwrap();

        // Register a file
        let hash = xxh3_64(b"test content");
        index.register("defaults/images/test.gif", 12, hash).unwrap();

        // Create a candidate with same content
        let candidate = dir.path().join("candidate.gif");
        fs::write(&candidate, b"test content").unwrap();
        let result = index.find_duplicate(&candidate, dir.path());
        assert!(result.is_some(), "Should find registered duplicate");
        assert_eq!(result.unwrap(), "defaults/images/test.gif");

        // Different content → no match
        let different = dir.path().join("different.gif");
        fs::write(&different, b"other content!").unwrap();
        let result = index.find_duplicate(&different, dir.path());
        assert!(result.is_none(), "Different content should not match");
    }

    #[test]
    fn test_dedup_index_unregister() {
        let dir = tempfile::tempdir().unwrap();
        let index = DedupIndex::open(dir.path()).unwrap();

        let hash = xxh3_64(b"removable");
        index.register("defaults/sounds/test.mp3", 9, hash).unwrap();

        // Verify it's findable
        let candidate = dir.path().join("candidate.mp3");
        fs::write(&candidate, b"removable").unwrap();
        assert!(index.find_duplicate(&candidate, dir.path()).is_some());

        // Unregister
        index.unregister("defaults/sounds/test.mp3").unwrap();

        // No longer findable
        assert!(index.find_duplicate(&candidate, dir.path()).is_none());
    }

    #[test]
    fn test_dedup_index_persistence() {
        let dir = tempfile::tempdir().unwrap();

        // Register in one instance
        {
            let index = DedupIndex::open(dir.path()).unwrap();
            let hash = xxh3_64(b"persistent data");
            index.register("defaults/music/song.mp3", 15, hash).unwrap();
        }

        // Re-open from same path — entries should survive
        {
            let index = DedupIndex::open(dir.path()).unwrap();
            let candidate = dir.path().join("candidate.mp3");
            fs::write(&candidate, b"persistent data").unwrap();
            let result = index.find_duplicate(&candidate, dir.path());
            assert!(result.is_some(), "Entries should persist across open/close");
            assert_eq!(result.unwrap(), "defaults/music/song.mp3");
        }
    }

    #[test]
    fn test_dedup_index_scan_skips_existing() {
        let dir = tempfile::tempdir().unwrap();

        // Create a file in defaults/
        let defaults_dir = dir.path().join("defaults");
        fs::create_dir_all(&defaults_dir).unwrap();
        fs::write(defaults_dir.join("file.gif"), b"content").unwrap();

        let index = DedupIndex::open(dir.path()).unwrap();

        // First scan registers 1 file
        let count1 = index.scan_and_register(dir.path(), "defaults").unwrap();
        assert_eq!(count1, 1);

        // Second scan skips it (already in db)
        let count2 = index.scan_and_register(dir.path(), "defaults").unwrap();
        assert_eq!(count2, 0, "Should not re-register existing files");
    }

    #[test]
    fn test_content_hash_deterministic() {
        // xxh3_64 is deterministic — same input always produces same hash
        let content = b"known test content for hash verification";
        let hash1 = xxh3_64(content);
        let hash2 = xxh3_64(content);
        assert_eq!(hash1, hash2, "Same content must produce same hash");
        assert_ne!(hash1, 0, "Hash should not be zero for non-empty content");

        // Different content → different hash
        let other = b"different content";
        let hash3 = xxh3_64(other);
        assert_ne!(hash1, hash3, "Different content should produce different hash");
    }

    #[test]
    fn test_dedup_index_populated_after_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();

        // Create defaults/ with a file
        let defaults_dir = data_dir.join("defaults").join("images");
        fs::create_dir_all(&defaults_dir).unwrap();
        fs::write(defaults_dir.join("sprite.gif"), b"sprite data").unwrap();

        // Create a case with assets/ (no overlap, just to trigger dedup to run scan_and_register)
        let case_dir = data_dir.join("case").join("42");
        let assets_dir = case_dir.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        fs::write(assets_dir.join("unique.gif"), b"unique data").unwrap();

        let mut asset_map = HashMap::new();
        asset_map.insert("http://example.com/unique.gif".into(), "assets/unique.gif".into());
        let manifest = super::super::manifest::CaseManifest {
            case_id: 42,
            title: "Test".into(), author: "A".into(), language: "en".into(),
            download_date: "2025-01-01".into(), format: "v6".into(),
            sequence: None,
            assets: super::super::manifest::AssetSummary {
                case_specific: 1, shared_defaults: 0,
                total_downloaded: 1, total_size_bytes: 11,
            },
            asset_map,
            failed_assets: vec![], has_plugins: false, has_case_config: false,
        };
        write_manifest(&manifest, &case_dir).unwrap();

        // Run dedup — this calls scan_and_register internally
        let _ = dedup_case_assets(42, data_dir).unwrap();

        // Open a FRESH index and verify the defaults/ file was registered
        let fresh_index = DedupIndex::open(data_dir).unwrap();
        let candidate = dir.path().join("candidate.gif");
        fs::write(&candidate, b"sprite data").unwrap();
        let result = fresh_index.find_duplicate(&candidate, data_dir);
        assert!(result.is_some(), "Index should contain the defaults/ file after dedup ran");
        assert!(
            result.unwrap().contains("sprite.gif"),
            "Should find the defaults/images/sprite.gif entry"
        );
    }
}
