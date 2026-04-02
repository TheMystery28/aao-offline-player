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
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("aao", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || {
                let paths = app.state::<AppPaths>();
                let config = server::ServerConfig {
                    engine_dir: paths.engine_dir.clone(),
                    data_dir: paths.data_dir.clone(),
                };

                let method = request.method().as_str();
                let url_path = request.uri().path();
                let range = request
                    .headers()
                    .get("range")
                    .and_then(|v| v.to_str().ok());

                let result = server::serve_file(&config, url_path, method, range);
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
            debug_log!("Loaded config: {:?}", app_config);

            // Start the custom asset server
            let port = server::start_server(server::ServerConfig {
                engine_dir: engine_dir.clone(),
                data_dir: data_dir.clone(),
            }).map_err(|e| format!("Asset server failed: {}", e))?;

            debug_log!("Asset server started on http://localhost:{}", port);
            debug_log!("Engine directory: {}", engine_dir.display());
            debug_log!("Data directory: {}", data_dir.display());

            // Write port file so external scripts (e.g. test runner) can find the server
            let port_file = data_dir.join(".server_port");
            let _ = fs::write(&port_file, port.to_string());

            // Shared HTTP client — reuses connection pool across all download commands
            let http_client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .pool_max_idle_per_host(10)
                .build()
                .unwrap_or_default();

            // Store immutable paths (no lock needed — Tauri wraps in Arc)
            app.manage(AppPaths {
                server_port: port,
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
            commands::settings::clear_unused_defaults,
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
