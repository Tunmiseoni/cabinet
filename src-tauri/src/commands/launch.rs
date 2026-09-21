use super::config::{config_and_provider, provider_for};
use crate::config::Config;
use crate::constants;
use crate::contracts::InstallInfo;
use crate::probe;
use crate::providers::{self, Capabilities, MatchRequest, Provider, ProviderKind, Role};
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
    pub kind: ProviderKind,
    pub install: InstallInfo,
    pub capabilities: Capabilities,
}

#[tauri::command(async)]
pub fn launcher_info(app: AppHandle) -> crate::error::CommandResult<ProviderInfo> {
    let provider = provider_for(&app, false)?;
    Ok(ProviderInfo {
        kind: provider.kind(),
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
    })
}

fn preflight(
    provider: &dyn Provider,
    role: Role,
    peer_ip: &str,
    force: bool,
) -> crate::error::Result<()> {
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
                    "host not reachable on {peer_ip}:{port}{detail} — start the host first, confirm the peer IP, and allow inbound TCP {port} on the host"
                ).into())
            }
        }
    }
}

#[tauri::command(async)]
pub fn launch_match(
    app: AppHandle,
    request: LaunchRequest,
) -> crate::error::CommandResult<MatchState> {
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
        return Err(format!("emulator not found — {}", install.detail).into());
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
    let wait_for_host = request.dev
        && provider.kind() == ProviderKind::Retroarch
        && matches!(request.role, Role::P2 | Role::Spectator);
    session::launch(
        &app,
        &plan,
        session::LaunchOptions {
            dev: request.dev,
            wait_for_host,
            peer_display: peer_ip,
            capture_netplay: provider.kind() == ProviderKind::Retroarch,
        },
    )
    .map_err(Into::into)
}

#[tauri::command(async)]
pub fn stop_match(app: AppHandle) -> crate::error::CommandResult<MatchState> {
    Ok(session::stop(&app)?)
}

#[tauri::command]
pub fn match_status(app: AppHandle) -> MatchState {
    session::status(&app)
}

#[tauri::command(async)]
pub fn launch_dev_pair(app: AppHandle, rom: String) -> crate::error::CommandResult<MatchState> {
    let (cfg, provider) = config_and_provider(&app, true)?;
    log::info!("launch dev pair: provider={:?} rom={rom}", provider.kind());
    let install = provider.detect()?;
    if !install.installed {
        return Err(format!("emulator not found — {}", install.detail).into());
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
            peer_display: "127.0.0.1 (P1↔P2)".to_string(),
            capture_netplay: provider.kind() == ProviderKind::Retroarch,
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
