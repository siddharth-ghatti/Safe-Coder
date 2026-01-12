// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::get_server_url,
            commands::is_server_ready,
            commands::select_directory,
            commands::open_in_explorer,
        ])
        .on_window_event(|_window, event| {
            // Clean up server when window is closed
            if let tauri::WindowEvent::Destroyed = event {
                let _ = commands::stop_server();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
