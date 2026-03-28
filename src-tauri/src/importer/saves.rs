use std::fs;
use std::io;
use std::path::Path;

use crate::downloader::manifest::{read_manifest, write_manifest};
use crate::utils::format_timestamp;

use super::shared::*;

/// Export saves as a .aaosave ZIP file.
///
/// ZIP format:
/// ```text
/// saves.json           Save data (required)
/// metadata.json        Export metadata (required)
/// plugins/{case_id}/   Per-case plugins (optional)
/// case_config/{id}.json Per-case config (optional)
/// ```
pub fn export_aaosave(
    case_ids: &[u32],
    saves: &serde_json::Value,
    include_plugins: bool,
    dest_path: &Path,
    engine_dir: &Path,
) -> Result<u64, String> {
    let file = fs::File::create(dest_path)
        .map_err(|e| format!("Failed to create .aaosave file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Write saves.json
    let saves_bytes = serde_json::to_string_pretty(saves)
        .map_err(|e| format!("Failed to serialize saves: {}", e))?;
    zip.start_file("saves.json", options)
        .map_err(|e| format!("Failed to add saves.json to ZIP: {}", e))?;
    io::Write::write_all(&mut zip, saves_bytes.as_bytes())
        .map_err(|e| format!("Failed to write saves.json to ZIP: {}", e))?;

    // Build metadata.json
    let mut cases_meta = Vec::new();
    let mut has_plugins = false;
    for &case_id in case_ids {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        let title = if let Ok(manifest) = read_manifest(&case_dir) {
            if include_plugins && manifest.has_plugins {
                has_plugins = true;
            }
            manifest.title
        } else {
            format!("Case {}", case_id)
        };

        let save_count = saves
            .get(case_id.to_string())
            .and_then(|v| v.as_object())
            .map(|m| m.len())
            .unwrap_or(0);

        cases_meta.push(serde_json::json!({
            "id": case_id,
            "title": title,
            "save_count": save_count
        }));
    }

    let metadata = serde_json::json!({
        "version": 1,
        "export_date": format_timestamp(std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()),
        "cases": cases_meta,
        "has_plugins": has_plugins
    });

    let metadata_bytes = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    zip.start_file("metadata.json", options)
        .map_err(|e| format!("Failed to add metadata.json to ZIP: {}", e))?;
    io::Write::write_all(&mut zip, metadata_bytes.as_bytes())
        .map_err(|e| format!("Failed to write metadata.json to ZIP: {}", e))?;

    // Add plugins and case_config if requested
    if include_plugins {
        for &case_id in case_ids {
            let case_dir = engine_dir.join("case").join(case_id.to_string());
            let plugins_dir = case_dir.join("plugins");
            if plugins_dir.is_dir() {
                let prefix = format!("plugins/{}", case_id);
                add_dir_to_zip_recursive(&mut zip, &plugins_dir, &prefix, options)?;
            }

            let config_path = case_dir.join("case_config.json");
            if config_path.is_file() {
                let data = fs::read(&config_path)
                    .map_err(|e| format!("Failed to read case_config.json: {}", e))?;
                let zip_name = format!("case_config/{}.json", case_id);
                zip.start_file(&zip_name, options)
                    .map_err(|e| format!("Failed to add {} to ZIP: {}", zip_name, e))?;
                io::Write::write_all(&mut zip, &data)
                    .map_err(|e| format!("Failed to write {} to ZIP: {}", zip_name, e))?;
            }
        }
    }

    zip.finish()
        .map_err(|e| format!("Failed to finalize .aaosave ZIP: {}", e))?;

    let meta = fs::metadata(dest_path)
        .map_err(|e| format!("Failed to get .aaosave file size: {}", e))?;
    Ok(meta.len())
}

/// Import saves from a .aaosave ZIP file.
pub fn import_aaosave(
    zip_path: &Path,
    engine_dir: &Path,
) -> Result<ImportSaveResult, String> {
    let file = fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open .aaosave file: {}", e))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Invalid .aaosave file: {}", e))?;

    // Read saves.json (required)
    let saves_text = read_zip_text(&mut archive, "saves.json")
        .map_err(|_| "Invalid .aaosave: missing saves.json".to_string())?;
    let saves: serde_json::Value = serde_json::from_str(&saves_text)
        .map_err(|e| format!("Failed to parse saves.json: {}", e))?;

    // Read metadata.json (required)
    let metadata_text = read_zip_text(&mut archive, "metadata.json")
        .map_err(|_| "Invalid .aaosave: missing metadata.json".to_string())?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata_text)
        .map_err(|e| format!("Failed to parse metadata.json: {}", e))?;

    // Collect plugins/ and case_config/ entries
    let mut plugin_entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut config_entries: Vec<(String, Vec<u8>)> = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;
        let name = entry.name().to_string();
        if entry.is_dir() { continue; }

        if name.starts_with("plugins/") {
            let mut buf = Vec::new();
            io::Read::read_to_end(&mut entry, &mut buf)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
            plugin_entries.push((name, buf));
        } else if name.starts_with("case_config/") {
            let mut buf = Vec::new();
            io::Read::read_to_end(&mut entry, &mut buf)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
            config_entries.push((name, buf));
        }
    }

    // Extract plugins if present
    let mut plugins_installed = Vec::new();
    for (zip_name, data) in &plugin_entries {
        // zip_name = "plugins/{case_id}/manifest.json" etc.
        let parts: Vec<&str> = zip_name.splitn(3, '/').collect();
        if parts.len() < 3 { continue; }
        let case_id_str = parts[1];
        let case_id: u32 = match case_id_str.parse() {
            Ok(id) => id,
            Err(_) => continue,
        };

        let case_dir = engine_dir.join("case").join(case_id_str);
        if !case_dir.exists() { continue; }

        let dest = case_dir.join("plugins").join(parts[2]);
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&dest, data);

        if !plugins_installed.contains(&case_id) {
            plugins_installed.push(case_id);
        }
    }

    // Update case manifests for plugins
    for &case_id in &plugins_installed {
        let case_dir = engine_dir.join("case").join(case_id.to_string());
        if let Ok(mut manifest) = read_manifest(&case_dir) {
            manifest.has_plugins = true;
            let _ = write_manifest(&manifest, &case_dir);
        }
    }

    // Extract case_config entries
    for (zip_name, data) in &config_entries {
        // zip_name = "case_config/{case_id}.json"
        let filename = zip_name.trim_start_matches("case_config/");
        let case_id_str = filename.trim_end_matches(".json");
        let case_dir = engine_dir.join("case").join(case_id_str);
        if case_dir.exists() {
            let _ = fs::write(case_dir.join("case_config.json"), data);
        }
    }

    Ok(ImportSaveResult {
        saves,
        metadata,
        plugins_installed,
    })
}
