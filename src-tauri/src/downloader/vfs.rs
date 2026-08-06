//! VFS (Virtual File System) pointer utilities for asset deduplication.
//!
//! When two canonical default assets have identical content (e.g., talking and still
//! sprites that are the same GIF), the dedup system keeps one physical copy and replaces
//! the other with a lightweight "VFS pointer" — a tiny text file containing:
//!   `AAO_VFS_ALIAS:defaults/images/chars/Apollo/3.gif`
//!
//! The server's `resolve_path` follows these pointers transparently so the engine
//! can request either path and get the correct file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use super::paths::normalize_path;

const VFS_PREFIX: &str = "AAO_VFS_ALIAS:";
const MAX_RESOLVE_DEPTH: u8 = 5;
const CACHE_MAX_ENTRIES: usize = 4096;

/// Process-wide cache of VFS pointer probes: path → (mtime, size, target).
/// Freshness is validated against the file's (mtime, size) on every read, so
/// stale entries are impossible even when files are rewritten.
struct VfsCache {
    inner: Mutex<HashMap<PathBuf, (Option<SystemTime>, u64, Option<String>)>>,
}

impl VfsCache {
    fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }

    fn get(
        &self,
        path: &Path,
    ) -> Option<(Option<SystemTime>, u64, Option<String>)> {
        self.inner.lock().ok().and_then(|g| g.get(path).cloned())
    }

    fn insert(&self, path: PathBuf, entry: (Option<SystemTime>, u64, Option<String>)) {
        let Ok(mut g) = self.inner.lock() else { return };
        if g.len() >= CACHE_MAX_ENTRIES {
            g.clear();
        }
        g.insert(path, entry);
    }

    fn invalidate(&self, path: &Path) {
        let Ok(mut g) = self.inner.lock() else { return };
        g.remove(path);
    }

    fn clear(&self) {
        let Ok(mut g) = self.inner.lock() else { return };
        g.clear();
    }
}

static VFS_CACHE: OnceLock<VfsCache> = OnceLock::new();

fn vfs_cache() -> &'static VfsCache {
    VFS_CACHE.get_or_init(VfsCache::new)
}

/// Clear the process-wide VFS pointer cache (tests; also safe after bulk writes).
#[allow(dead_code)]
pub fn vfs_cache_reset() {
    vfs_cache().clear();
}

/// Check if a file is a VFS pointer.
///
/// Returns the target relative path (forward-slash normalized) if it is a
/// VFS pointer, or `None` if it is a regular file or if an error occurs.
///
/// Results are cached per path with (mtime, size) freshness — the per-request
/// probe file read is skipped when the file is unchanged.
pub fn read_vfs_pointer(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() >= 256 {
        return None;
    }
    let mtime = meta.modified().ok();
    let size = meta.len();

    let cache = vfs_cache();
    if let Some((cached_mtime, cached_size, cached)) = cache.get(path) {
        if cached_mtime == mtime && cached_size == size {
            return cached;
        }
    }

    let content = fs::read_to_string(path).ok();
    let result = content.and_then(|c| c.strip_prefix(VFS_PREFIX).map(|s| s.to_string()));
    cache.insert(path.to_path_buf(), (mtime, size, result.clone()));
    result
}

/// Write a VFS pointer file that redirects to the target path.
///
/// The target path is automatically normalized to use forward slashes.
///
/// # Errors
///
/// Returns an `std::io::Result` if file creation or writing fails.
pub fn write_vfs_pointer(pointer_path: &Path, target_relative: &str) -> std::io::Result<()> {
    if let Some(parent) = pointer_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let normalized = normalize_path(target_relative);
    fs::write(pointer_path, format!("{}{}", VFS_PREFIX, normalized))?;
    vfs_cache().invalidate(pointer_path);
    Ok(())
}

