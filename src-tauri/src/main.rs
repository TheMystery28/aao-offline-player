//! Main entry point for the AAO Offline Player backend.
//! 
//! This crate provides the Tauri backend logic, including case downloading,
//! asset management, and a local HTTP server for serving the player engine.

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Standard Rust main function.
/// Delegates all execution logic to the `run` function in the library crate.
fn main() {
    aao_offline_player_lib::run()
}
