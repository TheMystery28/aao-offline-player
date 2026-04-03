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
        importer::import_aaocase_zip(&path, &data_dir, Some(&progress_cb))?
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
