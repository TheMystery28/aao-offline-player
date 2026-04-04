//! Commands for importing cases and sequences from external files.
//!
//! This module handles importing cases from `.aaocase` ZIP files, 
//! `aaoffline` download directories, and game saves from `.aaosave` files.

use std::fs;
use std::path::PathBuf;
use tauri::ipc::Channel;
use tauri::State;

use crate::app_state::AppPaths;
use crate::downloader::asset_downloader::DownloadEvent;
use crate::error::AppError;
use crate::importer;

/// Import a case or sequence from a file or directory.
///
/// Supported formats:
/// - `.aaocase` ZIP file: Contains case data, assets, and optionally saves.
/// - `aaoffline` directory: A directory containing `index.html` and assets.
/// - Batch import: A directory containing multiple `aaoffline` case subfolders.
///
/// # Arguments
///
/// * `source_path` - Path to the source file or directory.
/// * `on_event` - Progress channel for the UI.
///
/// # Returns
///
/// An `ImportResult` containing metadata about the imported case(s).
#[tauri::command]
pub async fn import_case(
    app: tauri::AppHandle,
    paths: State<'_, AppPaths>,
    source_path: String,
    on_event: Channel<DownloadEvent>,
) -> Result<importer::ImportResult, AppError> {
    let data_dir = paths.data_dir.clone();

    // On Android, the file picker returns content:// URIs which aren't regular filesystem paths.
    // Copy the file to a temp location using Tauri's fs plugin (handles content URIs).
    let (path, _temp_file) = if source_path.starts_with("content://") {
        use tauri_plugin_fs::FsExt;
        log::debug!("Android content URI detected: {}", source_path);

        let _ = on_event.send(DownloadEvent::Started { total: 1 });
        let _ = on_event.send(DownloadEvent::Progress {
            completed: 0, total: 1,
            current_url: "Reading file...".to_string(),
            bytes_downloaded: 0, elapsed_ms: 0,
        });

        let url = reqwest::Url::parse(&source_path)
            .map_err(|e| format!("Failed to parse content URI: {}", e))?;
        let file_path = tauri_plugin_fs::FilePath::from(url);
        let content = app.fs().read(file_path)
            .map_err(|e| format!("Failed to read from content URI: {}", e))?;

        let temp_path = data_dir.join("_import_temp.aaocase");
        fs::write(&temp_path, &content)
            .map_err(|e| format!("Failed to write temp import file: {}", e))?;

        log::debug!("Copied {} bytes from content URI to {}", content.len(), temp_path.display());
        (temp_path.clone(), Some(temp_path)) // _temp_file keeps the path for cleanup
    } else {
        let p = PathBuf::from(&source_path);
        if !p.exists() {
            return Err(format!("Path not found: {}", source_path).into());
        }
        (p, None)
    };

    let progress_cb = |completed: usize, total: usize| {
        let _ = on_event.send(DownloadEvent::Progress {
            completed,
            total,
            current_url: format!("{}/{}", completed, total),
            bytes_downloaded: 0, elapsed_ms: 0,
        });
    };

    let mut import_result = if path.is_dir() {
        let has_subfolders = !importer::find_aaoffline_subfolders(&path).is_empty();
        if has_subfolders {
            // Parent folder with case subfolders (may also have root index.html — batch handles both)
            let _ = on_event.send(DownloadEvent::Started { total: 0 });
            let case_progress_cb = |current: usize, total: usize, name: &str| {
                let _ = on_event.send(DownloadEvent::SequenceProgress {
                    current_part: current,
                    total_parts: total,
                    part_title: format!("Importing: {}", name),
                });
            };
            importer::import_aaoffline_batch(&path, &data_dir, Some(&case_progress_cb), Some(&progress_cb))?
        } else if path.join("index.html").exists() {
            // Single aaoffline case folder (no subfolders)
            let _ = on_event.send(DownloadEvent::Started { total: 0 });
            let output = importer::import_aaoffline(&path, &data_dir, Some(&progress_cb))?;
            importer::ImportResult { manifest: output.manifest, saves: None, missing_defaults: 0, batch_manifests: Vec::new(), batch_errors: Vec::new(), dedup_saved_bytes: output.dedup_saved_bytes }
        } else {
            return Err(format!(
                "No index.html found in {} and no subfolders with cases found either.",
                path.display()
            ).into());
        }
    } else if path.is_file() {
        let _ = on_event.send(DownloadEvent::Started { total: 0 });
        match importer::import_aaocase_zip(&path, &data_dir, Some(&progress_cb)) {
            Ok(result) => result,
            Err(aaocase_err) => {
                // Not a .aaocase — try extracting as zipped aaoffline folder
                let temp_dir = data_dir.join("_import_temp_unzip");
                if temp_dir.exists() { let _ = fs::remove_dir_all(&temp_dir); }
                let extract_result = (|| -> Result<importer::ImportResult, crate::error::AppError> {
                    fs::create_dir_all(&temp_dir)
                        .map_err(|e| format!("Failed to create temp dir: {}", e))?;
                    let file = fs::File::open(&path)
                        .map_err(|e| format!("Failed to open ZIP: {}", e))?;
                    let mut archive = zip::ZipArchive::new(file)
                        .map_err(|e| format!("Not a valid ZIP: {}", e))?;
                    archive.extract(&temp_dir)
                        .map_err(|e| format!("ZIP extraction failed: {}", e))?;

                    let root = temp_dir.as_path();
                    let has_subfolders = !importer::find_aaoffline_subfolders(root).is_empty();
                    if has_subfolders {
                        importer::import_aaoffline_batch(root, &data_dir, None, Some(&progress_cb))
                    } else if root.join("index.html").exists() {
                        let output = importer::import_aaoffline(root, &data_dir, Some(&progress_cb))?;
                        Ok(importer::ImportResult {
                            manifest: output.manifest, saves: None,
                            missing_defaults: 0, batch_manifests: Vec::new(),
                            batch_errors: Vec::new(), dedup_saved_bytes: output.dedup_saved_bytes,
                        })
                    } else {
                        Err("No index.html found in ZIP".to_string().into())
                    }
                })();
                let _ = fs::remove_dir_all(&temp_dir); // Always clean up
                match extract_result {
                    Ok(result) => result,
                    Err(zip_err) => {
                        return Err(format!(
                            "Could not import: not a valid .aaocase ({}), and not a valid aaoffline ZIP ({})",
                            aaocase_err, zip_err
                        ).into());
                    }
                }
            }
        }
    } else {
        return Err(format!("Not a file or directory: {}", source_path).into());
    };

    // Clean up temp file if we created one
    if let Some(temp) = _temp_file {
        let _ = fs::remove_file(&temp);
    }

    // Re-read manifests for accurate post-dedup sizes (dedup runs inside import functions)
    if !import_result.batch_manifests.is_empty() {
        for m in import_result.batch_manifests.iter_mut() {
            let case_dir = data_dir.join("case").join(m.case_id.to_string());
            if let Ok(updated) = crate::downloader::manifest::read_manifest(&case_dir) {
                *m = updated;
            }
        }
    } else {
        let case_dir = data_dir.join("case").join(import_result.manifest.case_id.to_string());
        if let Ok(updated) = crate::downloader::manifest::read_manifest(&case_dir) {
            import_result.manifest = updated;
        }
    }

    // Sum up totals (now using post-dedup manifest values)
    let (total_downloaded, total_bytes) = if !import_result.batch_manifests.is_empty() {
        import_result.batch_manifests.iter().fold((0usize, 0u64), |(d, b), m| {
            (d + m.assets.total_downloaded, b + m.assets.total_size_bytes)
        })
    } else {
        (import_result.manifest.assets.total_downloaded, import_result.manifest.assets.total_size_bytes)
    };

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: total_downloaded,
        failed: 0,
        total_bytes,
        dedup_saved_bytes: import_result.dedup_saved_bytes,
    });

    log::info!(
        "Imported case {} \"{}\" ({} assets, {} bytes{})",
        import_result.manifest.case_id,
        import_result.manifest.title,
        import_result.manifest.assets.total_downloaded,
        import_result.manifest.assets.total_size_bytes,
        if import_result.saves.is_some() { ", with saves" } else { "" }
    );

    Ok(import_result)
}

