//! Core downloading logic and data structures.
//!
//! This module coordinates the process of fetching case data from the AAO
//! website, identifying all necessary assets (images, music, sounds), and
//! managing the parallel download and storage of those assets.

pub mod asset_downloader;
pub mod asset_paths;
pub mod asset_resolver;
pub mod case_fetcher;
pub mod dedup;
pub mod manifest;
pub mod paths;
pub mod pipeline;
pub mod vfs;

use serde::{Deserialize, Serialize};

pub const AAONLINE_BASE: &str = "https://aaonline.fr";

/// Typed error for the downloader module.
///
/// Eliminates `.map_err(|e| format!(...))` boilerplate. Callers outside the
/// module convert to `String` via `.to_string()` or the `From` impl.
#[derive(thiserror::Error, Debug)]
pub enum DownloaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    Db(#[from] redb::Error),
    #[error("Database storage error: {0}")]
    DbStorage(#[from] redb::StorageError),
    #[error("Database table error: {0}")]
    DbTable(#[from] redb::TableError),
    #[error("Database commit error: {0}")]
    DbCommit(#[from] redb::CommitError),
    #[error("Database transaction error: {0}")]
    DbTransaction(#[from] redb::TransactionError),
    #[error("{0}")]
    Other(String),
}

impl From<DownloaderError> for String {
    fn from(e: DownloaderError) -> String {
        e.to_string()
    }
}

impl From<String> for DownloaderError {
    fn from(s: String) -> Self {
        DownloaderError::Other(s)
    }
}

/// Site paths extracted from AAO's bridge.js.php cfg variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitePaths {
    pub picture_dir: String,
    pub icon_subdir: String,
    pub talking_subdir: String,
    pub still_subdir: String,
    pub startup_subdir: String,
    pub evidence_subdir: String,
    pub bg_subdir: String,
    pub defaultplaces_subdir: String,
    pub popups_subdir: String,
    pub locks_subdir: String,
    pub music_dir: String,
    pub sounds_dir: String,
    pub voices_dir: String,
}

impl SitePaths {
    fn picture_path(&self, subdir: &str) -> String {
        format!("{}{}", self.picture_dir, subdir)
    }

    pub fn icon_path(&self) -> String { self.picture_path(&self.icon_subdir) }
    pub fn talking_path(&self) -> String { self.picture_path(&self.talking_subdir) }
    pub fn still_path(&self) -> String { self.picture_path(&self.still_subdir) }
    pub fn startup_path(&self) -> String { self.picture_path(&self.startup_subdir) }
    pub fn evidence_path(&self) -> String { self.picture_path(&self.evidence_subdir) }
    pub fn bg_path(&self) -> String { self.picture_path(&self.bg_subdir) }
    pub fn popups_path(&self) -> String { self.picture_path(&self.popups_subdir) }
    pub fn locks_path(&self) -> String { self.picture_path(&self.locks_subdir) }
    pub fn defaultplaces_path(&self) -> String { self.picture_path(&self.defaultplaces_subdir) }

    pub fn music_path(&self) -> &str { &self.music_dir }
    pub fn sounds_path(&self) -> &str { &self.sounds_dir }
    pub fn voices_path(&self) -> &str { &self.voices_dir }
}

/// Case metadata as represented in the AAO trial information.
///
/// This includes basic details about the case, as well as optional
/// sequence information for multi-part cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseInfo {
    pub id: u32,
    pub title: String,
    pub author: String,
    pub language: String,
    pub last_edit_date: u64,
    pub format: String,
    pub sequence: Option<serde_json::Value>,
}

/// A reference to a single asset that needs to be downloaded.
///
/// Assets are classified into "case-specific" (downloaded into the case's
/// `assets/` folder) and "shared/default" (downloaded into the global
/// `defaults/` cache).
#[derive(Debug, Clone, Serialize)]
pub struct AssetRef {
    /// Original URL from the trial data.
    pub url: String,
    /// Type of asset (e.g. "background", "music").
    pub asset_type: String,
    /// Whether this is a default AAO asset.
    pub is_default: bool,
    /// For internal (non-external) assets: the path under engine/ where the player expects
    /// to find this file (e.g. "defaults/images/backgrounds/AA4/Court.jpg").
    ///
    /// Empty for external assets (they get hashed filenames in case/assets/).
    pub local_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Regression: CaseInfo must serialize with all fields including sequence.
    #[test]
    fn test_case_info_serializes_with_sequence() {
        let info = CaseInfo {
            id: 69063,
            title: "Investigation".to_string(),
            author: "TestAuthor".to_string(),
            language: "en".to_string(),
            last_edit_date: 1700000000,
            format: "v6".to_string(),
            sequence: Some(json!({
                "title": "A Turnabout Called Justice",
                "list": [
                    {"id": 69063, "title": "Investigation"},
                    {"id": 69064, "title": "Trial"}
                ]
            })),
        };
        let json_val = serde_json::to_value(&info).unwrap();
        assert_eq!(json_val["id"], 69063);
        assert_eq!(json_val["title"], "Investigation");
        assert_eq!(json_val["sequence"]["title"], "A Turnabout Called Justice");
        assert_eq!(json_val["sequence"]["list"].as_array().unwrap().len(), 2);
        assert_eq!(json_val["sequence"]["list"][0]["id"], 69063);
        assert_eq!(json_val["sequence"]["list"][1]["id"], 69064);
    }

    /// Regression: CaseInfo without sequence must serialize with sequence: null.
    #[test]
    fn test_case_info_serializes_without_sequence() {
        let info = CaseInfo {
            id: 12345,
            title: "Standalone Case".to_string(),
            author: "Author".to_string(),
            language: "fr".to_string(),
            last_edit_date: 0,
            format: "v5".to_string(),
            sequence: None,
        };
        let json_val = serde_json::to_value(&info).unwrap();
        assert_eq!(json_val["id"], 12345);
        assert!(json_val["sequence"].is_null());
    }

    /// Regression: CaseInfo roundtrip through serialize → deserialize.
    #[test]
    fn test_case_info_roundtrip() {
        let info = CaseInfo {
            id: 96366,
            title: "Part 1".to_string(),
            author: "Someone".to_string(),
            language: "en".to_string(),
            last_edit_date: 1700000000,
            format: "v6".to_string(),
            sequence: Some(json!({
                "title": "Long Sequence",
                "list": [{"id": 96366, "title": "Part 1"}, {"id": 96367, "title": "Part 2"}]
            })),
        };
        let json_str = serde_json::to_string(&info).unwrap();
        let restored: CaseInfo = serde_json::from_str(&json_str).unwrap();
        assert_eq!(restored.id, info.id);
        assert_eq!(restored.title, info.title);
        assert_eq!(restored.sequence, info.sequence);
    }

    /// Regression: CaseManifest with sequence roundtrips through write/read.
    #[test]
    fn test_manifest_sequence_roundtrip() {
        let manifest = manifest::CaseManifest {
            case_id: 69063,
            title: "Investigation".to_string(),
            author: "TestAuthor".to_string(),
            language: "en".to_string(),
            download_date: "2025-01-15T12:00:00Z".to_string(),
            format: "v6".to_string(),
            sequence: Some(json!({
                "title": "A Turnabout Called Justice",
                "list": [{"id": 69063, "title": "Investigation"}, {"id": 69064, "title": "Trial"}]
            })),
            assets: manifest::AssetSummary {
                case_specific: 10,
                shared_defaults: 5,
                total_downloaded: 15,
                total_size_bytes: 50000,
            },
            asset_map: std::collections::HashMap::new(),
            failed_assets: vec![],
            has_plugins: false,
            has_case_config: false,
        };

        let dir = tempfile::tempdir().unwrap();
        manifest::write_manifest(&manifest, dir.path()).unwrap();
        let loaded = manifest::read_manifest(dir.path()).unwrap();

        assert_eq!(loaded.sequence, manifest.sequence);
        assert_eq!(loaded.case_id, 69063);
        let seq = loaded.sequence.unwrap();
        assert_eq!(seq["title"], "A Turnabout Called Justice");
        assert_eq!(seq["list"].as_array().unwrap().len(), 2);
    }

    // --- New tests ---

    /// CaseInfo with sequence containing an empty list serializes correctly.
    #[test]
    fn test_case_info_with_empty_sequence_list() {
        let info = CaseInfo {
            id: 11111,
            title: "Empty Seq".to_string(),
            author: "Author".to_string(),
            language: "en".to_string(),
            last_edit_date: 0,
            format: "v6".to_string(),
            sequence: Some(json!({
                "title": "Empty Sequence",
                "list": []
            })),
        };
        let json_val = serde_json::to_value(&info).unwrap();
        assert_eq!(json_val["sequence"]["title"], "Empty Sequence");
        assert!(json_val["sequence"]["list"].as_array().unwrap().is_empty());
        // Roundtrip
        let json_str = serde_json::to_string(&info).unwrap();
        let restored: CaseInfo = serde_json::from_str(&json_str).unwrap();
        assert_eq!(restored.sequence, info.sequence);
    }

    /// CaseInfo with a single-part sequence serializes correctly.
    #[test]
    fn test_case_info_with_single_part_sequence() {
        let info = CaseInfo {
            id: 22222,
            title: "Solo Part".to_string(),
            author: "Writer".to_string(),
            language: "fr".to_string(),
            last_edit_date: 1700000000,
            format: "v6".to_string(),
            sequence: Some(json!({
                "title": "One-Part Sequence",
                "list": [{"id": 22222, "title": "Solo Part"}]
            })),
        };
        let json_val = serde_json::to_value(&info).unwrap();
        assert_eq!(json_val["sequence"]["list"].as_array().unwrap().len(), 1);
        assert_eq!(json_val["sequence"]["list"][0]["id"], 22222);
    }

    /// AssetRef serializes with all expected fields.
    #[test]
    fn test_asset_ref_serialization() {
        let asset = AssetRef {
            url: "https://example.com/image.png".to_string(),
            asset_type: "background".to_string(),
            is_default: true,
            local_path: "defaults/images/backgrounds/Court.jpg".to_string(),
        };
        let json_val = serde_json::to_value(&asset).unwrap();
        assert_eq!(json_val["url"], "https://example.com/image.png");
        assert_eq!(json_val["asset_type"], "background");
        assert_eq!(json_val["is_default"], true);
        assert_eq!(json_val["local_path"], "defaults/images/backgrounds/Court.jpg");
    }

    /// Every field in CaseInfo must be present in the serialized JSON.
    #[test]
    fn test_case_info_all_fields_present_in_json() {
        let info = CaseInfo {
            id: 33333,
            title: "All Fields".to_string(),
            author: "Completionist".to_string(),
            language: "de".to_string(),
            last_edit_date: 9876543210,
            format: "Def6".to_string(),
            sequence: Some(json!({"title": "Seq", "list": [{"id": 33333, "title": "P1"}]})),
        };
        let json_val = serde_json::to_value(&info).unwrap();
        let obj = json_val.as_object().unwrap();

        let expected_keys = ["id", "title", "author", "language", "last_edit_date", "format", "sequence"];
        for key in &expected_keys {
            assert!(
                obj.contains_key(*key),
                "CaseInfo JSON missing expected field: {}",
                key
            );
        }
        assert_eq!(obj.len(), expected_keys.len(), "CaseInfo should have exactly {} fields", expected_keys.len());
    }

    // --- SitePaths regression tests ---
    // These tests lock the exact output of every SitePaths path method BEFORE the
    // private-helper refactor. They must continue to pass unchanged after the refactor.

    fn make_site_paths() -> SitePaths {
        SitePaths {
            picture_dir: "BASE/".to_string(),
            icon_subdir: "icons/".to_string(),
            talking_subdir: "talking/".to_string(),
            still_subdir: "still/".to_string(),
            startup_subdir: "startup/".to_string(),
            evidence_subdir: "evidence/".to_string(),
            bg_subdir: "bg/".to_string(),
            defaultplaces_subdir: "places/".to_string(),
            popups_subdir: "popups/".to_string(),
            locks_subdir: "locks/".to_string(),
            music_dir: "MUSIC/".to_string(),
            sounds_dir: "SOUNDS/".to_string(),
            voices_dir: "VOICES/".to_string(),
        }
    }

    #[test]
    fn test_site_paths_icon_path() {
        assert_eq!(make_site_paths().icon_path(), "BASE/icons/");
    }

    #[test]
    fn test_site_paths_talking_path() {
        assert_eq!(make_site_paths().talking_path(), "BASE/talking/");
    }

    #[test]
    fn test_site_paths_still_path() {
        assert_eq!(make_site_paths().still_path(), "BASE/still/");
    }

    #[test]
    fn test_site_paths_startup_path() {
        assert_eq!(make_site_paths().startup_path(), "BASE/startup/");
    }

    #[test]
    fn test_site_paths_evidence_path() {
        assert_eq!(make_site_paths().evidence_path(), "BASE/evidence/");
    }

    #[test]
    fn test_site_paths_bg_path() {
        assert_eq!(make_site_paths().bg_path(), "BASE/bg/");
    }

    #[test]
    fn test_site_paths_defaultplaces_path() {
        assert_eq!(make_site_paths().defaultplaces_path(), "BASE/places/");
    }

    #[test]
    fn test_site_paths_popups_path() {
        assert_eq!(make_site_paths().popups_path(), "BASE/popups/");
    }

    #[test]
    fn test_site_paths_locks_path() {
        assert_eq!(make_site_paths().locks_path(), "BASE/locks/");
    }

    /// Each path method returns picture_dir concatenated with its own subdir, not any other.
    #[test]
    fn test_site_paths_each_method_uses_correct_subdir() {
        // Use distinct values so a mix-up in subdir selection is immediately visible.
        let s = SitePaths {
            picture_dir: "P/".to_string(),
            icon_subdir: "A/".to_string(),
            talking_subdir: "B/".to_string(),
            still_subdir: "C/".to_string(),
            startup_subdir: "D/".to_string(),
            evidence_subdir: "E/".to_string(),
            bg_subdir: "F/".to_string(),
            defaultplaces_subdir: "G/".to_string(),
            popups_subdir: "H/".to_string(),
            locks_subdir: "I/".to_string(),
            music_dir: "M/".to_string(),
            sounds_dir: "S/".to_string(),
            voices_dir: "V/".to_string(),
        };
        assert_eq!(s.icon_path(),          "P/A/");
        assert_eq!(s.talking_path(),       "P/B/");
        assert_eq!(s.still_path(),         "P/C/");
        assert_eq!(s.startup_path(),       "P/D/");
        assert_eq!(s.evidence_path(),      "P/E/");
        assert_eq!(s.bg_path(),            "P/F/");
        assert_eq!(s.defaultplaces_path(), "P/G/");
        assert_eq!(s.popups_path(),        "P/H/");
        assert_eq!(s.locks_path(),         "P/I/");
    }

    #[test]
    fn test_site_paths_audio_methods() {
        let s = make_site_paths();
        assert_eq!(s.music_path(),  "MUSIC/");
        assert_eq!(s.sounds_path(), "SOUNDS/");
        assert_eq!(s.voices_path(), "VOICES/");
    }
}
