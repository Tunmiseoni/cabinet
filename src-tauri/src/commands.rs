use crate::config::{self, Config};
use crate::constants;
use crate::discovery::{self, DiscoveredRoom};
use crate::launcher::InstallInfo;
use crate::probe;
use crate::provider::{self, Capabilities, MatchRequest, Provider, ProviderKind, Role};
use crate::results::{self, OverlayStatus};
use crate::retroarch;
use crate::roms::{self, RomIndex};
use crate::room::RoomState;
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

pub(crate) fn load_config(app: &AppHandle) -> Result<Config, String> {
    Ok(Config::load(&config_file(app)?))
}

pub(crate) fn provider_for(app: &AppHandle, dev: bool) -> Result<Box<dyn Provider>, String> {
    let cfg = load_config(app)?;
    provider::resolve_provider(app, &cfg, dev)
}

pub(crate) fn config_and_provider(
    app: &AppHandle,
    dev: bool,
) -> Result<(Config, Box<dyn Provider>), String> {
    let cfg = load_config(app)?;
    let provider = provider::resolve_provider(app, &cfg, dev)?;
    Ok((cfg, provider))
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<Config, String> {
    load_config(&app)
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
    let cfg = load_config(&app)?;
    let binary = tailscale::resolve_binary(&cfg)?;
    tailscale::status(&binary)
}

#[tauri::command(async)]
pub fn peer_health(app: AppHandle, ip: String) -> Result<PeerHealth, String> {
    let cfg = load_config(&app)?;
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::ping(&binary, &ip))
}

#[tauri::command(async)]
pub fn peers_health(app: AppHandle, ips: Vec<String>) -> Result<Vec<PeerHealth>, String> {
    let cfg = load_config(&app)?;
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::ping_many(&binary, &ips))
}

#[tauri::command(async)]
pub fn list_roms(app: AppHandle) -> Result<RomIndex, String> {
    let cfg = load_config(&app)?;
    Ok(roms::index(&cfg))
}

#[tauri::command(async)]
pub fn list_rooms(app: AppHandle) -> Result<Vec<DiscoveredRoom>, String> {
    let cfg = load_config(&app)?;
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
        constants::DISCOVERY_PROBE_TIMEOUT,
    ))
}

fn rom_file(cfg: &Config, rom: &str) -> Result<PathBuf, String> {
    let dir = roms::resolve_rom_dir(cfg)
        .ok_or_else(|| "no ROM directory found — set one in settings".to_string())?;
    let path = dir.join(format!("{rom}.zip"));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("ROM file not found: {}", path.display()))
    }
}

fn optional_rom_file(cfg: &Config, provider: &dyn Provider, rom: &str) -> Result<PathBuf, String> {
    if provider.kind() == ProviderKind::Retroarch {
        return rom_file(cfg, rom);
    }
    Ok(roms::resolve_rom_dir(cfg)
        .map(|dir| dir.join(format!("{rom}.zip")))
        .unwrap_or_default())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub kind: ProviderKind,
    pub install: InstallInfo,
    pub capabilities: Capabilities,
}

#[tauri::command(async)]
pub fn launcher_info(app: AppHandle) -> Result<ProviderInfo, String> {
    let provider = provider_for(&app, false)?;
    Ok(ProviderInfo {
        kind: provider.kind(),
        install: provider.detect()?,
        capabilities: provider.capabilities(),
    })
}

#[tauri::command(async)]
pub fn overlay_status(app: AppHandle) -> Result<OverlayStatus, String> {
    let provider = provider_for(&app, false)?;
    if !provider.capabilities().overlay_results {
        return Ok(OverlayStatus {
            enabled: false,
            ini_path: String::new(),
        });
    }
    Ok(results::overlay_status(&provider.emulator_dir()))
}

#[tauri::command(async)]
pub fn parity_status(
    app: AppHandle,
    rom: String,
) -> Result<Option<retroarch::ParityStatus>, String> {
    let (cfg, provider) = config_and_provider(&app, false)?;
    if rom.trim().is_empty() {
        return Ok(None);
    }
    let rom_path = optional_rom_file(&cfg, provider.as_ref(), &rom)?;
    provider.parity(&rom_path)
}

#[tauri::command(async)]
pub fn enable_overlay(app: AppHandle) -> Result<OverlayStatus, String> {
    if session::status(&app).status == "running" {
        return Err("stop the match before changing FightCade settings".to_string());
    }
    let cfg = load_config(&app)?;
    let launcher = provider::resolve_launcher(&cfg, false)?;
    results::enable_overlay(&launcher.emulator_dir())
        .map_err(|err| format!("cannot update FightCade config: {err}"))
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
    pub role: Role,
    #[serde(default)]
    pub dev: bool,
    #[serde(default)]
    pub force: bool,
}

fn effective_peer(dev: bool, peer_ip: &str) -> String {
    if dev && peer_ip.trim().is_empty() {
        "127.0.0.1".to_string()
    } else {
        peer_ip.to_string()
    }
}

fn plan_for(
    provider: &dyn Provider,
    cfg: &Config,
    role: Role,
    rom: &str,
    peer_ip: &str,
) -> Result<Plan, String> {
    let rom_path = optional_rom_file(cfg, provider, rom)?;
    if let Some(status) = provider.parity(&rom_path)? {
        log::info!(
            "parity for {rom}: ok={} detail={}",
            status.ok,
            status.detail
        );
        if !status.ok {
            return Err(status.detail);
        }
    }
    let request = MatchRequest {
        role,
        rom,
        rom_path: &rom_path,
        peer_ip,
    };
    let spec = provider.spec(&request)?;
    Ok(Plan {
        spec,
        role,
        rom: rom.to_string(),
        peer_ip: peer_ip.to_string(),
        port: provider.port(role),
        side: role.side(),
    })
}