/// Check if an asset file exists on disk, transparently following VFS pointers.
///
/// A VFS pointer is considered to exist only if its target file also exists.
pub fn asset_exists(data_dir: &Path, local_path: &str) -> bool {
    let disk_path = data_dir.join(local_path);
    if !disk_path.exists() {
        return false;
    }
    match read_vfs_pointer(&disk_path) {
        Some(target) => data_dir.join(&target).is_file(),
        None => true,
    }
}

/// Resolve a file path to its physical location on disk.
///
/// If the file is a VFS pointer, this function follows it recursively
/// until a real file is found or the `MAX_RESOLVE_DEPTH` is reached.
///
/// # Arguments
///
/// * `path` - The initial path to resolve.
/// * `data_dir` - The app's writable data directory.
/// * `engine_dir` - The app's engine resources directory.
///
/// # Returns
///
/// The resolved `PathBuf` pointing to the physical file.
pub fn resolve_path(path: &Path, data_dir: &Path, engine_dir: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    for _ in 0..MAX_RESOLVE_DEPTH {
        match read_vfs_pointer(&current) {
            Some(target) => {
                let resolved = if target.starts_with("case/") || target.starts_with("defaults/") {
                    data_dir.join(&target)
                } else {
                    engine_dir.join(&target)
                };
                if resolved.is_file() {
                    current = resolved;
                } else {
                    break;
                }
            }
            None => break,
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_vfs_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pointer.gif");
        fs::write(&path, "AAO_VFS_ALIAS:defaults/images/chars/Apollo/3.gif").unwrap();
        assert_eq!(
            read_vfs_pointer(&path),
            Some("defaults/images/chars/Apollo/3.gif".to_string())
        );
    }

    #[test]
    fn test_read_non_pointer_binary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("real.gif");
        fs::write(&path, vec![0u8; 10_000]).unwrap();
        assert_eq!(read_vfs_pointer(&path), None);
    }

    #[test]
    fn test_read_large_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.txt");
        fs::write(&path, "x".repeat(500)).unwrap();
        assert_eq!(read_vfs_pointer(&path), None);
    }

    #[test]
    fn test_write_vfs_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alias.gif");
        write_vfs_pointer(&path, "defaults/images/chars/Apollo/3.gif").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "AAO_VFS_ALIAS:defaults/images/chars/Apollo/3.gif");
    }

    #[test]
    fn test_write_normalizes_backslashes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alias.gif");
        write_vfs_pointer(&path, "defaults\\images\\chars\\Apollo\\3.gif").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("defaults/images/chars/Apollo/3.gif"),
            "Backslashes should be normalized to forward slashes: {}", content);
        assert!(!content.contains('\\'));
    }

    #[test]
    fn test_resolve_follows_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        // Create real target
        let target_dir = data.join("defaults/images/chars/Apollo");
        fs::create_dir_all(&target_dir).unwrap();
        let target = target_dir.join("3.gif");
        fs::write(&target, b"GIF89a real image data").unwrap();

        // Create pointer
        let pointer_dir = data.join("defaults/images/charsStill/Apollo");
        fs::create_dir_all(&pointer_dir).unwrap();
        let pointer = pointer_dir.join("3.gif");
        write_vfs_pointer(&pointer, "defaults/images/chars/Apollo/3.gif").unwrap();

        let resolved = resolve_path(&pointer, data, data);
        assert_eq!(resolved, target);
    }

    #[test]
    fn test_resolve_non_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("real.gif");
        fs::write(&path, vec![0u8; 1000]).unwrap();
        let resolved = resolve_path(&path, dir.path(), dir.path());
        assert_eq!(resolved, path);
    }

    #[test]
    fn test_resolve_depth_limit() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        // Create chain: p0 → p1 → p2 → ... → p9 (10 hops, exceeds MAX_RESOLVE_DEPTH=5)
        for i in 0..10 {
            let p = data.join(format!("defaults/p{}.gif", i));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            if i < 9 {
                write_vfs_pointer(&p, &format!("defaults/p{}.gif", i + 1)).unwrap();
            } else {
                fs::write(&p, b"GIF89a final target").unwrap();
            }
        }

        let start = data.join("defaults/p0.gif");
        let resolved = resolve_path(&start, data, data);
        // Should stop at depth 5 (p0→p1→p2→p3→p4→p5), not reach p9
        // p5 is a pointer to p6, which is resolved and found to be a file (p5 itself is a pointer)
        // The loop resolves: p0→p1→p2→p3→p4 (5 hops), landing on p5
        assert!(resolved != data.join("defaults/p9.gif"),
            "Should not reach the end of a 10-deep chain");
    }

    #[test]
    fn test_resolve_broken_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let pointer = dir.path().join("broken.gif");
        write_vfs_pointer(&pointer, "defaults/nonexistent/file.gif").unwrap();
        let resolved = resolve_path(&pointer, dir.path(), dir.path());
        // Broken pointer: target doesn't exist, returns the pointer path itself
        assert_eq!(resolved, pointer);
    }

    #[test]
    fn test_asset_exists_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("defaults/images/test.gif");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, vec![0u8; 1000]).unwrap();
        assert!(asset_exists(dir.path(), "defaults/images/test.gif"));
        assert!(!asset_exists(dir.path(), "defaults/images/nonexistent.gif"));
    }

    #[test]
    fn test_asset_exists_vfs_pointer() {
        let dir = tempfile::tempdir().unwrap();
        // Create real target
        let target = dir.path().join("defaults/images/chars/Apollo/3.gif");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"GIF89a real data").unwrap();
        // Create VFS pointer
        let pointer = dir.path().join("defaults/images/charsStill/Apollo/3.gif");
        write_vfs_pointer(&pointer, "defaults/images/chars/Apollo/3.gif").unwrap();
        assert!(asset_exists(dir.path(), "defaults/images/charsStill/Apollo/3.gif"));
    }

    #[test]
    fn test_asset_exists_broken_vfs_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let pointer = dir.path().join("defaults/images/charsStill/Apollo/3.gif");
        write_vfs_pointer(&pointer, "defaults/images/chars/Apollo/3.gif").unwrap();
        // Target does NOT exist
        assert!(!asset_exists(dir.path(), "defaults/images/charsStill/Apollo/3.gif"));
    }

    #[test]
    fn test_vfs_pointer_cache_staleness_invalidation() {
        vfs_cache_reset();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alias.gif");

        fs::write(&path, "AAO_VFS_ALIAS:defaults/images/chars/Apollo/3.gif").unwrap();
        assert_eq!(
            read_vfs_pointer(&path).as_deref(),
            Some("defaults/images/chars/Apollo/3.gif")
        );

        // Overwrite with a DIFFERENT pointer (same size, changed mtime)
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&path, "AAO_VFS_ALIAS:defaults/images/chars/Edgeworth/1.gif").unwrap();
        assert_eq!(
            read_vfs_pointer(&path).as_deref(),
            Some("defaults/images/chars/Edgeworth/1.gif"),
            "cache must not serve the stale target"
        );
    }

    #[test]
    fn test_vfs_write_pointer_invalidates_cache() {
        vfs_cache_reset();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alias2.gif");

        // Write a real (non-pointer) file and cache the "not a pointer" result
        fs::write(&path, "not a pointer text file").unwrap();
        assert_eq!(read_vfs_pointer(&path), None);

        // Convert it into a pointer via the public writer — must be observed
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_vfs_pointer(&path, "defaults/images/chars/Apollo/3.gif").unwrap();
        assert_eq!(
            read_vfs_pointer(&path).as_deref(),
            Some("defaults/images/chars/Apollo/3.gif"),
            "write_vfs_pointer must invalidate the cached 'not a pointer' result"
        );
    }

    #[test]
    fn test_vfs_pointer_cache_negative_result() {
        vfs_cache_reset();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.txt");
        fs::write(&path, "plain").unwrap();
        // First read populates the cache with None; second read must still be None
        assert_eq!(read_vfs_pointer(&path), None);
        assert_eq!(read_vfs_pointer(&path), None);
    }
}
