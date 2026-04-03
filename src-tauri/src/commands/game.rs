//! Commands for launching the AAO engine and managing the game state.
//!
//! This module handles generating the URLs used to load the AAO player
//! in the WebView, as well as providing diagnostic information about
//! local files used by the engine.

use tauri::State;

use crate::app_state::{AppPaths, MutableConfig};
use crate::error::AppError;
use crate::importer;

/// Protocol base URL — platform-dependent.
/// Windows/Android: WebView2 transforms custom protocols to http://[scheme].localhost
/// macOS/iOS/Linux: WebKitGTK keeps the scheme intact as [scheme]://localhost
pub(crate) fn protocol_base_url() -> &'static str {
    if cfg!(any(target_os = "windows", target_os = "android")) {
        "http://aao.localhost"
    } else {
        "aao://localhost"
    }
}

/// Build the player URL for a given case using the custom protocol.
pub(crate) fn build_game_url(case_id: u32, lang: &str) -> String {
    format!(
        "{}/player.html?trial_id={}&lang={}",
        protocol_base_url(),
        case_id,
        lang
    )
}

/// Build the server base URL using the custom protocol.
pub(crate) fn build_server_url() -> String {
    protocol_base_url().to_string()
}

/// Returns the URL for playing a specific case, including language preference.
///
/// This builds a URL using the custom `aao://` (or `http://aao.localhost`)
/// protocol that the app's internal server handles.
///
/// # Arguments
///
/// * `case_id` - The ID of the case to play.
///
/// # Returns
///
/// A string containing the full URL to load in the WebView.
#[tauri::command]
pub fn open_game(paths: State<'_, AppPaths>, config: State<'_, MutableConfig>, case_id: u32) -> Result<String, AppError> {
    let lang = config.0.lock().map_err(|e| e.to_string())?.language.clone();
    // Resolve which global plugins apply to this case (writes resolved_plugins.json)
    let _ = importer::resolve_plugins_for_case(case_id, &paths.data_dir);
    Ok(build_game_url(case_id, &lang))
}

/// Returns the asset server's base URL (custom protocol).
///
/// This is used by the frontend to construct URLs for assets not directly
/// linked to a specific case (e.g., global UI images).
#[tauri::command]
pub fn get_server_url() -> Result<String, AppError> {
    Ok(build_server_url())
}

/// Returns the old tiny_http server URL for one-time localStorage migration.
/// This is the http://localhost:{port} URL that holds the user's old saves.
/// Returns an error when the migration server was not started (migration_complete = true).
/// Will be removed in a future release when tiny_http is fully deleted.
#[tauri::command]
pub fn get_migration_server_url(paths: State<'_, AppPaths>) -> Result<String, AppError> {
    if paths.server_port == 0 {
        return Err(AppError::Other(
            "Migration server is not running (migration already complete)".to_string(),
        ));
    }
    Ok(format!("http://localhost:{}", paths.server_port))
}

/// Debug command: check if a file exists on disk and return diagnostic info.
/// Returns full path details in debug builds, "debug only" in release.
#[tauri::command]
pub fn debug_check_file(
    paths: State<'_, AppPaths>,
    relative_path: String,
) -> Result<String, AppError> {
    if !cfg!(debug_assertions) {
        return Ok("debug_check_file is only available in debug builds".to_string());
    }
    let data_path = paths.data_dir.join(&relative_path);
    let engine_path = paths.engine_dir.join(&relative_path);
    let data_exists = data_path.exists();
    let data_is_file = data_path.is_file();
    let engine_exists = engine_path.exists();
    let engine_is_file = engine_path.is_file();
    let data_size = if data_is_file {
        std::fs::metadata(&data_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    let parent = data_path.parent().map(|p| p.to_path_buf());
    let parent_exists = parent.as_ref().map(|p| p.is_dir()).unwrap_or(false);
    let parent_contents = parent.as_ref().and_then(|p| {
        std::fs::read_dir(p).ok().map(|entries| {
            entries.flatten().take(20)
                .map(|e| format!("{}({})", e.file_name().to_string_lossy(),
                    e.metadata().map(|m| m.len()).unwrap_or(0)))
                .collect::<Vec<_>>().join(", ")
        })
    }).unwrap_or_else(|| "(cannot read parent)".to_string());

    Ok(format!(
        "relative={}\ndata_path={}\ndata_exists={}\ndata_is_file={}\ndata_size={}\nengine_path={}\nengine_exists={}\nengine_is_file={}\nparent_exists={}\nparent_contents=[{}]\ndata_dir={}\nengine_dir={}",
        relative_path,
        data_path.display(), data_exists, data_is_file, data_size,
        engine_path.display(), engine_exists, engine_is_file,
        parent_exists, parent_contents,
        paths.data_dir.display(), paths.engine_dir.display(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_base_url() {
        let url = protocol_base_url();
        if cfg!(any(target_os = "windows", target_os = "android")) {
            assert_eq!(url, "http://aao.localhost");
        } else {
            assert_eq!(url, "aao://localhost");
        }
        assert!(!url.ends_with('/'), "Base URL must not have trailing slash");
    }

    #[test]
    fn test_build_game_url_format() {
        let url = build_game_url(69063, "en");
        let expected = format!(
            "{}/player.html?trial_id=69063&lang=en",
            protocol_base_url()
        );
        assert_eq!(url, expected);
    }

    #[test]
    fn test_build_game_url_different_params() {
        let url = build_game_url(42, "fr");
        assert!(url.contains("trial_id=42"));
        assert!(url.contains("lang=fr"));
        assert!(url.starts_with(protocol_base_url()));
    }

    #[test]
    fn test_build_server_url_format() {
        let url = build_server_url();
        assert_eq!(url, protocol_base_url());
        assert!(!url.ends_with('/'), "Server URL must not have trailing slash");
    }

    #[test]
    fn test_game_url_contains_expected_parts() {
        let url = build_game_url(100, "de");
        assert!(url.starts_with(protocol_base_url()));
        assert!(url.contains("player.html"));
        assert!(url.contains("trial_id=100"));
        assert!(url.contains("lang=de"));
    }
}
