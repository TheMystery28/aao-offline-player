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
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
                    .expect("failed to resolve app data dir")
                    .join("engine");
                fs::create_dir_all(&dir)
                    .expect("failed to create data directory");

                // Extract bundled engine files from APK on first launch.
                // On Android, bundle.resources are inside the APK (not on filesystem).
                // We use Tauri's fs plugin to read them and write to the writable dir.
                if !dir.join("player.html").exists() {
                    app_state::extract_engine_files(&dir)
                        .expect("failed to extract engine files");
                }

                // On mobile, both dirs point to the same writable location
                (dir.clone(), dir)
            } else {
                // Desktop: in dev mode, serve directly from source engine/ so edits
                // are reflected immediately without manual copy to target/debug/engine/.
                // In release, use resource_dir/engine (bundled by installer).
                let engine_dir = if cfg!(debug_assertions) {
                    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                    manifest_dir.parent().unwrap().join("engine")
                } else {
                    app.path()
                        .resource_dir()
                        .ok()
                        .map(|d| d.join("engine"))
                        .filter(|d| d.exists())
                        .unwrap_or_else(|| {
                            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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
            });

            debug_log!("Asset server started on http://localhost:{}", port);
            debug_log!("Engine directory: {}", engine_dir.display());
            debug_log!("Data directory: {}", data_dir.display());

            // Write port file so external scripts (e.g. test runner) can find the server
            let port_file = data_dir.join(".server_port");
            let _ = fs::write(&port_file, port.to_string());

            // Store state for commands
            app.manage(Mutex::new(AppState {
                server_port: port,
                engine_dir,
                data_dir,
                config: app_config,
                cancel_flag: Arc::new(AtomicBool::new(false)),
            }));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_game,
            get_server_url,
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
            promote_plugin_to_global,
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
