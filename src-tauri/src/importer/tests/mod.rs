use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use crate::downloader::manifest::{AssetSummary, CaseManifest, write_manifest, read_manifest};

mod test_aaoffline;
mod test_case_export;
mod test_case_import;
mod test_plugins_case;
mod test_plugins_global;
mod test_plugins_utils;
mod test_saves;

/// Helper: create a .aaocase ZIP in a temp dir, returns the path.
pub(super) fn create_test_aaocase(dir: &Path, case_id: u32) -> PathBuf {
    use std::io::Write;

    let zip_path = dir.join(format!("test_{}.aaocase", case_id));
    let file = fs::File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // manifest.json
    let manifest = serde_json::json!({
        "case_id": case_id,
        "title": "ZIP Test Case",
        "author": "ZipTester",
        "language": "en",
        "download_date": "2025-01-01T00:00:00Z",
        "format": "Def6",
        "sequence": null,
        "assets": {
            "case_specific": 2,
            "shared_defaults": 0,
            "total_downloaded": 2,
            "total_size_bytes": 100
        },
        "asset_map": {
            "http://example.com/bg.png": "assets/bg.png",
            "http://example.com/music.mp3": "assets/music.mp3"
        },
        "failed_assets": []
    });
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes()).unwrap();

    // trial_info.json
    let info = serde_json::json!({
        "id": case_id,
        "title": "ZIP Test Case",
        "author": "ZipTester",
        "language": "en",
        "format": "Def6",
        "last_edit_date": 0,
        "sequence": null
    });
    zip.start_file("trial_info.json", options).unwrap();
    zip.write_all(serde_json::to_string_pretty(&info).unwrap().as_bytes()).unwrap();

    // trial_data.json
    let data = serde_json::json!({
        "frames": [0, {"id": 1}],
        "profiles": [0],
        "evidence": [0],
        "places": [0]
    });
    zip.start_file("trial_data.json", options).unwrap();
    zip.write_all(serde_json::to_string_pretty(&data).unwrap().as_bytes()).unwrap();

    // assets/
    zip.start_file("assets/bg.png", options).unwrap();
    zip.write_all(b"fake png data").unwrap();

    zip.start_file("assets/music.mp3", options).unwrap();
    zip.write_all(b"fake mp3 data").unwrap();

    zip.finish().unwrap();
    zip_path
}

/// Create a test case with optional sequence info.
pub(super) fn create_test_case(
    engine_dir: &Path,
    case_id: u32,
    title: &str,
    sequence: Option<(&str, usize)>,
) -> std::path::PathBuf {
    let case_dir = engine_dir.join("case").join(case_id.to_string());
    std::fs::create_dir_all(&case_dir).unwrap();
    let seq_val = sequence.map(|(t, i)| serde_json::json!({"title": t, "index": i}));
    let manifest = CaseManifest {
        case_id,
        title: title.to_string(),
        author: "Tester".to_string(),
        language: "en".to_string(),
        download_date: "2026-01-01".to_string(),
        format: "test".to_string(),
        sequence: seq_val,
        assets: AssetSummary {
            case_specific: 0,
            shared_defaults: 0,
            total_downloaded: 0,
            total_size_bytes: 0,
        },
        asset_map: std::collections::HashMap::new(),
        failed_assets: vec![],
        has_plugins: false,
        has_case_config: false,
    };
    write_manifest(&manifest, &case_dir).unwrap();
    case_dir
}

/// Shorthand: create a test case with default title and no sequence.
pub(super) fn create_test_case_for_save(engine_dir: &Path, case_id: u32) -> std::path::PathBuf {
    create_test_case(engine_dir, case_id, &format!("Test Case {}", case_id), None)
}
