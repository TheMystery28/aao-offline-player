use std::fs;
use std::path::Path;

use serde_json::Value;
use xxhash_rust::xxh3::xxh3_64;

use crate::error::AppError;

/// Compute xxh3_64 hash of a file's contents.
pub fn hash_file(path: &Path) -> Result<u64, AppError> {
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
