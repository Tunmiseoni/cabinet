use crate::config::{self, Config};
use crate::discovery::{self, DiscoveredRoom};
#[cfg(target_os = "linux")]
use crate::launcher::linux::LinuxLauncher;
#[cfg(target_os = "macos")]
use crate::launcher::macos::{MacosLauncher, DEFAULT_APP_DIR};
#[cfg(target_os = "windows")]
use crate::launcher::windows::WindowsLauncher;
use crate::launcher::{InstallInfo, Launcher, MatchConfig};
use crate::results::{self, OverlayStatus};
use crate::room::RoomState;
use crate::roms::{self, RomIndex};
use crate::scores::{ScoreBoard, Snapshot};
use crate::service::RoomService;
use crate::session::{self, MatchState, Plan};
use crate::tailscale::{self, PeerHealth, Tailnet};
use crate::windowing::{self, PlacementMode, Rect, WindowInfo};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub(crate) fn config_file(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|err| format!("cannot resolve config dir: {err}"))?;
    Ok(config::config_path(dir))
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<Config, String> {
    Ok(Config::load(&config_file(&app)?))
}

#[tauri::command]
pub fn set_config(app: AppHandle, config: Config) -> Result<Config, String> {
    let path = config_file(&app)?;
    config
        .save(&path)
        .map_err(|err| format!("cannot save config: {err}"))?;
    Ok(config)
}

#[tauri::command(async)]
pub fn list_peers(app: AppHandle) -> Result<Tailnet, String> {
    let cfg = Config::load(&config_file(&app)?);
    let binary = tailscale::resolve_binary(&cfg)?;
    tailscale::status(&binary)
}

#[tauri::command(async)]
pub fn peer_health(app: AppHandle, ip: String) -> Result<PeerHealth, String> {
    let cfg = Config::load(&config_file(&app)?);
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::ping(&binary, &ip))
}

#[tauri::command(async)]
pub fn peers_health(app: AppHandle, ips: Vec<String>) -> Result<Vec<PeerHealth>, String> {
    let cfg = Config::load(&config_file(&app)?);
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::ping_many(&binary, &ips))
}

#[tauri::command(async)]
pub fn list_roms(app: AppHandle) -> Result<RomIndex, String> {
    let cfg = Config::load(&config_file(&app)?);
    Ok(roms::index(&cfg))
}

#[tauri::command(async)]
pub fn list_rooms(app: AppHandle) -> Result<Vec<DiscoveredRoom>, String> {
    let cfg = Config::load(&config_file(&app)?);
    let binary = tailscale::resolve_binary(&cfg)?;
    let tailnet = tailscale::status(&binary)?;
    let ips: Vec<String> = tailnet
        .peers
        .iter()
        .filter(|peer| peer.online)
        .map(|peer| peer.ip.clone())
        .collect();
    Ok(discovery::probe_many(
        &ips,
        cfg.discovery_port,
        Duration::from_millis(400),
    ))
}

#[cfg(target_os = "macos")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    let app_dir = cfg
        .fightcade_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_APP_DIR));
    let launcher = if dev {
        MacosLauncher::loopback_with(app_dir)
    } else {
        MacosLauncher::new(app_dir)
    };
    Ok(Box::new(launcher))
}

#[cfg(target_os = "linux")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    let override_dir = cfg.fightcade_dir.clone().map(PathBuf::from);
    let launcher = LinuxLauncher::detect(override_dir);
    let launcher = if dev { launcher.loopback() } else { launcher };
    Ok(Box::new(launcher))
}

#[cfg(target_os = "windows")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    let override_dir = cfg.fightcade_dir.clone().map(PathBuf::from);
    let launcher = WindowsLauncher::detect(override_dir);
    let launcher = if dev { launcher.loopback() } else { launcher };
    Ok(Box::new(launcher))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    let _ = (cfg, dev);
    Err("this build only ships the macOS, Linux, and Windows launchers".into())
}

#[tauri::command(async)]
pub fn launcher_info(app: AppHandle) -> Result<InstallInfo, String> {
    let cfg = Config::load(&config_file(&app)?);
    resolve_launcher(&cfg, false)?.detect()
}

#[tauri::command(async)]
pub fn overlay_status(app: AppHandle) -> Result<OverlayStatus, String> {
    let cfg = Config::load(&config_file(&app)?);
    let launcher = resolve_launcher(&cfg, false)?;
    Ok(results::overlay_status(&launcher.emulator_dir()))
}

#[tauri::command]
pub fn get_scores(app: AppHandle) -> Snapshot {
    app.state::<ScoreBoard>().snapshot()
}

#[tauri::command]
pub fn reset_scores(app: AppHandle) -> Snapshot {
    app.state::<ScoreBoard>().reset()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub rom: String,
    pub peer_ip: String,
    pub side: u8,
    #[serde(default)]
    pub dev: bool,
}

fn effective_peer(dev: bool, peer_ip: &str) -> String {
    if dev && peer_ip.trim().is_empty() {
        "127.0.0.1".to_string()
    } else {
        peer_ip.to_string()
    }
}

#[tauri::command(async)]
pub fn launch_match(app: AppHandle, request: LaunchRequest) -> Result<MatchState, String> {
    let cfg = Config::load(&config_file(&app)?);
    let launcher = resolve_launcher(&cfg, request.dev)?;
    let install = launcher.detect()?;
    if !install.installed {
        return Err(format!("emulator not found — {}", install.detail));
    }
    let peer_ip = effective_peer(request.dev, &request.peer_ip);
    let config = MatchConfig::new(request.rom, peer_ip, request.side)?;
    let spec = launcher.spec(&config)?;
    session::launch(&app, &spec, &config, request.dev, !request.dev)
}

