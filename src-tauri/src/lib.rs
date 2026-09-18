mod commands;
mod config;
mod launcher;
mod roms;
mod session;
mod tailscale;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(session::Session::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::list_peers,
            commands::peer_health,
            commands::peers_health,
            commands::list_roms,
            commands::launcher_info,
            commands::launch_match,
            commands::launch_dev_pair,
            commands::stop_match,
            commands::match_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
