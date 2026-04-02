#[macro_use]
mod app_state;
mod collections;
mod commands;
mod config;
mod downloader;
mod importer;
mod server;
pub mod utils;

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;

use app_state::AppState;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    builder
        .register_asynchronous_uri_scheme_protocol("aao", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || {
                let state = app.state::<Mutex<AppState>>();
                let guard = match state.lock() {
                    Ok(s) => s,
                    Err(_) => {
                        // SAFETY: Response::builder() with a valid status and Vec<u8> body cannot fail
                        let resp = tauri::http::Response::builder()
                            .status(503)
                            .body(b"Service Unavailable".to_vec())
                            .unwrap();
                        responder.respond(resp);
                        return;
                    }
                };
                let config = server::ServerConfig {
                    engine_dir: guard.engine_dir.clone(),
                    data_dir: guard.data_dir.clone(),
                };
                drop(guard); // Release lock before file I/O

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

            // Store state for commands
            app.manage(Mutex::new(AppState {
                server_port: port,
                engine_dir,
                data_dir,
                config: app_config,
                cancel_flag: Arc::new(AtomicBool::new(false)),
                http_client,
            }));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_game,
            get_server_url,
            get_migration_server_url,
            debug_check_file,
            fetch_case_info,
            download_case,
            download_sequence,
            update_case,
            retry_failed_assets,
            list_cases,
            delete_case,
            backup_saves,
            load_saves_backup,
            read_saves_for_export,
            find_latest_save,
            list_collections,
            create_collection,
            update_collection,
            delete_collection,
            get_collection,
            add_to_collection,
            export_collection,
            get_settings,
            save_settings,
            get_storage_info,
            clear_unused_defaults,
            optimize_storage,
            open_data_dir,
            pick_folder,
            pick_import_file,
            import_case,
            import_plugin,
            import_aaoplug_global,
            attach_plugin_code,
            list_plugins,
            remove_plugin,
            toggle_plugin,
            list_global_plugins,
            attach_global_plugin_code,
            remove_global_plugin,
            toggle_global_plugin,
            toggle_plugin_for_scope,
            check_plugin_duplicate,
            set_global_plugin_params,
            get_plugin_params,
            get_plugin_descriptors,
            export_case_plugins,
            cancel_download,
            pick_export_plugin_file,
            export_save,
            import_save,
            pick_export_save_file,
            pick_export_file,
            export_case,
            export_sequence
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
