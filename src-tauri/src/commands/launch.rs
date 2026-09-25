use super::config::{config_and_provider, provider_for};
use crate::config::Config;
use crate::constants;
use crate::contracts::InstallInfo;
use crate::probe;
use crate::providers::{self, Capabilities, MatchRequest, Provider, Role};
use crate::roms;
use crate::session::{self, MatchState, Plan};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;

fn rom_file(cfg: &Config, rom: &str) -> crate::error::Result<PathBuf> {
    let dir = roms::resolve_rom_dir(cfg)
        .ok_or_else(|| "no ROM directory found — set one in settings".to_string())?;
    let path = dir.join(format!("{rom}.zip"));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("ROM file not found: {}", path.display()).into())
    }
}

fn optional_rom_file(
    cfg: &Config,
    provider: &dyn Provider,
    rom: &str,
) -> crate::error::Result<PathBuf> {
    if provider.requires_rom_file() {
        return rom_file(cfg, rom);
    }
    Ok(roms::resolve_rom_dir(cfg)
        .map(|dir| dir.join(format!("{rom}.zip")))
        .unwrap_or_default())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub install: InstallInfo,
    pub capabilities: Capabilities,
}

#[tauri::command(async)]
pub fn launcher_info(app: AppHandle) -> crate::error::CommandResult<ProviderInfo> {
    let provider = provider_for(&app, false)?;
    Ok(ProviderInfo {
        install: provider.detect()?,
        capabilities: provider.capabilities(),
    })
}

#[tauri::command(async)]
pub fn parity_status(
    app: AppHandle,
    rom: String,
) -> crate::error::CommandResult<Option<providers::ParityStatus>> {
    let (cfg, provider) = config_and_provider(&app, false)?;
    if rom.trim().is_empty() {
        return Ok(None);
    }
    let rom_path = optional_rom_file(&cfg, provider.as_ref(), &rom)?;
    Ok(provider.parity(&rom_path)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub rom: String,
    pub peer_ip: String,
    pub role: Role,
    #[serde(default)]
    pub player_slot: Option<u8>,
    #[serde(default)]
    pub dev: bool,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub host_spectating: bool,
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
    player_slot: Option<u8>,
    rom: &str,
    peer_ip: &str,
    start_as_spectator: bool,
) -> crate::error::Result<Plan> {
    let rom_path = optional_rom_file(cfg, provider, rom)?;
    if let Some(status) = provider.parity(&rom_path)? {
        log::info!(
            "parity for {rom}: ok={} detail={}",
            status.ok,
            status.detail
        );
        if !status.ok {
            return Err(status.detail.into());
        }
    }
    let request = MatchRequest {
        role,
        player_slot,
        rom_path: &rom_path,
        peer_ip,
        start_as_spectator,
    };
    let spec = provider.spec(&request)?;
    Ok(Plan {
        spec,
        role,
        rom: rom.to_string(),
        peer_ip: peer_ip.to_string(),
        port: provider.port(role),
        command_port: provider.command_port(role),
    })
}

fn preflight(
    provider: &dyn Provider,
    role: Role,
    peer_ip: &str,
    force: bool,
) -> crate::error::Result<()> {
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
                    Err(format!("{err} — is another instance already hosting?").into())
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
                    "host not reachable on {peer_ip}:{port}{detail} — start the host first, confirm the peer IP, use the same netplay port on both peers, and allow inbound TCP {port} on the host"
                ).into())
            }
        }
    }
}

/// Developer-mode LaunchCard / loopback diagnostics entry point; the lobby calls
/// `launch_match_inner` directly.
#[tauri::command(async)]
pub fn launch_match(
    app: AppHandle,
    request: LaunchRequest,
) -> crate::error::CommandResult<MatchState> {
    launch_match_inner(&app, request).map_err(Into::into)
}

