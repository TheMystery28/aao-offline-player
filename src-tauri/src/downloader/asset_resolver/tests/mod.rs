use super::*;
use super::defaults::parse_js_object_assignment;
use crate::downloader::asset_downloader::DownloadedAsset;
use crate::downloader::SitePaths;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

mod test_helpers;
mod test_extractors;
mod test_rewriter;
mod test_defaults;

/// Dummy engine dir with no default_data.js -- startup sprites won't be generated.
pub(super) fn test_engine_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test_dummy_engine")
}

pub(super) fn test_site_paths() -> SitePaths {
    SitePaths {
        picture_dir: "Ressources/Images/".to_string(),
        icon_subdir: "persos/".to_string(),
        talking_subdir: "persos/".to_string(),
        still_subdir: "persos_static/".to_string(),
        startup_subdir: "persos_startup/".to_string(),
        evidence_subdir: "dossier/".to_string(),
        bg_subdir: "cinematiques/".to_string(),
        defaultplaces_subdir: "lieux/".to_string(),
        popups_subdir: "persos/Cour/".to_string(),
        locks_subdir: "persos/Cour/psyche_locks/".to_string(),
        music_dir: "Ressources/Musiques/".to_string(),
        sounds_dir: "Ressources/Sons/".to_string(),
        voices_dir: "Ressources/Voix/".to_string(),
    }
}

/// Helper: create a temp engine dir with given default_profiles_nb and default_profiles_startup.
pub(super) fn temp_engine_dir(profiles_nb_json: &str, profiles_startup_json: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let js_dir = dir.path().join("Javascript");
    std::fs::create_dir_all(&js_dir).unwrap();
    std::fs::write(
        js_dir.join("default_data.js"),
        format!(
            "var default_profiles_nb = {};\nvar default_profiles_startup = {};",
            profiles_nb_json, profiles_startup_json
        ),
    ).unwrap();
    dir
}