/// Download a file from a URL to a temp path for subsequent import.
///
/// Streams the response to disk to avoid loading large files into RAM.
/// Returns the temp file path. The caller is responsible for cleanup.
#[tauri::command]
pub async fn download_from_url(
    paths: State<'_, AppPaths>,
    url: String,
    on_event: Channel<DownloadEvent>,
) -> Result<String, AppError> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    let client = &paths.http_client;
    let response = client.get(&url).send().await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()).into());
    }

    // Total size from Content-Length (if available)
    let total_size = response.content_length().unwrap_or(0);
    let _ = on_event.send(DownloadEvent::Started { total: 1 });

    // Detect extension from URL path first
    let url_ext = url.rsplit('/').next()
        .and_then(|name| name.split('?').next()) // strip query params
        .and_then(|name| name.rsplit('.').next())
        .and_then(|e| {
            let lower = e.to_lowercase();
            match lower.as_str() {
                "aaocase" | "aaoplug" | "aaosave" | "zip" => Some(lower),
                _ => None,
            }
        });

    // Download to a temp file (no extension yet — we'll rename after detection)
    let temp_raw = paths.data_dir.join("_download_temp_raw");

    let mut file = tokio::fs::File::create(&temp_raw).await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let start = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk).await
            .map_err(|e| format!("File write error: {}", e))?;
        downloaded += chunk.len() as u64;
        let _ = on_event.send(DownloadEvent::Progress {
            completed: if total_size > 0 { ((downloaded * 100) / total_size) as usize } else { 0 },
            total: 100,
            current_url: url.clone(),
            bytes_downloaded: downloaded,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }
    drop(file); // flush

    // If URL didn't have a known extension, detect from file content
    let ext = if let Some(e) = url_ext {
        e
    } else {
        detect_file_type(&temp_raw).ok_or_else(|| {
            let _ = std::fs::remove_file(&temp_raw);
            AppError::from("Could not determine file type. Expected .aaocase, .aaoplug, .aaosave, or .zip".to_string())
        })?
    };

    let temp_path = paths.data_dir.join(format!("_download_temp.{}", ext));
    std::fs::rename(&temp_raw, &temp_path)
        .map_err(|e| format!("Failed to rename temp file: {}", e))?;

    Ok(temp_path.to_string_lossy().to_string())
}

