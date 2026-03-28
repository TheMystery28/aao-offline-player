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

/// Open a native "Save As" dialog for exporting a .aaocase file.
/// `default_name` is the suggested filename (e.g. "My Case.aaocase").
#[tauri::command]
pub async fn pick_export_file(app: tauri::AppHandle, default_name: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app
        .dialog()
        .file()
        .set_title("Export case as .aaocase")
        .set_file_name(&default_name);

    // On Android, extension filters don't work — use MIME type
    if !cfg!(target_os = "android") {
        builder = builder.add_filter("AAO Case", &["aaocase"]);
    }

    let result = builder.blocking_save_file();
    match result {
        Some(file_path) => {
            // On desktop: filesystem path. On Android: content:// URI.
            if let Some(path) = file_path.as_path() {
                Ok(Some(path.to_string_lossy().to_string()))
            } else {
                Ok(Some(file_path.to_string()))
            }
        }
        None => Ok(None),
    }
}

/// Open a native "Save As" dialog for exporting a .aaoplug file.
#[tauri::command]
pub async fn pick_export_plugin_file(app: tauri::AppHandle, default_name: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app
        .dialog()
        .file()
        .set_title("Export plugins as .aaoplug")
        .set_file_name(&default_name);

    if !cfg!(target_os = "android") {
        builder = builder.add_filter("AAO Plugin", &["aaoplug"]);
    }

    let result = builder.blocking_save_file();
    match result {
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

/// Open a native "Save As" dialog for exporting a .aaosave file.
#[tauri::command]
pub async fn pick_export_save_file(app: tauri::AppHandle, default_name: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app
        .dialog()
        .file()
        .set_title("Export saves as .aaosave")
        .set_file_name(&default_name);

    if !cfg!(target_os = "android") {
        builder = builder.add_filter("AAO Save", &["aaosave"]);
    }

    let result = builder.blocking_save_file();
    match result {
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
