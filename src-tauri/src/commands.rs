use crate::config::{self, Config};
use crate::discovery::{self, DiscoveredRoom};
#[cfg(target_os = "linux")]
use crate::launcher::linux::LinuxLauncher;
#[cfg(target_os = "macos")]
use crate::launcher::macos::{MacosLauncher, DEFAULT_APP_DIR};
#[cfg(target_os = "windows")]
use crate::launcher::windows::WindowsLauncher;
use crate::launcher::{retroarch, InstallInfo, Launcher};
use crate::logging;
use crate::probe;
use crate::provider::{
    self, Capabilities, FightCadeProvider, MatchRequest, Provider, ProviderKind, Role,
};
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
use tauri_plugin_opener::OpenerExt;

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
    let rom_dir = roms::resolve_rom_dir(cfg);
    let launcher = LinuxLauncher::detect(override_dir, rom_dir);
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

pub(crate) fn resolve_provider(
    app: &AppHandle,
    cfg: &Config,
    dev: bool,
) -> Result<Box<dyn Provider>, String> {
    match cfg.provider {
        ProviderKind::Fightcade => Ok(Box::new(FightCadeProvider::new(resolve_launcher(
            cfg, dev,
        )?))),
        ProviderKind::Retroarch => {
            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|err| format!("cannot resolve config dir: {err}"))?;
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|err| format!("cannot resolve data dir: {err}"))?;
            Ok(Box::new(provider::retroarch_provider(
                cfg, &config_dir, &data_dir, dev,
            )))
        }
    }
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
    let cfg = Config::load(&config_file(&app)?);
    let provider = resolve_provider(&app, &cfg, false)?;
    Ok(ProviderInfo {
        kind: provider.kind(),
        install: provider.detect()?,
        capabilities: provider.capabilities(),
    })
}

#[tauri::command(async)]
pub fn overlay_status(app: AppHandle) -> Result<OverlayStatus, String> {
    let cfg = Config::load(&config_file(&app)?);
    let provider = resolve_provider(&app, &cfg, false)?;
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
    let cfg = Config::load(&config_file(&app)?);
    let provider = resolve_provider(&app, &cfg, false)?;
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
    let cfg = Config::load(&config_file(&app)?);
    let launcher = resolve_launcher(&cfg, false)?;
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
            let result = probe::probe(peer_ip, port, Duration::from_secs(3));
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
    let cfg = Config::load(&config_file(&app)?);
    let provider = resolve_provider(&app, &cfg, request.dev)?;
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
    let plan = plan_for(provider.as_ref(), &cfg, request.role, &request.rom, &peer_ip)?;
    let overlay = provider.capabilities().overlay_results;
    let wait_for_host = request.dev
        && provider.kind() == ProviderKind::Retroarch
        && matches!(request.role, Role::P2 | Role::Spectator);
    session::launch(
        &app,
        &plan,
        request.dev,
        wait_for_host,
        overlay,
        overlay,
        peer_ip,
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
    let cfg = Config::load(&config_file(&app)?);
    let provider = resolve_provider(&app, &cfg, true)?;
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
        true,
        provider.kind() == ProviderKind::Retroarch,
        false,
        false,
        "127.0.0.1 (P1↔P2)".to_string(),
    )
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsResult {
    pub path: String,
    pub log_dir: String,
}

#[tauri::command(async)]
pub fn probe_port(ip: String, port: u16, timeout_ms: Option<u64>) -> probe::PortProbe {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(3000).clamp(100, 30_000));
    probe::probe(&ip, port, timeout)
}

#[tauri::command]
pub fn log_dir(app: AppHandle) -> Result<String, String> {
    Ok(logging::app_log_dir(&app)?.to_string_lossy().to_string())
}

#[tauri::command]
pub fn open_logs_dir(app: AppHandle) -> Result<String, String> {
    let dir = logging::app_log_dir(&app)?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|err| format!("cannot open {}: {err}", dir.display()))?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command(async)]
