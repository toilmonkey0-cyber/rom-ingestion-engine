// src-tauri/src/lib.rs
pub mod chdman;
pub mod classifier;
pub mod commands;
pub mod models;
pub mod organizer;
pub mod plan_builder;
pub mod scanner;

pub use chdman::*;
pub use classifier::jev::*;
pub use commands::*;
pub use organizer::*;
pub use plan_builder::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::scan_and_plan,
            commands::execute_plan,
            commands::trash_source_files,
            commands::check_chdman_status,
            commands::download_chdman,
            commands::set_custom_chdman_path,
            commands::resolve_game_artwork,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
