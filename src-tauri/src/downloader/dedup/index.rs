use std::fs;
use std::path::Path;

use redb::{Database, MultimapTableDefinition, TableDefinition};

use super::helpers::{hash_file, normalize_ext};
use crate::downloader::paths::normalize_path;

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
        let relative_path = normalize_path(relative_path);
        let ext = Path::new(&*relative_path)
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
            hash_table.insert(&*relative_path, (size, hash))
                .map_err(|e| format!("Failed to insert hash: {}", e))?;

            let mut lookup_table = txn.open_multimap_table(PATHS_BY_SIZE_EXT)
                .map_err(|e| format!("Failed to open lookup table: {}", e))?;
            lookup_table.insert(&*size_ext_key, &*relative_path)
                .map_err(|e| format!("Failed to insert lookup: {}", e))?;
        }
        txn.commit().map_err(|e| format!("Failed to commit: {}", e))?;
        Ok(())
    }

    /// Remove a file from the index.
    /// Reads old entry to reconstruct the size+ext key, then removes from both tables.
    pub fn unregister(&self, relative_path: &str) -> Result<(), String> {
        let relative_path = normalize_path(relative_path);
        // Read old entry to get size for the secondary key
        let old_entry = {
            let txn = self.db.begin_read()
                .map_err(|e| format!("Failed to begin read: {}", e))?;
            match txn.open_table(HASH_BY_PATH) {
                Ok(table) => table.get(&*relative_path)
                    .map_err(|e| format!("Failed to read: {}", e))?
                    .map(|v| v.value()),
                Err(_) => None,
            }
        };

        if let Some((size, _hash)) = old_entry {
            let ext = Path::new(&*relative_path)
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
                let _ = hash_table.remove(&*relative_path);

                let mut lookup_table = txn.open_multimap_table(PATHS_BY_SIZE_EXT)
                    .map_err(|e| format!("Failed to open lookup table: {}", e))?;
                let _ = lookup_table.remove(&*size_ext_key, &*relative_path);
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
    pub fn find_duplicate(&self, file_path: &Path, base_dir: &Path) -> Option<String> {
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
                    // Verify the candidate file still exists on disk (defense against stale entries)
                    if base_dir.join(&candidate_path).is_file() {
                        return Some(candidate_path);
                    }
                }
            }
        }
        None
    }

    /// Find any file in the index matching the given size, extension, and content hash.
    /// Prefers defaults/ matches over case/ matches. Returns None if no match.
    /// If `exclude` is provided, skip that exact path (used to avoid self-matches).
    pub fn find_by_hash(&self, size: u64, ext: &str, hash: u64, exclude: Option<&str>) -> Option<String> {
        let size_ext_key = format!("{}:{}", size, normalize_ext(ext));
        let txn = self.db.begin_read().ok()?;
        let lookup_table = txn.open_multimap_table(PATHS_BY_SIZE_EXT).ok()?;
        let candidates = lookup_table.get(&*size_ext_key).ok()?;
        let hash_table = txn.open_table(HASH_BY_PATH).ok()?;

        let mut best: Option<String> = None;
        for candidate in candidates.flatten() {
            let path = candidate.value().to_string();
            if let Some(excl) = exclude {
                if path == excl {
                    continue;
                }
            }
            if let Ok(Some(entry)) = hash_table.get(&*path) {
                let (_, candidate_hash) = entry.value();
                if candidate_hash == hash {
                    if path.starts_with("defaults/") {
                        return Some(path); // Prefer defaults/ — return immediately
                    }
                    if best.is_none() {
                        best = Some(path);
                    }
                }
            }
        }
        best
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
                    Ok(r) => normalize_path(&r.to_string_lossy()),
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

    /// Scan all case asset directories and register files not already indexed.
    /// Keys: `case/{id}/assets/{filename}`. Used for migrating pre-existing downloads.
    pub fn scan_and_register_cases(&self, data_dir: &Path) -> Result<usize, String> {
        let cases_dir = data_dir.join("case");
        if !cases_dir.is_dir() {
            return Ok(0);
        }
        let mut count = 0;
        let entries = fs::read_dir(&cases_dir)
            .map_err(|e| format!("Failed to read case directory: {}", e))?;
        for entry in entries.flatten() {
            let case_dir = entry.path();
            if !case_dir.is_dir() {
                continue;
            }
            let case_id = match case_dir.file_name().and_then(|n| n.to_str()) {
                Some(name) => match name.parse::<u32>() {
                    Ok(id) => id,
                    Err(_) => continue,
                },
                None => continue,
            };
            let assets_dir = case_dir.join("assets");
            if !assets_dir.is_dir() {
                continue;
            }
            let files = match fs::read_dir(&assets_dir) {
                Ok(rd) => rd,
                Err(_) => continue,
            };
            for file_entry in files.flatten() {
                let path = file_entry.path();
                if !path.is_file() {
                    continue;
                }
                let filename = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let reg_key = format!("case/{}/assets/{}", case_id, filename);

                // Check if already registered
                let already_exists = {
                    let txn = self.db.begin_read()
                        .map_err(|e| format!("Failed to begin read: {}", e))?;
                    match txn.open_table(HASH_BY_PATH) {
                        Ok(table) => table.get(&*reg_key)
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
                self.register(&reg_key, size, hash)?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Remove all entries whose path starts with the given prefix.
    /// Uses B-tree sorted range scan. Returns count of removed entries.
    pub fn unregister_prefix(&self, prefix: &str) -> Result<usize, String> {
        // Collect entries to remove (read transaction)
        let to_remove: Vec<(String, u64, String)> = {
            let txn = self.db.begin_read()
                .map_err(|e| format!("Failed to begin read: {}", e))?;
            let table = match txn.open_table(HASH_BY_PATH) {
                Ok(t) => t,
                Err(_) => return Ok(0),
            };
            let mut entries = Vec::new();
            if let Ok(range) = table.range::<&str>(prefix..) {
                for item in range.flatten() {
                    let path = item.0.value().to_string();
                    if !path.starts_with(prefix) {
                        break; // Sorted, no more matches
                    }
                    let (size, _hash) = item.1.value();
                    let ext = Path::new(&path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(normalize_ext)
                        .unwrap_or_default();
                    entries.push((path, size, ext));
                }
            }
            entries
        };

        if to_remove.is_empty() {
            return Ok(0);
        }

        // Remove in a write transaction
        let txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write: {}", e))?;
        {
            let mut hash_table = txn.open_table(HASH_BY_PATH)
                .map_err(|e| format!("Failed to open hash table: {}", e))?;
            let mut lookup_table = txn.open_multimap_table(PATHS_BY_SIZE_EXT)
                .map_err(|e| format!("Failed to open lookup table: {}", e))?;
            for (path, size, ext) in &to_remove {
                let _ = hash_table.remove(&**path);
                let size_ext_key = format!("{}:{}", size, ext);
                let _ = lookup_table.remove(&*size_ext_key, &**path);
            }
        }
        txn.commit().map_err(|e| format!("Failed to commit: {}", e))?;
        Ok(to_remove.len())
    }

    /// Query all case asset entries from the index.
    /// Returns `(path, case_id, filename, size, hash)` for all `case/*/assets/*` entries.
    pub fn query_case_assets(&self) -> Result<Vec<(u32, String, u64, String, u64)>, String> {
        let txn = self.db.begin_read()
            .map_err(|e| format!("Failed to begin read: {}", e))?;
        let table = match txn.open_table(HASH_BY_PATH) {
            Ok(t) => t,
            Err(_) => return Ok(Vec::new()),
        };

        let mut result = Vec::new();
        if let Ok(range) = table.range::<&str>("case/"..) {
            for item in range.flatten() {
                let path = item.0.value().to_string();
                if !path.starts_with("case/") {
                    break;
                }
                // Parse "case/{id}/assets/{filename}"
                let parts: Vec<&str> = path.splitn(4, '/').collect();
                if parts.len() < 4 || parts[2] != "assets" {
                    continue;
                }
                let case_id = match parts[1].parse::<u32>() {
                    Ok(id) => id,
                    Err(_) => continue,
                };
                let filename = parts[3].to_string();
                let (size, hash) = item.1.value();
                let ext = Path::new(&filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(normalize_ext)
                    .unwrap_or_default();
                result.push((case_id, filename, size, ext, hash));
            }
        }
        Ok(result)
    }
}