/// Detect AAO file type by peeking at content.
/// Returns "aaocase", "aaoplug", "aaosave", or "zip" — or None if unrecognizable.
fn detect_file_type(path: &std::path::Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 4 { return None; }

    // ZIP magic bytes: PK\x03\x04
    if data[0..4] == [0x50, 0x4B, 0x03, 0x04] {
        // It's a ZIP — peek inside to classify
        if let Ok(file) = std::fs::File::open(path) {
            if let Ok(mut archive) = zip::ZipArchive::new(file) {
                let names: Vec<String> = (0..archive.len())
                    .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
                    .collect();
                if names.iter().any(|n| n == "manifest.json" || n.ends_with("/manifest.json")) {
                    return Some("aaocase".to_string());
                }
                if names.iter().any(|n| n == "saves.json" || n.ends_with("/saves.json")) {
                    // Could be .aaosave (saves.json at root without manifest.json)
                    if !names.iter().any(|n| n.ends_with("trial_data.json")) {
                        return Some("aaosave".to_string());
                    }
                }
                if names.iter().any(|n| n == "index.html" || n.ends_with("/index.html")) {
                    return Some("zip".to_string()); // zipped aaoffline
                }
                // Check for .aaoplug (has plugin JS files)
                if names.iter().any(|n| n.ends_with(".js")) {
                    return Some("aaoplug".to_string());
                }
                return Some("zip".to_string()); // generic ZIP, let import_case handle it
            }
        }
        return Some("zip".to_string());
    }

    // Not a ZIP — check if it's JSON (possible raw save data)
    let text = String::from_utf8_lossy(&data);
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Some("aaosave".to_string());
    }

    None // unrecognizable
}

/// Delete a temp file created by download_from_url.
#[tauri::command]
pub async fn delete_temp_file(path: String) -> Result<(), AppError> {
    let p = std::path::PathBuf::from(&path);
    // Safety: only delete files in data dir that start with _download_temp
    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
        if name.starts_with("_download_temp") {
            let _ = std::fs::remove_file(&p);
        }
    }
    Ok(())
}

/// Import game saves from a `.aaosave` file.
#[tauri::command]
pub async fn import_save(
    paths: State<'_, AppPaths>,
    source_path: String,
) -> Result<importer::ImportSaveResult, AppError> {
    let data_dir = paths.data_dir.clone();
    let path = std::path::PathBuf::from(&source_path);
    Ok(tokio::task::spawn_blocking(move || {
        importer::import_aaosave(&path, &data_dir)
    }).await.map_err(|e| format!("Import task failed: {}", e))??)
}
