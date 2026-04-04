//! Main entry point for the AAO Offline Player application logic.
//!
//! This module sets up the Tauri application builder, handles logging
//! configuration, registers custom URI schemes, and manages the lifecycle
//! of the internal asset server.

#[macro_use]
mod app_state;
mod collections;
mod commands;
mod config;
mod downloader;
pub mod error;
mod importer;
mod server;
pub mod utils;

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;

use app_state::{AppPaths, MutableConfig};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Initializes and runs the Tauri application.
///
/// This is the primary entry point for the Rust backend. It performs the following:
///
/// 1.  **Logging Setup**: Configures `tauri-plugin-log` with appropriate targets
///     (Stdout for debug, file-based for release).
/// 2.  **Plugin Registration**: Initializes essential plugins for shell, HTTP,
///     filesystem, and native dialogs.
/// 3.  **Custom URI Scheme**: Registers the `aao://` protocol to serve engine
///     files and case assets securely from the local filesystem.
/// 4.  **App Setup**:
///     - Determines platform-specific paths for engine files and user data.
///     - Extracts bundled engine assets to the writable filesystem on mobile.
///     - Loads user configuration (`config.json`).
///     - Starts a background `tiny_http` server for one-time localStorage migration.
///     - Initializes the shared `reqwest` HTTP client.
/// 5.  **State Management**: Registers `AppPaths` and `MutableConfig` as managed state.
/// 6.  **Command Registration**: Exposes all functions in the `commands` module
///     to the frontend.
///
/// # Panics
///
/// Panics if the Tauri context cannot be generated or if the application
/// fails to initialize its core state.
pub fn run() {
    tauri::Builder::default()
        .plugin({
            let builder = tauri_plugin_log::Builder::new();
            #[cfg(debug_assertions)]
            let builder = builder
                .level(log::LevelFilter::Debug)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ));
            #[cfg(not(debug_assertions))]
            let builder = builder
                .level(log::LevelFilter::Info)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("tauri", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Warn)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("aao-offline-player".to_string()),
                    },
                ))
                .max_file_size(5 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne);
            builder.build()
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("aao", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            // Extract owned strings before the async move — spawn_blocking requires 'static.
            let url_path = request.uri().path().to_owned();
            let method = request.method().as_str().to_owned();
            let range = request
                .headers()
                .get("range")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_owned());

            tauri::async_runtime::spawn(async move {
                let paths = app.state::<AppPaths>();
                let config = server::ServerConfig {
                    engine_dir: paths.engine_dir.clone(),
                    data_dir: paths.data_dir.clone(),
                };

                let url_path_log = url_path.clone();

                // serve_file does blocking disk I/O — delegate to the blocking thread pool.
                let timeout_result = tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    tokio::task::spawn_blocking(move || {
                        server::serve_file(&config, &url_path, &method, range.as_deref())
                    }),
                )
                .await;

                let result = match timeout_result {
                    Ok(Ok(serve_res)) => serve_res,
                    Ok(Err(e)) => {
                        log::error!("File serving task panicked: {:?}", e);
                        server::ServeResult {
                            status: 500,
                            headers: vec![],
                            data: b"Internal server error".to_vec(),
                        }
                    }
                    Err(_elapsed) => {
                        log::warn!("File serving request timed out for path: {}", url_path_log);
                        server::ServeResult {
                            status: 504,
                            headers: vec![],
                            data: b"Request timeout".to_vec(),
                        }
                    }
                };

                responder.respond(server::serve_result_to_response(result));
            });
        })
        .setup(|app| {
            // Determine engine_dir and data_dir based on platform.
            //
            // Desktop (Windows/macOS/Linux):
            //   engine_dir = resource_dir/engine (installed) or source engine/ (dev mode)
            //   data_dir = engine_dir (everything in one writable directory)
            //
            // Mobile (Android/iOS):
            //   data_dir = app_data_dir/engine (writable private storage)
            //   Engine files are bundled inside the APK — not on the filesystem.
            //   On first launch, extract them from APK assets to data_dir.
            //   engine_dir = data_dir (both point to the same writable directory)
            let (engine_dir, data_dir) = if cfg!(target_os = "android") || cfg!(target_os = "ios") {
                let dir = app.path().app_data_dir()
                    .map_err(|e| format!("Failed to resolve app data dir: {}", e))?
                    .join("engine");
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("Failed to create data directory: {}", e))?;

                // Extract bundled engine files from APK on first launch.
                // On Android, bundle.resources are inside the APK (not on filesystem).
                // We use Tauri's fs plugin to read them and write to the writable dir.
                if !dir.join("player.html").exists() {
                    app_state::extract_engine_files(&dir)
                        .map_err(|e| format!("Failed to extract engine files: {}", e))?;
                }

                // On mobile, both dirs point to the same writable location
                (dir.clone(), dir)
            } else {
                // Desktop: in dev mode, serve directly from source engine/ so edits
                // are reflected immediately without manual copy to target/debug/engine/.
                // In release, use resource_dir/engine (bundled by installer).
                let engine_dir = if cfg!(debug_assertions) {
                    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                    // SAFETY: CARGO_MANIFEST_DIR is a compile-time path that always has a parent
                    manifest_dir.parent().unwrap().join("engine")
                } else {
                    app.path()
                        .resource_dir()
                        .ok()
                        .map(|d| d.join("engine"))
                        .filter(|d| d.exists())
                        .unwrap_or_else(|| {
                            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                            // SAFETY: CARGO_MANIFEST_DIR is a compile-time path that always has a parent
                            manifest_dir.parent().unwrap().join("engine")
                        })
                };
                // In dev mode, data_dir stays in target/debug/engine for runtime
                // data (cases, defaults, config). In release, same as engine_dir.
                let data_dir = if cfg!(debug_assertions) {
                    app.path()
                        .resource_dir()
                        .ok()
                        .map(|d| d.join("engine"))
                        .filter(|d| d.exists())
                        .unwrap_or_else(|| engine_dir.clone())
                } else {
                    engine_dir.clone()
                };
                (engine_dir, data_dir)
            };

            // Load user config from writable data dir
            let app_config = config::load_config(&data_dir);
            log::info!("Loaded config: {:?}", app_config);

            // Start the migration server only when needed.
            // Once migration_complete = true the server is pure overhead — skip it.
            let migration_server = if app_config.migration_complete {
                log::debug!("Migration already complete — skipping tiny_http server startup");
                None
            } else {
                match server::start_server(server::ServerConfig {
                    engine_dir: engine_dir.clone(),
                    data_dir: data_dir.clone(),
                }) {
                    Ok(ms) => {
                        log::info!("Migration server started on port {}", ms.port());
                        // Write port file so the JS migration script can find the server URL.
                        let port_file = data_dir.join(".server_port");
                        let _ = fs::write(&port_file, ms.port().to_string());
                        Some(ms)
                    }
                    Err(e) => {
                        log::warn!("Migration server failed to start: {} — migration will be skipped", e);
                        None
                    }
                }
            };

            let server_port = migration_server.as_ref().map_or(0, |ms| ms.port());

            log::info!("Engine directory: {}", engine_dir.display());
            log::info!("Data directory: {}", data_dir.display());

            // Shared HTTP client — reuses connection pool across all download commands
            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .pool_max_idle_per_host(10)
                .build()
                .unwrap_or_default();

            // Store immutable paths (no lock needed — Tauri wraps in Arc)
            app.manage(AppPaths {
                server_port,
                migration_server,
                engine_dir,
                data_dir,
                cancel_flag: Arc::new(AtomicBool::new(false)),
                http_client,
            });
            // Store mutable config (locked only by settings commands)
            app.manage(MutableConfig(Mutex::new(app_config)));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // game
            commands::game::open_game,
            commands::game::get_server_url,
            commands::game::get_migration_server_url,
            commands::game::debug_check_file,
            // download
            commands::download::fetch_case_info,
            commands::download::download_case,
            commands::download::download_sequence,
            commands::download::update_case,
            commands::download::retry_failed_assets,
            commands::download::cancel_download,
            // cases
            commands::cases::list_cases,
            commands::cases::get_missing_assets,
            commands::cases::delete_case,
            // saves
            commands::saves::backup_saves,
            commands::saves::load_saves_backup,
            commands::saves::read_saves_for_export,
            commands::saves::find_latest_save,
            // collections
            commands::collections::list_collections,
            commands::collections::create_collection,
            commands::collections::update_collection,
            commands::collections::delete_collection,
            commands::collections::get_collection,
            commands::collections::add_to_collection,
            // settings
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_storage_info,
            commands::settings::optimize_storage,
            commands::settings::open_data_dir,
            // dialogs
            commands::dialogs::pick_folder,
            commands::dialogs::pick_import_file,
            commands::dialogs::pick_export_file,
            commands::dialogs::pick_export_plugin_file,
            commands::dialogs::pick_export_save_file,
            // import
            commands::import::import_case,
            commands::import::import_save,
            // export
            commands::export::export_case,
            commands::export::export_sequence,
            commands::export::export_collection,
            commands::export::export_save,
            // plugins
            commands::plugins::import_plugin,
            commands::plugins::import_aaoplug_global,
            commands::plugins::attach_plugin_code,
            commands::plugins::attach_global_plugin_code,
            commands::plugins::list_plugins,
            commands::plugins::remove_plugin,
            commands::plugins::toggle_plugin,
            commands::plugins::list_global_plugins,
            commands::plugins::remove_global_plugin,
            commands::plugins::toggle_global_plugin,
            commands::plugins::toggle_plugin_for_scope,
            commands::plugins::check_plugin_duplicate,
            commands::plugins::set_global_plugin_params,
            commands::plugins::get_plugin_params,
            commands::plugins::get_plugin_descriptors,
            commands::plugins::export_case_plugins,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