#[tauri::command(async)]
pub fn stop_match(app: AppHandle) -> Result<MatchState, String> {
    session::stop(&app)
}

#[tauri::command]
pub fn match_status(app: AppHandle) -> MatchState {
    session::status(&app)
}

#[tauri::command(async)]
pub fn launch_dev_pair(app: AppHandle, rom: String) -> Result<MatchState, String> {
    let cfg = Config::load(&config_file(&app)?);
    let launcher = resolve_launcher(&cfg, true)?;
    let install = launcher.detect()?;
    if !install.installed {
        return Err(format!("emulator not found — {}", install.detail));
    }
    let p1 = MatchConfig::new(rom.clone(), "127.0.0.1".into(), 0)?;
    let p2 = MatchConfig::new(rom, "127.0.0.1".into(), 1)?;
    let plans = vec![
        Plan {
            spec: launcher.spec(&p1)?,
            config: p1,
        },
        Plan {
            spec: launcher.spec(&p2)?,
            config: p2,
        },
    ];
    session::launch_many(&app, &plans, true, false, "127.0.0.1 (P1↔P2)".to_string())
}

#[tauri::command(async)]
pub fn host_room(
    app: AppHandle,
    rom: String,
    secret: Option<String>,
) -> Result<RoomState, String> {
    RoomService::host(&app, rom, secret)
}

#[tauri::command(async)]
pub fn join_room(
    app: AppHandle,
    ip: String,
    room_id: String,
    secret: Option<String>,
) -> Result<RoomState, String> {
    RoomService::join(&app, ip, room_id, secret)
}

#[tauri::command(async)]
pub fn leave_room(app: AppHandle) -> Result<(), String> {
    RoomService::leave(&app)
}

#[tauri::command(async)]
pub fn room_enqueue(app: AppHandle) -> Result<(), String> {
    RoomService::enqueue(&app)
}

#[tauri::command(async)]
pub fn room_leave_queue(app: AppHandle) -> Result<(), String> {
    RoomService::leave_queue(&app)
}

#[tauri::command(async)]
pub fn report_room_result(
    app: AppHandle,
    match_id: String,
    won: bool,
) -> Result<(), String> {
    RoomService::report_result(&app, match_id, won)
}

#[tauri::command]
pub fn room_state(app: AppHandle) -> Option<RoomState> {
    app.state::<RoomService>().state()
}

#[tauri::command]
pub fn room_secret(app: AppHandle) -> Option<String> {
    app.state::<RoomService>().secret()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CabinetStatus {
    pub platform: String,
    pub supported: bool,
    pub permission: windowing::Permission,
    pub mode: PlacementMode,
    pub detail: String,
    pub owner_pids: Vec<i32>,
    pub windows: Vec<WindowInfo>,
}

pub(crate) fn session_pids(app: &AppHandle) -> Vec<i32> {
    session::status(app)
        .instances
        .iter()
        .filter_map(|instance| instance.pid.map(|pid| pid as i32))
        .collect()
}

#[tauri::command(async)]
pub fn cabinet_status(app: AppHandle) -> Result<CabinetStatus, String> {
    let host = &app.state::<windowing::Host>().0;
    let status = host.status();
    let owner_pids = session_pids(&app);
    let windows = host.list_windows(&owner_pids).unwrap_or_default();
    eprintln!(
        "cabinet_status: platform={} supported={} permission={:?} mode={:?} pids={:?} windows={}",
        status.platform,
        status.supported,
        status.permission,
        status.mode,
        owner_pids,
        windows.len()
    );
    Ok(CabinetStatus {
        platform: status.platform,
        supported: status.supported,
        permission: status.permission,
        mode: status.mode,
        detail: status.detail,
        owner_pids,
        windows,
    })
}

#[tauri::command(async)]
pub fn cabinet_place(
    app: AppHandle,
    window_id: u32,
    rect: Rect,
) -> Result<PlacementMode, String> {
    let host = &app.state::<windowing::Host>().0;
    let allowed = host.list_windows(&session_pids(&app))?;
    if !allowed.iter().any(|window| window.id == window_id) {
        return Err(format!(
            "window {window_id} is not part of the current match"
        ));
    }
    host.place(window_id, rect.round())
}

#[tauri::command(async)]
pub fn cabinet_release(app: AppHandle, window_id: u32) -> Result<(), String> {
    app.state::<windowing::Host>().0.release(window_id)
}

#[tauri::command(async)]
pub fn cabinet_request_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::windowing::macos::prompt_permission())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("no permission is required on this platform".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_launch_defaults_an_empty_peer_to_loopback() {
        assert_eq!(effective_peer(true, ""), "127.0.0.1");
        assert_eq!(effective_peer(true, "   "), "127.0.0.1");
    }

    #[test]
    fn non_dev_launch_keeps_the_peer_and_still_requires_one() {
        assert_eq!(effective_peer(false, "100.64.0.2"), "100.64.0.2");
        assert!(MatchConfig::new("rom".into(), effective_peer(false, ""), 0).is_err());
    }

    #[test]
    fn dev_launch_makes_an_empty_peer_config_valid() {
        let config = MatchConfig::new("rom".into(), effective_peer(true, ""), 0).unwrap();
        assert_eq!(config.peer_ip, "127.0.0.1");
    }
}
