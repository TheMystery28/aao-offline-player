//! Tauri commands exposed to the frontend.
//!
//! This module acts as the entry point for all frontend-to-backend communication.
//! Commands are grouped into submodules based on their functional area (e.g.,
//! case management, downloading, settings).
//!
//! All commands marked with `#[tauri::command]` are registered in `lib.rs`
//! and can be invoked from the frontend using `window.__TAURI__.core.invoke`.

pub mod game;
pub mod download;
pub mod cases;
pub mod saves;
pub mod collections;
pub mod settings;
pub mod dialogs;
pub mod import;
pub mod export;
pub mod plugins;
