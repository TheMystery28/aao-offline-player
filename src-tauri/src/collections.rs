//! Collections feature: group downloaded cases into named collections.
//!
//! Collections are stored as `collections.json` in the data directory.
//! Each collection has a unique ID, title, and ordered list of items
//! (cases or sequences).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

/// A named collection of cases and/or sequences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub title: String,
    pub items: Vec<CollectionItem>,
    pub created_date: String,
}

/// An item in a collection: either a single case or a named sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CollectionItem {
    #[serde(rename = "case")]
    Case { case_id: u32 },
    #[serde(rename = "sequence")]
    Sequence { title: String },
}

/// Top-level wrapper for the collections.json file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionsData {
    pub collections: Vec<Collection>,
}

/// Returns the path to `collections.json` in the given data directory.
pub fn collections_path(data_dir: &Path) -> PathBuf {
    data_dir.join("collections.json")
}

/// Load collections from disk.
///
/// If the `collections.json` file is missing or contains invalid JSON,
/// an empty `CollectionsData` object is returned.
pub fn load_collections(data_dir: &Path) -> CollectionsData {
    let path = collections_path(data_dir);
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(data) => {
                serde_json::from_str(&data).unwrap_or(CollectionsData {
                    collections: Vec::new(),
                })
            }
            Err(_) => CollectionsData {
                collections: Vec::new(),
            },
        }
    } else {
        CollectionsData {
            collections: Vec::new(),
        }
    }
}

/// Persist all collections to disk as pretty-printed JSON.
///
/// # Errors
///
/// Returns an `AppError` if serialization or file writing fails.
pub fn save_collections(data_dir: &Path, data: &CollectionsData) -> Result<(), crate::error::AppError> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| format!("Failed to serialize collections: {}", e))?;
    fs::write(collections_path(data_dir), json)
        .map_err(|e| format!("Failed to write collections.json: {}", e))?;
    Ok(())
}

/// Generate a unique random ID using UUID v4.
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

/// Generate an ISO 8601 UTC timestamp string from the current time.
pub fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    crate::utils::format_timestamp(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_case_item() {
        let item = CollectionItem::Case { case_id: 42 };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"type\":\"case\""));
        assert!(json.contains("\"case_id\":42"));
    }

    #[test]
    fn test_serialize_sequence_item() {
        let item = CollectionItem::Sequence {
            title: "My Sequence".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"type\":\"sequence\""));
        assert!(json.contains("\"My Sequence\""));
    }

    #[test]
    fn test_deserialize_case_item() {
        let json = r#"{"type":"case","case_id":99}"#;
        let item: CollectionItem = serde_json::from_str(json).unwrap();
        match item {
            CollectionItem::Case { case_id } => assert_eq!(case_id, 99),
            _ => panic!("Expected Case variant"),
        }
    }

    #[test]
    fn test_deserialize_sequence_item() {
        let json = r#"{"type":"sequence","title":"Test Seq"}"#;
        let item: CollectionItem = serde_json::from_str(json).unwrap();
        match item {
            CollectionItem::Sequence { title } => assert_eq!(title, "Test Seq"),
            _ => panic!("Expected Sequence variant"),
        }
    }

    #[test]
    fn test_collection_roundtrip() {
        let collection = Collection {
            id: "test-123".to_string(),
            title: "My Collection".to_string(),
            items: vec![
                CollectionItem::Case { case_id: 1 },
                CollectionItem::Sequence {
                    title: "Seq A".to_string(),
                },
                CollectionItem::Case { case_id: 2 },
            ],
            created_date: "2025-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string_pretty(&collection).unwrap();
        let parsed: Collection = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-123");
        assert_eq!(parsed.title, "My Collection");
        assert_eq!(parsed.items.len(), 3);
    }

    #[test]
    fn test_load_save_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let data = CollectionsData {
            collections: vec![
                Collection {
                    id: "c1".to_string(),
                    title: "First".to_string(),
                    items: vec![CollectionItem::Case { case_id: 10 }],
                    created_date: "2025-06-15T12:00:00Z".to_string(),
                },
                Collection {
                    id: "c2".to_string(),
                    title: "Second".to_string(),
                    items: vec![CollectionItem::Sequence {
                        title: "Seq".to_string(),
                    }],
                    created_date: "2025-06-15T13:00:00Z".to_string(),
                },
            ],
        };
        save_collections(dir.path(), &data).unwrap();
        let loaded = load_collections(dir.path());
        assert_eq!(loaded.collections.len(), 2);
        assert_eq!(loaded.collections[0].id, "c1");
        assert_eq!(loaded.collections[0].title, "First");
        assert_eq!(loaded.collections[1].id, "c2");
        assert_eq!(loaded.collections[1].items.len(), 1);
    }

    #[test]
    fn test_load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let data = load_collections(dir.path());
        assert!(data.collections.is_empty());
    }

    #[test]
    fn test_load_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("collections.json"), "not valid json!").unwrap();
        let data = load_collections(dir.path());
        assert!(data.collections.is_empty());
    }

    #[test]
    fn test_generate_id_unique() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        // UUID v4: 32 hex digits + 4 hyphens = 36 chars, 5 groups (8-4-4-4-12)
        assert_eq!(id.len(), 36, "UUID should be 36 chars, got: {id}");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "UUID should only contain hex digits and hyphens, got: {id}"
        );
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5, "UUID should have 5 groups, got: {id}");
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    #[test]
    fn test_now_iso8601_format() {
        let ts = now_iso8601();
        // Should match YYYY-MM-DDTHH:MM:SSZ
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
    }

    #[test]
    fn test_collections_path() {
        let p = collections_path(Path::new("/some/dir"));
        assert_eq!(p, PathBuf::from("/some/dir/collections.json"));
    }
}
