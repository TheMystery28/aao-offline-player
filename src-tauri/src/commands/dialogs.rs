/// Open a native folder picker dialog. Returns the selected path or null if cancelled.
/// On Android, folder picking is not supported — returns an error.
#[tauri::command]
pub async fn pick_folder(_app: tauri::AppHandle) -> Result<Option<String>, String> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;
        let result = _app
            .dialog()
            .file()
            .set_title("Select aaoffline download folder")
            .blocking_pick_folder();
        match result {
            Some(file_path) => {
                let path = file_path
                    .into_path()
                    .map_err(|e| format!("Invalid path: {}", e))?;
                Ok(Some(path.to_string_lossy().to_string()))
            }
            None => Ok(None),
        }
    }
    #[cfg(target_os = "android")]
    {
        Err("Folder picking is not supported on Android. Use file import instead.".to_string())
    }
}

/// Open a native file picker dialog for .aaocase/.zip files. Returns the selected path or null.
///
/// On Android, the dialog returns `content://` URIs instead of filesystem paths.
/// The import_case command handles both formats.
#[tauri::command]
pub async fn pick_import_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app
        .dialog()
        .file()
        .set_title("Select .aaocase, .aaoplug, or .aaosave file");

    // On Android, the SAF uses MIME types instead of file extensions.
    if cfg!(target_os = "android") {
        builder = builder.add_filter("AAO Files", &["application/zip", "application/octet-stream"]);
    } else {
        builder = builder.add_filter("AAO Files", &["aaocase", "aaoplug", "aaosave", "zip"]);
    }

    let result = builder.blocking_pick_file();
    match result {
        Some(file_path) => {
            // On desktop: into_path() gives a filesystem path.
            // On Android: into_path() fails for content:// URIs.
            // Try path conversion first, fall back to path() for URI.
            if let Some(path) = file_path.as_path() {
                Ok(Some(path.to_string_lossy().to_string()))
            } else {
                // Android content:// URI — convert to string for import_case.
                // import_case will copy it to a temp file via Tauri's fs plugin.
                Ok(Some(file_path.to_string()))
            }
        }
        None => Ok(None),
    }
}

/// Shared helper for "Save As" dialogs. Handles Android (no extension filters) and desktop.
fn pick_save_file(app: &tauri::AppHandle, title: &str, filter_name: &str, ext: &str, default_name: &str) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file()
        .set_title(title)
        .set_file_name(default_name);
    if !cfg!(target_os = "android") {
        builder = builder.add_filter(filter_name, &[ext]);
    }
    match builder.blocking_save_file() {
        Some(file_path) => {
            if let Some(path) = file_path.as_path() {
                Ok(Some(path.to_string_lossy().to_string()))
            } else {
                Ok(Some(file_path.to_string()))
            }
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn pick_export_file(app: tauri::AppHandle, default_name: String) -> Result<Option<String>, String> {
    pick_save_file(&app, "Export case as .aaocase", "AAO Case", "aaocase", &default_name)
}

#[tauri::command]
pub async fn pick_export_plugin_file(app: tauri::AppHandle, default_name: String) -> Result<Option<String>, String> {
    pick_save_file(&app, "Export plugins as .aaoplug", "AAO Plugin", "aaoplug", &default_name)
}

#[tauri::command]
pub async fn pick_export_save_file(app: tauri::AppHandle, default_name: String) -> Result<Option<String>, String> {
    pick_save_file(&app, "Export saves as .aaosave", "AAO Save", "aaosave", &default_name)
}