pub fn collect_diagnostics(app: AppHandle) -> Result<DiagnosticsResult, String> {
    let dir = logging::app_log_dir(&app)?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let path = dir.join(format!("diagnostics-{}.txt", logging::utc_stamp()));
    let text = diagnostics_text(&app);
    std::fs::write(&path, text).map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    log::info!("diagnostics written to {}", path.display());
    let _ = app.opener().open_path(dir.to_string_lossy().to_string(), None::<&str>);
    Ok(DiagnosticsResult {
        path: path.to_string_lossy().to_string(),
        log_dir: dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn log_frontend(level: String, message: String, context: Option<String>) {
    use std::str::FromStr;
    let level = log::Level::from_str(&level).unwrap_or(log::Level::Info);
    let suffix = context
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" ({value})"))
        .unwrap_or_default();
    log::log!(target: "webview", level, "{message}{suffix}");
}

fn diagnostics_text(app: &AppHandle) -> String {
    use std::fmt::Write as _;

    let cfg = Config::load(&config_file(app).unwrap_or_default());
    let mut out = String::new();
    let _ = writeln!(out, "The Cabinet diagnostics — {}", logging::utc_stamp());
    let _ = writeln!(out, "version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        out,
        "platform: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    match logging::app_log_dir(app) {
        Ok(dir) => {
            let _ = writeln!(out, "log dir: {}", dir.display());
        }
        Err(err) => {
            let _ = writeln!(out, "log dir error: {err}");
        }
    }

    let _ = writeln!(out, "\n--- config ---");
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&cfg).unwrap_or_else(|_| "<unavailable>".to_string())
    );

    let _ = writeln!(out, "\n--- provider ---");
    match resolve_provider(app, &cfg, false) {
        Ok(provider) => {
            let _ = writeln!(out, "kind: {:?}", provider.kind());
            match provider.detect() {
                Ok(install) => {
                    let _ = writeln!(
                        out,
                        "install: {} installed={} detail={}",
                        install.label, install.installed, install.detail
                    );
                }
                Err(err) => {
                    let _ = writeln!(out, "install error: {err}");
                }
            }
            let caps = provider.capabilities();
            let _ = writeln!(
                out,
                "capabilities: spectate={} overlayResults={} devPair={}",
                caps.spectate, caps.overlay_results, caps.dev_pair
            );
        }
        Err(err) => {
            let _ = writeln!(out, "provider error: {err}");
        }
    }

    let _ = writeln!(out, "\n--- tailnet ---");
    match tailscale::resolve_binary(&cfg) {
        Ok(binary) => match tailscale::status(&binary) {
            Ok(tailnet) => {
                let online = tailnet.peers.iter().filter(|peer| peer.online).count();
                let _ = writeln!(out, "backendState: {}", tailnet.backend_state);
                let _ = writeln!(out, "peers: {} ({online} online)", tailnet.peers.len());
            }
            Err(err) => {
                let _ = writeln!(out, "status error: {err}");
            }
        },
        Err(err) => {
            let _ = writeln!(out, "tailscale binary error: {err}");
        }
    }

    let _ = writeln!(out, "\n--- last match ---");
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&session::status(app))
            .unwrap_or_else(|_| "<unavailable>".to_string())
    );

    let _ = writeln!(out, "\n--- sessions ---");
    match logging::sessions_dir(app) {
        Ok(sessions) => match std::fs::read_dir(&sessions) {
            Ok(entries) => {
                let mut dirs: Vec<PathBuf> = entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())
                    .collect();
                dirs.sort();
                for dir in dirs.iter().rev().take(10) {
                    let _ = writeln!(out, "{}", dir.display());
                    if let Ok(files) = std::fs::read_dir(dir) {
                        for file in files.flatten() {
                            let size = file.metadata().map(|meta| meta.len()).unwrap_or(0);
                            let _ = writeln!(out, "  {} ({size} bytes)", file.path().display());
                        }
                    }
                }
            }
            Err(err) => {
                let _ = writeln!(out, "cannot list sessions: {err}");
            }
        },
        Err(err) => {
            let _ = writeln!(out, "{err}");
        }
    }

    let _ = writeln!(out, "\n--- recent app log (last 200 lines) ---");
    match logging::app_log_path(app) {
        Ok(path) => {
            let _ = writeln!(out, "{}", tail_text(&path, 200));
        }
        Err(err) => {
            let _ = writeln!(out, "{err}");
        }
    }

    out
}

fn tail_text(path: &std::path::Path, max_lines: usize) -> String {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return format!("(no log at {})", path.display());
    };
    let lines: Vec<&str> = raw.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
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
            crate::launcher::MatchConfig::new(
                "rom".into(),
                effective_peer(false, ""),
                0
            )
            .is_err()
        );
    }

    #[test]
    fn dev_launch_makes_an_empty_peer_config_valid() {
        let config =
            crate::launcher::MatchConfig::new("rom".into(), effective_peer(true, ""), 0).unwrap();
        assert_eq!(config.peer_ip, "127.0.0.1");
    }
}
