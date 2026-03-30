use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::config;

/// Print only in debug builds.
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            println!($($arg)*);
        }
    };
}

// Engine files embedded at compile time by build.rs via include_bytes!.
// Used on Android to extract engine files to the writable filesystem.
// This bypasses Tauri's fs plugin which corrupts binary data on Android.
include!(concat!(env!("OUT_DIR"), "/engine_embed.rs"));

/// Shared state holding the asset server port, engine directory, and user config.
pub(crate) struct AppState {
    pub(crate) server_port: u16,
    /// Static engine files (JS, CSS, HTML, img, Languages). Read-only on mobile.
    pub(crate) engine_dir: PathBuf,
    /// Writable data directory (case/, defaults/, config.json).
    /// On desktop this equals engine_dir. On Android/iOS it's the app's private data dir.
    pub(crate) data_dir: PathBuf,
    pub(crate) config: config::AppConfig,
    /// Cancel flag for in-progress downloads. Checked per-asset in the download loop.
    pub(crate) cancel_flag: Arc<AtomicBool>,
    /// Shared HTTP client — reuses connection pool across all download commands.
    pub(crate) http_client: reqwest::Client,
}

/// Extract engine files from the embedded binary data to the writable filesystem.
///
/// Engine files are embedded at compile time via `include_bytes!` in build.rs.
/// This avoids Tauri's `app.fs().read()` which corrupts binary data (GIFs, fonts)
/// when reading from APK assets on Android. The embedded data is byte-identical
/// to the original files from the build machine.
pub(crate) fn extract_engine_files(dest: &std::path::Path) -> Result<(), String> {
    debug_log!(
        "Extracting {} engine files to {}...",
        EMBEDDED_ENGINE_FILES.len(),
        dest.display()
    );

    for (name, data) in EMBEDDED_ENGINE_FILES {
        let dest_path = dest.join(name);
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory for '{}': {}", name, e))?;
        }
        std::fs::write(&dest_path, data)
            .map_err(|e| format!("Failed to write '{}': {}", name, e))?;
    }

    debug_log!("Engine files extracted successfully");
    Ok(())
}
