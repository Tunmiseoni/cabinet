mod commands;
mod config;
mod control;
mod discovery;
mod launcher;
mod player;
mod process;
mod provider;
mod results;
mod room;
mod roms;
mod scores;
mod service;
mod session;
mod tailscale;
mod windowing;

const LEGACY_IDENTIFIER: &str = "com.cabinet.app";
const MIGRATED_FILES: [&str; 3] = ["config.json", "scores.json", "room-ledger.json"];

fn migrate_legacy_config_dir(current: &std::path::Path) {
    let Some(legacy) = current.parent().map(|parent| parent.join(LEGACY_IDENTIFIER)) else {
        return;
    };
    if !legacy.is_dir() {
        return;
    }
    for name in MIGRATED_FILES {
        let src = legacy.join(name);
        let dst = current.join(name);
        if src.is_file() && !dst.exists() {
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(err) = std::fs::copy(&src, &dst) {
                eprintln!("failed to migrate {src:?}: {err}");
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn apply_linux_webkit_workarounds() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_linux_webkit_workarounds();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(session::Session::default())
        .manage(service::RoomService::default())
        .manage(windowing::Host::new())
        .setup(|app| {
            use tauri::Manager;
            let dir = app.path().app_config_dir()?;
            migrate_legacy_config_dir(&dir);
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
            commands::parity_status,
            commands::enable_overlay,
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
            commands::cabinet_status,
            commands::cabinet_place,
            commands::cabinet_release,
            commands::cabinet_request_permission,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "the-cabinet-migrate-{tag}-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&root).ok();
        let legacy = root.join(LEGACY_IDENTIFIER);
        let current = root.join("com.the-cabinet.app");
        std::fs::create_dir_all(&legacy).unwrap();
        (root, legacy, current)
    }

    #[test]
    fn migrates_missing_files_from_legacy_dir() {
        let (root, legacy, current) = scratch("copy");
        std::fs::write(legacy.join("config.json"), "{}").unwrap();
        std::fs::write(legacy.join("scores.json"), "[]").unwrap();

        migrate_legacy_config_dir(&current);

        assert_eq!(
            std::fs::read_to_string(current.join("config.json")).unwrap(),
            "{}"
        );
        assert_eq!(
            std::fs::read_to_string(current.join("scores.json")).unwrap(),
            "[]"
        );
        assert!(!current.join("room-ledger.json").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn does_not_overwrite_existing_files() {
        let (root, legacy, current) = scratch("keep");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(legacy.join("config.json"), "old").unwrap();
        std::fs::write(current.join("config.json"), "new").unwrap();

        migrate_legacy_config_dir(&current);

        assert_eq!(
            std::fs::read_to_string(current.join("config.json")).unwrap(),
            "new"
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
