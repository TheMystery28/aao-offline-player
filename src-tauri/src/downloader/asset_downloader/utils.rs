use xxhash_rust::xxh3::xxh3_64;

/// Check if a file already exists locally and should be skipped.
/// Returns `Some(size)` if the file exists and has content (skip download),
/// or `None` if the file is missing or empty (proceed with download).
pub fn check_skip_existing(save_dir: &std::path::Path, relative_path: &str) -> Option<u64> {
    let file_path = save_dir.join(relative_path);
    if file_path.exists() {
        let size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            // Reject imgur's "removed" placeholder — treat as if file doesn't exist
            // so the downloader re-attempts (and fails properly via content hash check).
            if size == 503 {
                if let Ok(bytes) = std::fs::read(&file_path) {
                    if xxhash_rust::xxh3::xxh3_64(&bytes) == 0x38da9bd2e10a4bc8 {
                        let _ = std::fs::remove_file(&file_path); // Clean up the placeholder
                        return None;
                    }
                }
            }
            return Some(size);
        }
    }
    None
}

pub(super) fn generate_filename(url: &str) -> String {
    let hash = xxh3_64(url.as_bytes());
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
