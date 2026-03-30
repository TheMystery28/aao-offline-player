use std::fs;
use std::io;
use std::path::Path;

use serde::{Serialize, Deserialize};
use serde_json::Value;

use crate::downloader::manifest::CaseManifest;

/// Result of importing a .aaocase ZIP file.
/// Contains the manifest and optionally any game saves that were included.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub manifest: CaseManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saves: Option<Value>,
    /// Number of default assets referenced in the manifest but missing from disk.
    /// Non-zero means the .aaocase was exported without defaults (old format).
    #[serde(default)]
    pub missing_defaults: usize,
    /// For batch imports: all manifests imported (empty for single imports).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_manifests: Vec<CaseManifest>,
    /// For batch imports: errors for individual cases that failed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch_errors: Vec<String>,
    /// Bytes saved by dedup during import (skipped duplicate files).
    #[serde(default)]
    pub dedup_saved_bytes: u64,
}

/// Internal return type for import functions.
/// Consolidates the manifest + optional collection + dedup stats.
#[derive(Debug)]
pub(crate) struct ImportOutput {
    pub(crate) manifest: CaseManifest,
    pub(crate) collection: Option<crate::collections::Collection>,
    pub(crate) dedup_saved_bytes: u64,
}

/// Metadata extracted from trial_information.
pub(super) struct ImportedCaseInfo {
    pub(super) id: u32,
    pub(super) title: String,
    pub(super) author: String,
    pub(super) language: String,
    pub(super) format: String,
    pub(super) last_edit_date: u64,
    pub(super) sequence: Option<Value>,
}

/// A match found by the plugin duplicate checker.
/// Returns list of matches with filename and location.
#[derive(Debug, Clone, Serialize)]
pub struct DuplicateMatch {
    pub filename: String,
    pub location: String,
}

/// Result of importing a .aaosave ZIP file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSaveResult {
    pub saves: serde_json::Value,
    pub metadata: serde_json::Value,
    pub plugins_installed: Vec<u32>,
}

/// Read, modify, and write the global plugin manifest in one step.
/// The closure receives a mutable reference to the parsed JSON.
/// The manifest is written back after the closure returns Ok(()).
pub(super) fn with_global_manifest<F>(engine_dir: &Path, f: F) -> Result<(), String>
where F: FnOnce(&mut serde_json::Value) -> Result<(), String>
{
    let manifest_path = engine_dir.join("plugins").join("manifest.json");
    if !manifest_path.exists() {
        return Err("No global plugin manifest".to_string());
    }
    let text = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let mut val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;
    f(&mut val)?;
    fs::write(&manifest_path, serde_json::to_string_pretty(&val).unwrap())
        .map_err(|e| format!("Failed to write manifest: {}", e))?;
    Ok(())
}

/// Add or remove a string value from a JSON array.
pub(super) fn toggle_in_string_array(arr: &mut Vec<serde_json::Value>, value: &str, add: bool) {
    if add {
        if !arr.iter().any(|s| s.as_str() == Some(value)) {
            arr.push(serde_json::Value::String(value.to_string()));
        }
    } else {
        arr.retain(|s| s.as_str() != Some(value));
    }
}

/// Read a text file from a ZIP archive.
pub(super) fn read_zip_text(archive: &mut zip::ZipArchive<fs::File>, name: &str) -> Result<String, String> {
    let mut entry = archive.by_name(name)
        .map_err(|_| format!("ZIP does not contain '{}'. Is this a valid .aaocase file?", name))?;
    let mut contents = String::new();
    io::Read::read_to_string(&mut entry, &mut contents)
        .map_err(|e| format!("Failed to read '{}' from ZIP: {}", name, e))?;
    Ok(contents)
}

/// Recursively add a directory to a ZIP archive.
pub(super) fn add_dir_to_zip_recursive(
    zip: &mut zip::ZipWriter<fs::File>,
    dir: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {}", prefix, e))? {
        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
        let path = entry.path();
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
        if path.is_dir() {
            add_dir_to_zip_recursive(zip, &path, &name, options)?;
        } else if path.is_file() {
            let path = crate::downloader::vfs::resolve_path(&path, dir, dir);
            let data = fs::read(&path)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
            zip.start_file(&name, options)
                .map_err(|e| format!("Failed to add {} to ZIP: {}", name, e))?;
            io::Write::write_all(zip, &data)
                .map_err(|e| format!("Failed to write {} to ZIP: {}", name, e))?;
        }
    }
    Ok(())
}
