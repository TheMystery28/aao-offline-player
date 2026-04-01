use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::State;

use crate::app_state::{AppState, AppStateLock};
use crate::collections as coll;
use crate::downloader::asset_downloader::DownloadEvent;
use crate::importer;

/// Resolve export destination: on Android, content:// URIs need a temp file.
fn resolve_export_path(dest_path: &str, data_dir: &Path) -> (PathBuf, Option<String>) {
    if dest_path.starts_with("content://") {
        let temp = data_dir.join("_export_temp.aaocase");
        (temp, Some(dest_path.to_string()))
    } else {
        (PathBuf::from(dest_path), None)
    }
}

/// Copy a temp export file to an Android content:// URI, then clean up the temp.
fn write_to_content_uri(app: &tauri::AppHandle, export_path: &Path, uri: &str) -> Result<(), String> {
    use tauri_plugin_fs::FsExt;
    use std::io::Write;
    let data = fs::read(export_path)
        .map_err(|e| format!("Failed to read temp export: {}", e))?;
    let dest_url = reqwest::Url::parse(uri)
        .map_err(|e| format!("Failed to parse content URI: {}", e))?;
    let dest_fp = tauri_plugin_fs::FilePath::from(dest_url);
    let opts = tauri_plugin_fs::OpenOptions::new().write(true).create(true).clone();
    let mut file = app.fs().open(dest_fp, opts)
        .map_err(|e| format!("Failed to open content URI for writing: {}", e))?;
    file.write_all(&data)
        .map_err(|e| format!("Failed to write to content URI: {}", e))?;
    let _ = fs::remove_file(export_path);
    Ok(())
}

/// Export a case as a .aaocase ZIP file.
/// If `saves` is provided, includes it as saves.json in the ZIP.
/// On Android, `dest_path` may be a content:// URI — exports to temp then copies.
#[tauri::command]
pub async fn export_case(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
    case_id: u32,
    dest_path: String,
    saves: Option<serde_json::Value>,
    include_plugins: Option<bool>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, String> {
    let data_dir = state.data_dir()?;

    let (export_path, content_uri) = resolve_export_path(&dest_path, &data_dir);

    let _ = on_event.send(DownloadEvent::Started { total: 0 });

    let include_plugins_flag = include_plugins.unwrap_or(true);
    let export_path_clone = export_path.clone();
    let data_dir_clone = data_dir.clone();
    let on_event_clone = on_event.clone();

    let size = tokio::task::spawn_blocking(move || {
        let progress_cb = |completed: usize, total: usize| {
            let _ = on_event_clone.send(DownloadEvent::Progress {
                completed,
                total,
                current_url: format!("{}/{}", completed, total),
                bytes_downloaded: 0, elapsed_ms: 0,
            });
        };
        importer::export_aaocase(case_id, &data_dir_clone, &export_path_clone, Some(&progress_cb), saves.as_ref(), include_plugins_flag)
    }).await.map_err(|e| format!("Export task failed: {}", e))??;

    if let Some(ref uri) = content_uri {
        write_to_content_uri(&app, &export_path, uri)?;
    }

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
        dedup_saved_bytes: 0,
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
pub async fn export_sequence(
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
    let data_dir = state.data_dir()?;

    let (export_path, content_uri) = resolve_export_path(&dest_path, &data_dir);

    let _ = on_event.send(DownloadEvent::Started { total: 0 });

    let include_plugins_flag = include_plugins.unwrap_or(true);
    let export_path_clone = export_path.clone();
    let data_dir_clone = data_dir.clone();
    let on_event_clone = on_event.clone();

    let size = tokio::task::spawn_blocking(move || {
        let progress_cb = |completed: usize, total: usize| {
            let _ = on_event_clone.send(DownloadEvent::Progress {
                completed,
                total,
                current_url: format!("{}/{}", completed, total),
                bytes_downloaded: 0, elapsed_ms: 0,
            });
        };
        importer::export_sequence(
            &case_ids,
            &sequence_title,
            &sequence_list,
            &data_dir_clone,
            &export_path_clone,
            Some(&progress_cb),
            saves.as_ref(),
            include_plugins_flag,
        )
    }).await.map_err(|e| format!("Export task failed: {}", e))??;

    if let Some(ref uri) = content_uri {
        write_to_content_uri(&app, &export_path, uri)?;
    }

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
        dedup_saved_bytes: 0,
    });

    Ok(size)
}

/// Export a collection as a .aaocase ZIP file.
#[tauri::command]
pub async fn export_collection(
    state: State<'_, Mutex<AppState>>,
    collection_id: String,
    dest_path: String,
    saves: Option<serde_json::Value>,
    include_plugins: Option<bool>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, String> {
    let data_dir = state.data_dir()?;

    let coll_data = coll::load_collections(&data_dir);
    let collection = coll_data.collections.iter()
        .find(|c| c.id == collection_id)
        .ok_or_else(|| format!("Collection {} not found", collection_id))?
        .clone();

    let export_path = PathBuf::from(&dest_path);
    let on_event_clone = on_event.clone();
    let include_plugins_flag = include_plugins.unwrap_or(true);

    let size = tokio::task::spawn_blocking(move || {
        let progress_cb = |completed: usize, total: usize| {
            let _ = on_event_clone.send(DownloadEvent::Progress {
                completed, total,
                current_url: format!("{}/{}", completed, total),
                bytes_downloaded: 0, elapsed_ms: 0,
            });
        };
        importer::export_collection(&collection, &data_dir, &export_path, Some(&progress_cb), saves.as_ref(), include_plugins_flag)
    }).await.map_err(|e| format!("Export task failed: {}", e))??;

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
        dedup_saved_bytes: 0,
    });

    Ok(size)
}

/// Export saves as a .aaosave file.
#[tauri::command]
pub async fn export_save(
    state: State<'_, Mutex<AppState>>,
    case_ids: Vec<u32>,
    saves: serde_json::Value,
    include_plugins: bool,
    dest_path: String,
) -> Result<u64, String> {
    let data_dir = state.data_dir()?;
    let path = PathBuf::from(&dest_path);
    tokio::task::spawn_blocking(move || {
        importer::export_aaosave(&case_ids, &saves, include_plugins, &path, &data_dir)
    }).await.map_err(|e| format!("Export task failed: {}", e))?
}
