// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod error;
mod hotkey;
mod recording;
mod replay;

use commands::*;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            list_recordings,
            play_recording,
            stop_playback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

