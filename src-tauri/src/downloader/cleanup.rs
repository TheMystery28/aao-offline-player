//! Crash-leftover cleanup: downloads stream to `{path}.part` and rename on
//! success; a hard crash can leave `.part` strays behind. This pass removes
//! them at startup so they are never indexed as assets or counted by storage.

use std::fs;
use std::path::Path;

/// Delete every `*.part` file under `case/` and `defaults/`.
/// Returns the number of files removed.
pub fn remove_part_files(data_dir: &Path) -> usize {
    let mut removed = 0;
    for base in ["case", "defaults"] {
        let dir = data_dir.join(base);
        removed += walk_and_remove_parts(&dir);
    }
    removed
}

fn walk_and_remove_parts(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            removed += walk_and_remove_parts(&path);
        } else if path.is_file() {
            if path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".part"))
            {
                if fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    removed
}
