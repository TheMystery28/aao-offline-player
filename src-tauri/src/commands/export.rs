use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::State;

use crate::app_state::AppState;
use crate::collections as coll;
use crate::downloader::asset_downloader::DownloadEvent;
use crate::importer;

/// Export a case as a .aaocase ZIP file.
/// If `saves` is provided, includes it as saves.json in the ZIP.
/// On Android, `dest_path` may be a content:// URI — exports to temp then copies.
#[tauri::command]
pub fn export_case(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    dest_path: String,
    saves: Option<serde_json::Value>,
    include_plugins: Option<bool>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    // On Android, dest_path is a content:// URI. Export to temp, then copy.
    let (export_path, content_uri) = if dest_path.starts_with("content://") {
        let temp = data_dir.join("_export_temp.aaocase");
        (temp, Some(dest_path.clone()))
    } else {
        (PathBuf::from(&dest_path), None)
    };

    let _ = on_event.send(DownloadEvent::Started { total: 0 });

    let progress_cb = |completed: usize, total: usize| {
        let _ = on_event.send(DownloadEvent::Progress {
            completed,
            total,
            current_url: format!("{}/{}", completed, total),
            bytes_downloaded: 0, elapsed_ms: 0,
        });
    };

    let include_plugins_flag = include_plugins.unwrap_or(true);
    let size = importer::export_aaocase(case_id, &data_dir, &export_path, Some(&progress_cb), saves.as_ref(), include_plugins_flag)?;

    // Copy temp file to content URI on Android
    if let Some(uri) = content_uri {
        use tauri_plugin_fs::FsExt;
        use std::io::Write;
        let data = fs::read(&export_path)
            .map_err(|e| format!("Failed to read temp export: {}", e))?;
        let dest_url = reqwest::Url::parse(&uri)
            .map_err(|e| format!("Failed to parse content URI: {}", e))?;
        let dest_fp = tauri_plugin_fs::FilePath::from(dest_url);
        let opts = tauri_plugin_fs::OpenOptions::new().write(true).create(true).clone();
        let mut file = app.fs().open(dest_fp, opts)
            .map_err(|e| format!("Failed to open content URI for writing: {}", e))?;
        file.write_all(&data)
            .map_err(|e| format!("Failed to write to content URI: {}", e))?;
        let _ = fs::remove_file(&export_path);
    }

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
    });

    debug_log!(
        "Exported case {} to {} ({} bytes)",
        case_id,
        dest_path,
        size
    );

    Ok(size)
}

/// Export an entire sequence as a single .aaocase ZIP file.
/// If `saves` is provided, includes it as saves.json in the ZIP.
/// On Android, `dest_path` may be a content:// URI — exports to temp then copies.
#[tauri::command]
pub fn export_sequence(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
    sequence_title: String,
    sequence_list: serde_json::Value,
    dest_path: String,
    saves: Option<serde_json::Value>,
    include_plugins: Option<bool>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    let (export_path, content_uri) = if dest_path.starts_with("content://") {
        let temp = data_dir.join("_export_temp.aaocase");
        (temp, Some(dest_path.clone()))
    } else {
        (PathBuf::from(&dest_path), None)
    };

    let _ = on_event.send(DownloadEvent::Started { total: 0 });

    let progress_cb = |completed: usize, total: usize| {
        let _ = on_event.send(DownloadEvent::Progress {
            completed,
            total,
            current_url: format!("{}/{}", completed, total),
            bytes_downloaded: 0, elapsed_ms: 0,
        });
    };

    let include_plugins_flag = include_plugins.unwrap_or(true);
    let size = importer::export_sequence(
        &case_ids,
        &sequence_title,
        &sequence_list,
        &data_dir,
        &export_path,
        Some(&progress_cb),
        saves.as_ref(),
        include_plugins_flag,
    )?;

    if let Some(uri) = content_uri {
        use tauri_plugin_fs::FsExt;
        use std::io::Write;
        let data = fs::read(&export_path)
            .map_err(|e| format!("Failed to read temp export: {}", e))?;
        let dest_url = reqwest::Url::parse(&uri)
            .map_err(|e| format!("Failed to parse content URI: {}", e))?;
        let dest_fp = tauri_plugin_fs::FilePath::from(dest_url);
        let opts = tauri_plugin_fs::OpenOptions::new().write(true).create(true).clone();
        let mut file = app.fs().open(dest_fp, opts)
            .map_err(|e| format!("Failed to open content URI for writing: {}", e))?;
        file.write_all(&data)
            .map_err(|e| format!("Failed to write to content URI: {}", e))?;
        let _ = fs::remove_file(&export_path);
    }

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
    });

    Ok(size)
}

/// Export a collection as a .aaocase ZIP file.
#[tauri::command]
pub fn export_collection(
    state: State<'_, Mutex<AppState>>,
    collection_id: String,
    dest_path: String,
    saves: Option<serde_json::Value>,
    include_plugins: Option<bool>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };

    let coll_data = coll::load_collections(&data_dir);
    let collection = coll_data.collections.iter()
        .find(|c| c.id == collection_id)
        .ok_or_else(|| format!("Collection {} not found", collection_id))?
        .clone();

    let export_path = PathBuf::from(&dest_path);
    let progress_cb = |completed: usize, total: usize| {
        let _ = on_event.send(DownloadEvent::Progress {
            completed, total,
            current_url: format!("{}/{}", completed, total),
            bytes_downloaded: 0, elapsed_ms: 0,
        });
    };

    let include_plugins_flag = include_plugins.unwrap_or(true);
    let size = importer::export_collection(&collection, &data_dir, &export_path, Some(&progress_cb), saves.as_ref(), include_plugins_flag)?;

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
    });

    Ok(size)
}

/// Export saves as a .aaosave file.
#[tauri::command]
pub fn export_save(
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
    saves: serde_json::Value,
    include_plugins: bool,
    dest_path: String,
) -> Result<u64, String> {
    let data_dir = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.data_dir.clone()
    };
    let path = std::path::PathBuf::from(&dest_path);
    importer::export_aaosave(&case_ids, &saves, include_plugins, &path, &data_dir)
}
