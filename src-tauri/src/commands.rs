use crate::config::{self, Config};
use crate::launcher::macos::{MacosLauncher, DEFAULT_APP_DIR};
use crate::launcher::{InstallInfo, Launcher, MatchConfig};
use crate::roms::{self, RomIndex};
use crate::session::{self, MatchState, Plan};
use crate::tailscale::{self, PeerHealth, Tailnet};
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn config_file(app: &AppHandle) -> Result<std::path::PathBuf, String> {
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

#[tauri::command]
pub fn list_peers(app: AppHandle) -> Result<Tailnet, String> {
    let cfg = Config::load(&config_file(&app)?);
    let binary = tailscale::resolve_binary(&cfg)?;
    tailscale::status(&binary)
}

#[tauri::command]
pub fn peer_health(app: AppHandle, ip: String) -> Result<PeerHealth, String> {
    let cfg = Config::load(&config_file(&app)?);
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::ping(&binary, &ip))
}

#[tauri::command]
pub fn peers_health(app: AppHandle, ips: Vec<String>) -> Result<Vec<PeerHealth>, String> {
    let cfg = Config::load(&config_file(&app)?);
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::ping_many(&binary, &ips))
}

#[tauri::command]
pub fn list_roms(app: AppHandle) -> Result<RomIndex, String> {
    let cfg = Config::load(&config_file(&app)?);
    Ok(roms::index(&cfg))
}

fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    if !cfg!(target_os = "macos") {
        return Err("this build only ships the macOS launcher so far".into());
    }
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

#[tauri::command]
pub fn launcher_info(app: AppHandle) -> Result<InstallInfo, String> {
    let cfg = Config::load(&config_file(&app)?);
    resolve_launcher(&cfg, false)?.detect()
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

#[tauri::command]
pub fn launch_match(app: AppHandle, request: LaunchRequest) -> Result<MatchState, String> {
    let cfg = Config::load(&config_file(&app)?);
    let launcher = resolve_launcher(&cfg, request.dev)?;
    let install = launcher.detect()?;
    if !install.installed {
        return Err(format!("emulator not found — {}", install.detail));
    }
    let config = MatchConfig::new(request.rom, request.peer_ip, request.side)?;
    let spec = launcher.spec(&config)?;
    session::launch(&app, &spec, &config, request.dev)
}

#[tauri::command]
pub fn stop_match(app: AppHandle) -> Result<MatchState, String> {
    session::stop(&app)
}

#[tauri::command]
pub fn match_status(app: AppHandle) -> MatchState {
    session::status(&app)
}

#[tauri::command]
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
    session::launch_many(&app, &plans, true, "127.0.0.1 (P1↔P2)".to_string())
}
