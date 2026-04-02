use tauri::ipc::Channel;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::app_state::{AppPaths, MutableConfig};
use crate::config;
use crate::downloader;
use crate::downloader::asset_downloader::DownloadEvent;
use crate::error::AppError;

/// Return current user settings.
#[tauri::command]
pub fn get_settings(config: State<'_, MutableConfig>) -> Result<config::AppConfig, AppError> {
    let cfg = config.0.lock().map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

/// Save user settings (validated and clamped).
#[tauri::command]
pub fn save_settings(
    paths: State<'_, AppPaths>,
    config_state: State<'_, MutableConfig>,
    settings: config::AppConfig,
) -> Result<(), AppError> {
    let mut validated = settings;
    config::validate(&mut validated);
    config::save_config(&paths.data_dir, &validated)?;
    let mut cfg = config_state.0.lock().map_err(|e| e.to_string())?;
    *cfg = validated;
    Ok(())
}

/// Return storage usage statistics.
#[tauri::command]
pub fn get_storage_info(paths: State<'_, AppPaths>) -> Result<config::StorageInfo, AppError> {
    let data_dir = &paths.data_dir;
    Ok(config::compute_storage_info(data_dir))
}

/// Optimize storage by deduplicating assets across all cases.
/// Promotes shared assets to defaults/shared/ and removes duplicate case copies.
#[tauri::command]
pub async fn optimize_storage(
    paths: State<'_, AppPaths>,
    on_event: Channel<DownloadEvent>,
) -> Result<serde_json::Value, AppError> {
    let data_dir = &paths.data_dir;
    let (deduped, bytes_saved) = downloader::dedup::optimize_all_cases(
        data_dir,
        Some(&|completed, total, current_path| {
            let _ = on_event.send(DownloadEvent::Progress {
                completed,
                total,
                current_url: current_path.to_string(),
                bytes_downloaded: 0,
                elapsed_ms: 0,
            });
        }),
    )?;
    log::info!("Optimize storage: {} files deduplicated, {} bytes saved", deduped, bytes_saved);
    Ok(serde_json::json!({
        "deduped": deduped,
        "bytes_saved": bytes_saved
    }))
}

/// Open the data directory in the system file explorer.
#[tauri::command]
pub fn open_data_dir(
    paths: State<'_, AppPaths>,
    app: tauri::AppHandle,
) -> Result<(), AppError> {
    let path_str = paths.data_dir.to_string_lossy();
    #[cfg(not(target_os = "android"))]
    app.opener()
        .open_path(&*path_str, None::<&str>)
        .map_err(|e| format!("Failed to open directory: {}", e))?;
    #[cfg(target_os = "android")]
    log::debug!("Data directory (Android): {}", path_str);
    Ok(())
}
