//! Commands for exporting cases and sequences to external files.
//!
//! This module handles exporting downloaded cases as `.aaocase` ZIP files,
//! which can be shared with other users or used as backups. It also
//! supports exporting game saves as `.aaosave` files.

use std::fs;
use std::path::{Path, PathBuf};
use tauri::ipc::Channel;
use tauri::State;

use crate::app_state::AppPaths;
use crate::collections as coll;
use crate::downloader::asset_downloader::DownloadEvent;
use crate::error::AppError;
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

fn write_to_content_uri<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    export_path: &Path,
    uri: &str,
) -> Result<(), AppError> {
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

async fn run_blocking_export<F>(
    on_event: Channel<DownloadEvent>,
    f: F,
) -> Result<u64, AppError>
where
    F: FnOnce(Box<dyn Fn(usize, usize) + Send + Sync>) -> Result<u64, AppError>
        + Send
        + 'static,
{
    let on_event_clone = on_event.clone();
    tokio::task::spawn_blocking(move || {
        let progress_cb: Box<dyn Fn(usize, usize) + Send + Sync> =
            Box::new(move |completed, total| {
                let _ = on_event_clone.send(DownloadEvent::Progress {
                    completed,
                    total,
                    current_url: format!("{}/{}", completed, total),
                    bytes_downloaded: 0,
                    elapsed_ms: 0,
                });
            });
        f(progress_cb)
    })
    .await
    .map_err(|e| AppError::Other(format!("Export task failed: {}", e)))?
}

async fn run_export<R, F>(
    app: tauri::AppHandle<R>,
    dest_path: &str,
    data_dir: &std::path::Path,
    on_event: Channel<DownloadEvent>,
    f: F,
) -> Result<u64, AppError>
where
    R: tauri::Runtime,
    F: FnOnce(PathBuf, Box<dyn Fn(usize, usize) + Send + Sync>) -> Result<u64, AppError>
        + Send
        + 'static,
{
    let (export_path, content_uri) = resolve_export_path(dest_path, data_dir);
    let _ = on_event.send(DownloadEvent::Started { total: 0 });

    let export_path_for_uri = export_path.clone();

    // Collect all fallible work after Started into one Result so we can
    // route cleanly to either Finished or Error — no ? can escape silently.
    let result: Result<u64, AppError> = async {
        let size = run_blocking_export(on_event.clone(), move |progress_cb| {
            f(export_path, progress_cb)
        })
        .await?;

        if let Some(ref uri) = content_uri {
            write_to_content_uri(&app, &export_path_for_uri, uri)?;
        }

        Ok(size)
    }
    .await;

    match result {
        Ok(size) => {
            let _ = on_event.send(DownloadEvent::Finished {
                downloaded: 0,
                failed: 0,
                total_bytes: size,
                dedup_saved_bytes: 0,
            });
            Ok(size)
        }
        Err(e) => {
            // Unblock the frontend — JS "error" handler dismisses the spinner.
            let _ = on_event.send(DownloadEvent::Error { message: e.to_string() });
            Err(e)
        }
    }
}

/// Export a single case as a `.aaocase` ZIP file.
///
/// # Arguments
///
/// * `case_id` - The ID of the case to export.
/// * `dest_path` - Local file path where the ZIP should be saved.
/// * `saves` - (Optional) Current game saves to include in the export.
/// * `include_plugins` - Whether to include custom plugins in the ZIP.
/// * `on_event` - Progress channel for the UI.
#[tauri::command]
pub async fn export_case(
    app: tauri::AppHandle,
    paths: State<'_, AppPaths>,
    case_id: u32,
    dest_path: String,
    saves: Option<serde_json::Value>,
    include_plugins: Option<bool>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, AppError> {
    let data_dir = paths.data_dir.clone();
    let data_dir_for_closure = data_dir.clone();
    let include_plugins_flag = include_plugins.unwrap_or(true);

    let size = run_export(app, &dest_path, &data_dir, on_event, move |export_path, progress_cb| {
        importer::export_aaocase(
            case_id,
            &data_dir_for_closure,
            &export_path,
            Some(&*progress_cb),
            saves.as_ref(),
            include_plugins_flag,
        )
    })
    .await?;

    log::info!("Exported case {} to {} ({} bytes)", case_id, dest_path, size);
    Ok(size)
}

/// Export a sequence of cases as a single `.aaocase` ZIP file.
///
/// # Arguments
///
/// * `case_ids` - List of case IDs to include.
/// * `sequence_title` - Title for the exported sequence.
/// * `sequence_list` - JSON metadata describing the sequence order.
/// * `dest_path` - Output ZIP path.
/// * `saves` - (Optional) Game saves to include.
#[tauri::command]
pub async fn export_sequence(
    app: tauri::AppHandle,
    paths: State<'_, AppPaths>,
    case_ids: Vec<u32>,
    sequence_title: String,
    sequence_list: serde_json::Value,
    dest_path: String,
    saves: Option<serde_json::Value>,
    include_plugins: Option<bool>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, AppError> {
    let data_dir = paths.data_dir.clone();
    let data_dir_for_closure = data_dir.clone();
    let include_plugins_flag = include_plugins.unwrap_or(true);

    run_export(app, &dest_path, &data_dir, on_event, move |export_path, progress_cb| {
        importer::export_sequence(
            &case_ids,
            &sequence_title,
            &sequence_list,
            &data_dir_for_closure,
            &export_path,
            Some(&*progress_cb),
            saves.as_ref(),
            include_plugins_flag,
        )
    })
    .await
}

/// Export a collection of cases as a single `.aaocase` ZIP file.
#[tauri::command]
pub async fn export_collection(
    paths: State<'_, AppPaths>,
    collection_id: String,
    dest_path: String,
    saves: Option<serde_json::Value>,
    include_plugins: Option<bool>,
    on_event: Channel<DownloadEvent>,
) -> Result<u64, AppError> {
    let data_dir = paths.data_dir.clone();

    let coll_data = coll::load_collections(&data_dir);
    let collection = coll_data.collections.iter()
        .find(|c| c.id == collection_id)
        .ok_or_else(|| format!("Collection {} not found", collection_id))?
        .clone();

    let export_path = PathBuf::from(&dest_path);
    let include_plugins_flag = include_plugins.unwrap_or(true);

    let size = run_blocking_export(on_event.clone(), move |progress_cb| {
        importer::export_collection(
            &collection,
            &data_dir,
            &export_path,
            Some(&*progress_cb),
            saves.as_ref(),
            include_plugins_flag,
        )
    })
    .await?;

    let _ = on_event.send(DownloadEvent::Finished {
        downloaded: 0,
        failed: 0,
        total_bytes: size,
        dedup_saved_bytes: 0,
    });

    Ok(size)
}

/// Export game saves for one or more cases as a `.aaosave` file.
#[tauri::command]
pub async fn export_save(
    paths: State<'_, AppPaths>,
    case_ids: Vec<u32>,
    saves: serde_json::Value,
    include_plugins: bool,
    dest_path: String,
) -> Result<u64, AppError> {
    let data_dir = paths.data_dir.clone();
    let path = PathBuf::from(&dest_path);
    Ok(tokio::task::spawn_blocking(move || {
        importer::export_aaosave(&case_ids, &saves, include_plugins, &path, &data_dir)
    }).await.map_err(|e| format!("Export task failed: {}", e))??)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // ─── resolve_export_path ──────────────────────────────────────────────────
    // Pure-function regression tests written BEFORE the refactoring.

    #[test]
    fn test_resolve_export_path_normal_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = "/some/path/output.aaocase";
        let (path, uri) = resolve_export_path(dest, tmp.path());
        assert_eq!(path, PathBuf::from(dest));
        assert!(uri.is_none(), "normal path must have no content URI");
    }

    #[test]
    fn test_resolve_export_path_content_uri() {
        let tmp = tempfile::tempdir().unwrap();
        let dest =
            "content://com.android.externalstorage.documents/document/primary%3Atest.aaocase";
        let (path, uri) = resolve_export_path(dest, tmp.path());
        assert_eq!(
            path,
            tmp.path().join("_export_temp.aaocase"),
            "content:// URI must route through temp file"
        );
        assert_eq!(uri.as_deref(), Some(dest), "URI string must be preserved verbatim");
    }

    #[test]
    fn test_resolve_export_path_empty_string() {
        let tmp = tempfile::tempdir().unwrap();
        let (path, uri) = resolve_export_path("", tmp.path());
        assert_eq!(path, PathBuf::from(""), "empty path should pass through unchanged");
        assert!(uri.is_none());
    }

    // ─── run_blocking_export ─────────────────────────────────────────────────
    // TDD tests written before the commands are rewritten to use this helper.
    // Note: tauri::test::mock_app() requires the "test" feature which is
    // incompatible with Windows dev builds, so run_export event-sequence tests
    // are covered by manual UI testing (Step 8 of the implementation plan).

    #[tokio::test]
    async fn test_run_blocking_export_returns_correct_size() {
        let on_event = tauri::ipc::Channel::new(|_| Ok(()));
        let result = run_blocking_export(on_event, |_cb| Ok(1234u64)).await;
        assert_eq!(result.unwrap(), 1234u64);
    }

    #[tokio::test]
    async fn test_run_blocking_export_calls_progress_callback() {
        let call_count: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
        let call_count_clone = call_count.clone();

        let sent: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sent_clone = sent.clone();
        let on_event = tauri::ipc::Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                sent_clone.lock().unwrap().push(s);
            }
            *call_count_clone.lock().unwrap() += 1;
            Ok(())
        });

        run_blocking_export(on_event, |progress_cb| {
            progress_cb(1, 3);
            progress_cb(2, 3);
            progress_cb(3, 3);
            Ok(0u64)
        })
        .await
        .unwrap();

        assert_eq!(*call_count.lock().unwrap(), 3, "expected exactly 3 progress events");
        for s in sent.lock().unwrap().iter() {
            assert!(
                s.contains("\"progress\""),
                "event JSON must be a progress event: {s}"
            );
        }
    }

    #[tokio::test]
    async fn test_run_blocking_export_propagates_error() {
        let on_event = tauri::ipc::Channel::new(|_| Ok(()));
        let result = run_blocking_export(on_event, |_cb| {
            Err(AppError::Other("simulated failure".to_string()))
        })
        .await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("simulated failure"),
            "error message must propagate unchanged"
        );
    }

    #[tokio::test]
    async fn test_run_blocking_export_zero_progress_ok() {
        // Closure that never calls the progress callback must still succeed.
        let on_event = tauri::ipc::Channel::new(|_| Ok(()));
        let result = run_blocking_export(on_event, |_cb| Ok(0u64)).await;
        assert_eq!(result.unwrap(), 0u64);
    }
}
