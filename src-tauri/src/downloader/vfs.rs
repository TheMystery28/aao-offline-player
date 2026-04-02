//! VFS (Virtual File System) pointer utilities for asset deduplication.
//!
//! When two canonical default assets have identical content (e.g., talking and still
//! sprites that are the same GIF), the dedup system keeps one physical copy and replaces
//! the other with a lightweight "VFS pointer" — a tiny text file containing:
//!   `AAO_VFS_ALIAS:defaults/images/chars/Apollo/3.gif`
//!
//! The server's `resolve_path` follows these pointers transparently so the engine
//! can request either path and get the correct file.

use std::fs;
use std::path::{Path, PathBuf};

use super::paths::normalize_path;

const VFS_PREFIX: &str = "AAO_VFS_ALIAS:";
const MAX_RESOLVE_DEPTH: u8 = 5;

/// Check if a file is a VFS pointer.
/// Returns the target relative path (forward-slash normalized) if it is, None otherwise.
/// Files >= 256 bytes are assumed to be real assets (a GIF header alone is larger).
pub fn read_vfs_pointer(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if meta.len() >= 256 {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    content.strip_prefix(VFS_PREFIX).map(|s| s.to_string())
}

/// Write a VFS pointer file that redirects to the target path.
/// Target is always stored with forward slashes via `normalize_path`.
pub fn write_vfs_pointer(pointer_path: &Path, target_relative: &str) -> std::io::Result<()> {
    if let Some(parent) = pointer_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let normalized = normalize_path(target_relative);
    fs::write(pointer_path, format!("{}{}", VFS_PREFIX, normalized))
}

/// Check if an asset file truly exists on disk — follows VFS pointers.
/// A VFS pointer whose target is missing counts as "not exists".
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

/// Resolve a file path, following VFS pointers with a depth limit.
/// Returns the real physical file path. If the file is not a pointer, returns it as-is.
/// Stops after `MAX_RESOLVE_DEPTH` hops to prevent infinite loops.
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
}
