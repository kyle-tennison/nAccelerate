mod cancel;
mod commands;
mod credentials;
mod events;
mod job;
pub mod notebook;
mod remote;
mod server;
mod settings;
mod solver;
mod stats;
mod transfer;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::JobState::default())
        .invoke_handler(tauri::generate_handler![
            commands::load_settings,
            commands::save_settings,
            commands::test_connection,
            commands::save_password,
            commands::has_password,
            commands::inspect_folder,
            commands::start_job,
            commands::cancel_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running nAccelerate");
}
