use crate::constants;
use crate::probe;
use crate::roms::{self, RomIndex};
use crate::tailscale::{self, PeerHealth, Tailnet};
use std::time::Duration;
use tauri::AppHandle;

pub(crate) mod cabinet;
pub(crate) mod config;
pub(crate) mod launch;
pub(crate) mod lobby;

pub(crate) use config::load_config;

#[tauri::command(async)]
pub fn list_peers(app: AppHandle) -> crate::error::CommandResult<Tailnet> {
    let cfg = load_config(&app)?;
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::status(&binary)?)
}

#[tauri::command(async)]
pub fn peer_health(app: AppHandle, ip: String) -> crate::error::CommandResult<PeerHealth> {
    let cfg = load_config(&app)?;
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::ping(&binary, &ip))
}

#[tauri::command(async)]
pub fn peers_health(
    app: AppHandle,
    ips: Vec<String>,
) -> crate::error::CommandResult<Vec<PeerHealth>> {
    let cfg = load_config(&app)?;
    let binary = tailscale::resolve_binary(&cfg)?;
    Ok(tailscale::ping_many(&binary, &ips))
}

#[tauri::command(async)]
pub fn list_roms(app: AppHandle) -> crate::error::CommandResult<RomIndex> {
    let cfg = load_config(&app)?;
    Ok(roms::index(&cfg))
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
