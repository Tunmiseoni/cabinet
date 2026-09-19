mod commands;
mod config;
mod control;
mod discovery;
mod launcher;
mod player;
mod results;
mod room;
mod roms;
mod scores;
mod service;
mod session;
mod tailscale;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(session::Session::default())
        .manage(service::RoomService::default())
        .setup(|app| {
            use tauri::Manager;
            let dir = app.path().app_config_dir()?;
            app.manage(scores::ScoreBoard::load(dir.join("scores.json")));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::list_peers,
            commands::peer_health,
            commands::peers_health,
            commands::list_roms,
            commands::list_rooms,
            commands::launcher_info,
            commands::overlay_status,
            commands::get_scores,
            commands::reset_scores,
            commands::launch_match,
            commands::launch_dev_pair,
            commands::stop_match,
            commands::match_status,
            commands::host_room,
            commands::join_room,
            commands::leave_room,
            commands::room_enqueue,
            commands::room_leave_queue,
            commands::report_room_result,
            commands::room_state,
            commands::room_secret,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