/// Shared launcher used by the lobby (`lobby_start` / `lobby_join`). The `launch_match` command
/// is kept only for the developer-mode LaunchCard and loopback diagnostics.
pub(crate) fn launch_match_inner(
    app: &AppHandle,
    request: LaunchRequest,
) -> crate::error::Result<MatchState> {
    let (cfg, provider) = config_and_provider(app, request.dev)?;
    log::info!(
        "launch request: role={} seat={:?} rom={} peer={:?} dev={} force={}",
        request.role.label(),
        request.player_slot,
        request.rom,
        request.peer_ip,
        request.dev,
        request.force
    );
    let install = provider.detect()?;
    if !install.installed {
        log::error!("emulator not found — {}", install.detail);
        return Err(format!("emulator not found — {}", install.detail).into());
    }
    if request.role == Role::Spectator && !provider.capabilities().spectate {
        return Err("this provider cannot spectate".into());
    }
    if let Err(err) = provider.ensure_core_visible() {
        log::error!("cannot stage the emulator core: {err}");
        return Err(err);
    }
    let peer_ip = effective_peer(request.dev, &request.peer_ip);
    if !request.dev {
        preflight(provider.as_ref(), request.role, &peer_ip, request.force)?;
    }
    let plan = plan_for(
        provider.as_ref(),
        &cfg,
        request.role,
        request.player_slot,
        &request.rom,
        &peer_ip,
        request.host_spectating,
    )?;
    let wait_for_host =
        matches!(request.role, Role::P2 | Role::Spectator) && (request.dev || request.force);
    session::launch(
        app,
        &plan,
        session::LaunchOptions {
            dev: request.dev,
            wait_for_host,
            peer_display: peer_ip,
            capture_netplay: true,
        },
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedCore {
    pub path: String,
    pub sha256: String,
}

pub const CORE_DOWNLOAD_EVENT: &str = "core-download-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreDownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[tauri::command(async)]
pub fn retroarch_hotkey_map(app: AppHandle) -> crate::error::CommandResult<providers::InputMap> {
    let provider = provider_for(&app, false)?;
    Ok(provider.input_map().unwrap_or_default())
}

#[tauri::command(async)]
pub fn download_retroarch_core(app: AppHandle) -> crate::error::CommandResult<DownloadedCore> {
    use tauri::{Emitter, Manager};
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("cannot resolve data dir: {err}"))?;
    let core = providers::download_managed_core(&data_dir, |downloaded, total| {
        app.emit(
            CORE_DOWNLOAD_EVENT,
            CoreDownloadProgress { downloaded, total },
        )
        .ok();
    })?;
    Ok(DownloadedCore {
        path: core.to_string_lossy().to_string(),
        sha256: providers::frozen_core_sha256().to_string(),
    })
}

#[tauri::command(async)]
pub fn stop_match(app: AppHandle) -> crate::error::CommandResult<MatchState> {
    Ok(session::stop(&app)?)
}

#[tauri::command]
pub fn match_status(app: AppHandle) -> MatchState {
    session::status(&app)
}

/// Developer-mode / loopback diagnostics only; the lobby does not use this pair flow.
#[tauri::command(async)]
pub fn launch_dev_pair(app: AppHandle, rom: String) -> crate::error::CommandResult<MatchState> {
    let (cfg, provider) = config_and_provider(&app, true)?;
    log::info!("launch dev pair: rom={rom}");
    let install = provider.detect()?;
    if !install.installed {
        return Err(format!("emulator not found — {}", install.detail).into());
    }
    if !provider.capabilities().dev_pair {
        return Err("this provider has no dev pair".into());
    }
    let plans = vec![
        plan_for(
            provider.as_ref(),
            &cfg,
            Role::P1,
            None,
            &rom,
            "127.0.0.1",
            false,
        )?,
        plan_for(
            provider.as_ref(),
            &cfg,
            Role::P2,
            None,
            &rom,
            "127.0.0.1",
            false,
        )?,
    ];
    session::launch_many(
        &app,
        &plans,
        session::LaunchOptions {
            dev: true,
            wait_for_host: true,
            peer_display: "127.0.0.1 (P1↔P2)".to_string(),
            capture_netplay: true,
        },
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_launch_defaults_an_empty_peer_to_loopback() {
        assert_eq!(effective_peer(true, ""), "127.0.0.1");
        assert_eq!(effective_peer(true, "   "), "127.0.0.1");
    }
}
