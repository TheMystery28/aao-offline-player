use std::fs;
use std::path::Path;

use redb::{Database, MultimapTableDefinition, ReadableDatabase, TableDefinition};

use super::helpers::{hash_file, normalize_ext};
use crate::downloader::DownloaderError;
use crate::downloader::paths::normalize_path;

/// Primary index: relative_path → (file_size, xxh3_hash)
const HASH_BY_PATH: TableDefinition<&str, (u64, u64)> =
    TableDefinition::new("hash_by_path");

/// Secondary lookup: "{hash}" → relative_path (multimap)
/// Keyed by content hash for direct O(log n) lookups without per-candidate comparison.
const PATHS_BY_HASH: MultimapTableDefinition<&str, &str> =
    MultimapTableDefinition::new("paths_by_hash");

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
    /// Inserts into both hash_by_path and paths_by_hash in one transaction.
    pub fn register(&self, relative_path: &str, size: u64, hash: u64) -> Result<(), String> {
        let relative_path = normalize_path(relative_path);
        let hash_key = format!("{}", hash);

        let txn = self.db.begin_write()
            .map_err(|e| format!("Failed to begin write: {}", e))?;
        {
            let mut hash_table = txn.open_table(HASH_BY_PATH)
                .map_err(|e| format!("Failed to open hash table: {}", e))?;
            hash_table.insert(&*relative_path, (size, hash))
                .map_err(|e| format!("Failed to insert hash: {}", e))?;

            let mut lookup_table = txn.open_multimap_table(PATHS_BY_HASH)
                .map_err(|e| format!("Failed to open lookup table: {}", e))?;
            lookup_table.insert(&*hash_key, &*relative_path)
                .map_err(|e| format!("Failed to insert lookup: {}", e))?;
        }
        txn.commit().map_err(|e| format!("Failed to commit: {}", e))?;
        Ok(())
    }

    /// Register multiple files in the index in a single transaction.
    /// More efficient than calling register() per file.
    pub fn register_batch(&self, entries: &[(&str, u64, u64)]) -> Result<(), DownloaderError> {
        if entries.is_empty() {
            return Ok(());
        }
        let txn = self.db.begin_write()?;
        {
            let mut hash_table = txn.open_table(HASH_BY_PATH)?;
            let mut lookup_table = txn.open_multimap_table(PATHS_BY_HASH)?;
            for &(path, size, hash) in entries {
                let normalized = normalize_path(path);
                let hash_key = format!("{}", hash);
                hash_table.insert(&*normalized, (size, hash))?;
                lookup_table.insert(&*hash_key, &*normalized)?;
            }
        }
        txn.commit()?;
        Ok(())
    }

    /// Remove a file from the index.
    /// Reads old entry to get hash for the secondary key, then removes from both tables.
    pub fn unregister(&self, relative_path: &str) -> Result<(), DownloaderError> {
        let relative_path = normalize_path(relative_path);
        let old_entry = {
            let txn = self.db.begin_read()?;
            match txn.open_table(HASH_BY_PATH) {
                Ok(table) => table.get(&*relative_path)?.map(|v| v.value()),
                Err(_) => None,
            }
        };

        if let Some((_size, hash)) = old_entry {
            let hash_key = format!("{}", hash);
            let txn = self.db.begin_write()?;
            {
                let mut hash_table = txn.open_table(HASH_BY_PATH)?;
                let _ = hash_table.remove(&*relative_path);
                let mut lookup_table = txn.open_multimap_table(PATHS_BY_HASH)?;
                let _ = lookup_table.remove(&*hash_key, &*relative_path);
            }
            txn.commit()?;
        }
        Ok(())
    }

    /// Find any file in the index matching the given content hash.
    /// Prefers defaults/ matches over case/ matches. Returns None if no match.
    /// If `exclude` is provided, skip that exact path (used to avoid self-matches).
    pub fn find_by_hash(&self, hash: u64, exclude: Option<&str>) -> Option<String> {
        let hash_key = format!("{}", hash);
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_multimap_table(PATHS_BY_HASH).ok()?;
        let candidates = table.get(&*hash_key).ok()?;

        let mut best: Option<String> = None;
        for candidate in candidates.flatten() {
            let path = candidate.value().to_string();
            if let Some(excl) = exclude {
                if path == excl {
                    continue;
                }
            }
            if path.starts_with("defaults/") {
                return Some(path); // Prefer defaults/ — return immediately
            }
            if best.is_none() {
                best = Some(path);
            }
        }
        best
    }

    /// Scan a directory and register all files not already in the db.
    /// Used on first run or when the index is out of date.
    /// Returns the count of newly registered files.
    pub fn scan_and_register(&self, data_dir: &Path, prefix: &str) -> Result<usize, DownloaderError> {
        let dir = data_dir.join(prefix);
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut pending: Vec<(String, u64, u64)> = Vec::new();
        Self::collect_unregistered(&dir, data_dir, &self.db, &mut pending)?;
        let count = pending.len();
        if !pending.is_empty() {
            let refs: Vec<(&str, u64, u64)> = pending.iter()
                .map(|(p, s, h)| (p.as_str(), *s, *h))
                .collect();
            self.register_batch(&refs)?;
        }
        Ok(count)
    }

    fn collect_unregistered(
        dir: &Path,
        base_dir: &Path,
        db: &Database,
        pending: &mut Vec<(String, u64, u64)>,
    ) -> Result<(), DownloaderError> {
        let read_dir = match fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => return Ok(()),
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::collect_unregistered(&path, base_dir, db, pending)?;
            } else if path.is_file() {
                let relative = match path.strip_prefix(base_dir) {
                    Ok(r) => normalize_path(&r.to_string_lossy()),
                    Err(_) => continue,
                };

                let already_exists = {
                    let txn = db.begin_read()?;
                    match txn.open_table(HASH_BY_PATH) {
                        Ok(table) => table.get(&*relative)?.is_some(),
                        Err(_) => false,
                    }
                };

                if already_exists {
                    continue;
                }

                // If the file is a VFS pointer, hash the target (not the 50-byte pointer text)
                let (actual_path, actual_size) = if let Some(target_rel) = crate::downloader::vfs::read_vfs_pointer(&path) {
                    let target = base_dir.join(&target_rel);
                    match target.metadata() {
                        Ok(m) if m.len() > 0 => (target, m.len()),
                        _ => continue, // Broken pointer — skip
                    }
                } else {
                    let size = match path.metadata() {
                        Ok(m) => m.len(),
                        Err(_) => continue,
                    };
                    (path.clone(), size)
                };
                let hash = match hash_file(&actual_path) {
                    Ok(h) => h,
                    Err(_) => continue,
                };
                pending.push((relative.to_string(), actual_size, hash));
            }
        }
        Ok(())
    }

    /// Scan all case asset directories and register files not already indexed.
    /// Keys: `case/{id}/assets/{filename}`. Used for migrating pre-existing downloads.
    pub fn scan_and_register_cases(&self, data_dir: &Path) -> Result<usize, DownloaderError> {
        let cases_dir = data_dir.join("case");
        if !cases_dir.is_dir() {
            return Ok(0);
        }
        let mut count = 0;
        let entries = fs::read_dir(&cases_dir)?;
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
                let reg_key = crate::downloader::asset_paths::case_asset(case_id, &filename);

                let already_exists = {
                    let txn = self.db.begin_read()?;
                    match txn.open_table(HASH_BY_PATH) {
                        Ok(table) => table.get(&*reg_key)?.is_some(),
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
                // register() returns String error — convert at boundary
                self.register(&reg_key, size, hash)
                    .map_err(|e| DownloaderError::Other(e))?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Remove all entries whose path starts with the given prefix.
    /// Uses B-tree sorted range scan. Returns count of removed entries.
    pub fn unregister_prefix(&self, prefix: &str) -> Result<usize, String> {
        // Collect entries to remove (read transaction)
        let to_remove: Vec<(String, u64)> = {
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
                    let (_size, hash) = item.1.value();
                    entries.push((path, hash));
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
            let mut lookup_table = txn.open_multimap_table(PATHS_BY_HASH)
                .map_err(|e| format!("Failed to open lookup table: {}", e))?;
            for (path, hash) in &to_remove {
                let _ = hash_table.remove(&**path);
                let hash_key = format!("{}", hash);
                let _ = lookup_table.remove(&*hash_key, &**path);
            }
        }
        txn.commit().map_err(|e| format!("Failed to commit: {}", e))?;
        Ok(to_remove.len())
    }

    /// Query all case asset entries from the index.
    /// Returns `(case_id, filename, size, ext, hash)` for all `case/*/assets/*` entries.
    pub fn query_case_assets(&self) -> Result<Vec<(u32, String, u64, String, u64)>, DownloaderError> {
        let txn = self.db.begin_read()?;
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
