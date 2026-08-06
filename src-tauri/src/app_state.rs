//! Global application state management.
//!
//! This module defines the core structs used for sharing state across
//! Tauri commands and threads. It also handles the specialized asset
//! extraction required on mobile platforms.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crate::config;
use crate::downloader::cancel::Cancellation;
use crate::downloader::dedup::DedupIndex;

// Engine files embedded at compile time by build.rs via include_bytes!.
// Used on Android to extract engine files to the writable filesystem.
// This bypasses Tauri's fs plugin which corrupts binary data on Android.
include!(concat!(env!("OUT_DIR"), "/engine_embed.rs"));

/// Shared application state (immutable).
///
/// This struct is created once during app initialization and is available
/// to all commands via Tauri's `State` system.
pub(crate) struct AppPaths {
    // --- Server ---
    /// Port the localhost HTTP server is listening on, or 0 if not started.
    /// On Android the server always runs (audio needs real HTTP to bypass a
    /// Chromium custom-protocol Range-request bug). On desktop it only runs
    /// during one-time localStorage migration.
    pub(crate) server_port: u16,
    /// Localhost HTTP server handle. Dropping on app exit stops the thread.
    #[allow(dead_code)]
    pub(crate) localhost_server: Option<crate::server::LocalhostServer>,
    // --- Directories ---
    /// Static engine files (JS, CSS, HTML, img, Languages). Read-only on mobile.
    pub(crate) engine_dir: PathBuf,
    /// Writable data directory (case/, defaults/, config.json).
    ///
    /// On desktop this equals engine_dir. On Android/iOS it's the app's private data dir.
    pub(crate) data_dir: PathBuf,
    // --- Downloads ---
    /// Per-operation cancellation (generation counter). Each operation
    /// snapshots the current generation at start and checks it per asset.
    pub(crate) cancellation: Cancellation,
    /// Shared HTTP client — reuses connection pool across all download commands.
    pub(crate) http_client: reqwest::Client,
    /// Single shared dedup index. `None` = index unavailable (dedup disabled, non-fatal).
    pub(crate) dedup_index: Option<Arc<DedupIndex>>,
    /// TTL cache for storage-info scans (see config::StorageCache).
    pub(crate) storage_cache: crate::config::StorageCache,
}

/// Shared application configuration (mutable).
///
/// This wraps `AppConfig` in a `Mutex` to allow safe modification from
/// any thread or command.
pub(crate) struct MutableConfig(pub(crate) Mutex<config::AppConfig>);

/// Extract engine assets from the embedded binary data to the disk.
///
/// Since Android's Asset Manager can be slow and sometimes corrupts binary
/// files when read via certain plugins, we embed the core engine assets
/// (HTML, JS, CSS, images) directly into the executable at compile time.
/// On the first run, these are extracted to the app's writable data directory.
///
/// # Errors
///
/// Returns an error if any file fails to write to the destination.
pub(crate) fn extract_engine_files(dest: &std::path::Path) -> Result<(), crate::error::AppError> {
    log::info!(
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

    log::info!("Engine files extracted successfully");
    Ok(())
}