fn preflight(
    provider: &dyn Provider,
    role: Role,
    peer_ip: &str,
    force: bool,
) -> Result<(), String> {
    if provider.kind() != ProviderKind::Retroarch {
        return Ok(());
    }
    let Some(port) = provider.port(role) else {
        return Ok(());
    };
    match role {
        Role::P1 => match probe::port_is_free(port) {
            Ok(()) => {
                log::info!("preflight host: netplay port {port} is free");
                Ok(())
            }
            Err(err) => {
                log::warn!("preflight host: {err}");
                if force {
                    Ok(())
                } else {
                    Err(format!("{err} — is another instance already hosting?"))
                }
            }
        },
        Role::P2 | Role::Spectator => {
            let result = probe::probe(peer_ip, port, constants::PROBE_TIMEOUT);
            log::info!(
                "preflight {}: {peer_ip}:{port} reachable={} latency={:?} error={:?}",
                role.label(),
                result.reachable,
                result.latency_ms,
                result.error
            );
            if result.reachable || force {
                Ok(())
            } else {
                let detail = result
                    .error
                    .map(|err| format!(" ({err})"))
                    .unwrap_or_default();
                Err(format!(
                    "host not reachable on {peer_ip}:{port}{detail} — start the host first, confirm the peer IP, and allow inbound TCP {port} on the host"
                ))
            }
        }
    }
}

#[tauri::command(async)]
pub fn launch_match(app: AppHandle, request: LaunchRequest) -> Result<MatchState, String> {
    let (cfg, provider) = config_and_provider(&app, request.dev)?;
    log::info!(
        "launch request: provider={:?} role={} rom={} peer={:?} dev={} force={}",
        provider.kind(),
        request.role.label(),
        request.rom,
        request.peer_ip,
        request.dev,
        request.force
    );
    let install = provider.detect()?;
    if !install.installed {
        log::error!("emulator not found — {}", install.detail);
        return Err(format!("emulator not found — {}", install.detail));
    }
    if request.role == Role::Spectator && !provider.capabilities().spectate {
        return Err("this provider cannot spectate".into());
    }
    let peer_ip = effective_peer(request.dev, &request.peer_ip);
    if !request.dev {
        preflight(provider.as_ref(), request.role, &peer_ip, request.force)?;
    }
    let plan = plan_for(
        provider.as_ref(),
        &cfg,
        request.role,
        &request.rom,
        &peer_ip,
    )?;
    let overlay = provider.capabilities().overlay_results;
    let wait_for_host = request.dev
        && provider.kind() == ProviderKind::Retroarch
        && matches!(request.role, Role::P2 | Role::Spectator);
    session::launch(
        &app,
        &plan,
        session::LaunchOptions {
            dev: request.dev,
            wait_for_host,
            overlay,
            track_scores: overlay,
            peer_display: peer_ip,
        },
    )
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
    let (cfg, provider) = config_and_provider(&app, true)?;
    log::info!("launch dev pair: provider={:?} rom={rom}", provider.kind());
    let install = provider.detect()?;
    if !install.installed {
        return Err(format!("emulator not found — {}", install.detail));
    }
    if !provider.capabilities().dev_pair {
        return Err("this provider has no dev pair".into());
    }
    let plans = vec![
        plan_for(provider.as_ref(), &cfg, Role::P1, &rom, "127.0.0.1")?,
        plan_for(provider.as_ref(), &cfg, Role::P2, &rom, "127.0.0.1")?,
    ];
    session::launch_many(
        &app,
        &plans,
        session::LaunchOptions {
            dev: true,
            wait_for_host: provider.kind() == ProviderKind::Retroarch,
            overlay: false,
            track_scores: false,
            peer_display: "127.0.0.1 (P1↔P2)".to_string(),
        },
    )
}

#[tauri::command(async)]
pub fn host_room(app: AppHandle, rom: String, secret: Option<String>) -> Result<RoomState, String> {
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
pub fn report_room_result(app: AppHandle, match_id: String, won: bool) -> Result<(), String> {
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
    log::debug!(
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
pub fn cabinet_place(app: AppHandle, window_id: u32, rect: Rect) -> Result<PlacementMode, String> {
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

#[tauri::command(async)]
pub fn probe_port(ip: String, port: u16, timeout_ms: Option<u64>) -> probe::PortProbe {
    let timeout = Duration::from_millis(
        timeout_ms
            .unwrap_or(constants::PROBE_DEFAULT_TIMEOUT_MS)
            .clamp(
                constants::PROBE_TIMEOUT_MIN_MS,
                constants::PROBE_TIMEOUT_MAX_MS,
            ),
    );
    probe::probe(&ip, port, timeout)
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
        assert!(
            crate::launcher::MatchConfig::new("rom".into(), effective_peer(false, ""), 0).is_err()
        );
    }

    #[test]
    fn dev_launch_makes_an_empty_peer_config_valid() {
        let config =
            crate::launcher::MatchConfig::new("rom".into(), effective_peer(true, ""), 0).unwrap();
        assert_eq!(config.peer_ip, "127.0.0.1");
    }
}
