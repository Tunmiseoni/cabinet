use crate::commands::config::{config_and_provider, load_config};
use crate::commands::launch::{launch_match, LaunchRequest};
use crate::config::Config;
use crate::constants;
use crate::lobby::beacon;
use crate::lobby::room::{new_room_id, Room};
use crate::lobby::Lobby;
use crate::providers::Role;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const DEFAULT_FIRST_TO: u8 = 2;

fn default_first_to() -> u8 {
    DEFAULT_FIRST_TO
}

fn beacon_port(cfg: &Config) -> u16 {
    if cfg.lobby_beacon_port == 0 {
        constants::LOBBY_BEACON_PORT
    } else {
        cfg.lobby_beacon_port
    }
}

fn host_identity(cfg: &Config) -> (String, String) {
    let self_peer = crate::tailscale::resolve_binary(cfg)
        .ok()
        .and_then(|binary| crate::tailscale::status(&binary).ok())
        .and_then(|tailnet| tailnet.self_peer);
    let node_id = self_peer
        .as_ref()
        .map(|peer| peer.node_id.clone())
        .filter(|value| !value.trim().is_empty())
        .or_else(crate::env::os_hostname)
        .unwrap_or_else(|| "unknown".to_string());
    let handle = cfg
        .handle
        .as_deref()
        .or(cfg.retroarch_nickname.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| self_peer.as_ref().map(|peer| peer.hostname.clone()))
        .or_else(crate::env::os_hostname)
        .unwrap_or_else(|| "player".to_string());
    (node_id, handle)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyStartRequest {
    pub rom: String,
    #[serde(default = "default_first_to")]
    pub first_to: u8,
}

#[tauri::command(async)]
pub fn lobby_start(
    app: AppHandle,
    request: LobbyStartRequest,
) -> crate::error::CommandResult<Room> {
    let (cfg, provider) = config_and_provider(&app, false)?;
    let lobby = app.state::<Lobby>();
    if lobby.room().is_some() {
        return Err("a room is already hosting".into());
    }
    let install = provider.detect()?;
    if !install.installed {
        return Err(format!("emulator not found — {}", install.detail).into());
    }
    let rom = request.rom.trim().to_string();
    if rom.is_empty() {
        return Err("choose a ROM for the room".into());
    }
    let first_to = request.first_to.clamp(1, 9);
    let (node_id, handle) = host_identity(&cfg);
    let room = Room::new(new_room_id(), node_id, handle, rom.clone(), first_to);
    let room = lobby.start(room, beacon_port(&cfg))?;

    log::info!(
        "hosting room {} ({} first to {first_to})",
        room.room_id,
        room.rom
    );
    if let Err(err) = launch_match(
        app.clone(),
        LaunchRequest {
            rom,
            peer_ip: String::new(),
            role: Role::P1,
            dev: false,
            force: false,
            host_spectating: true,
        },
    ) {
        lobby.stop();
        return Err(err);
    }
    Ok(room)
}

#[tauri::command(async)]
pub fn lobby_stop(app: AppHandle) -> crate::error::CommandResult<()> {
    app.state::<Lobby>().stop();
    let _ = crate::session::stop(&app);
    Ok(())
}

#[tauri::command]
pub fn lobby_status(app: AppHandle) -> Option<Room> {
    app.state::<Lobby>().room()
}

#[tauri::command(async)]
pub fn lobby_query(app: AppHandle, host: String) -> crate::error::CommandResult<Option<Room>> {
    let cfg = load_config(&app)?;
    let host = host.trim();
    if host.is_empty() {
        return Err("enter a host address to join".into());
    }
    Ok(beacon::query(
        host,
        beacon_port(&cfg),
        constants::LOBBY_BEACON_QUERY_TIMEOUT,
    ))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredRoom {
    pub host_ip: String,
    pub host_hostname: String,
    pub room: Room,
}

#[tauri::command(async)]
pub fn lobby_discover(app: AppHandle) -> crate::error::CommandResult<Vec<DiscoveredRoom>> {
    let cfg = load_config(&app)?;
    let port = beacon_port(&cfg);
    let Ok(binary) = crate::tailscale::resolve_binary(&cfg) else {
        return Ok(Vec::new());
    };
    let tailnet = crate::tailscale::status(&binary)?;
    let mut rooms = Vec::new();
    for peer in tailnet
        .peers
        .iter()
        .filter(|peer| peer.online && !peer.is_self && !peer.ip.trim().is_empty())
    {
        if let Some(room) = beacon::query(&peer.ip, port, constants::LOBBY_BEACON_PROBE_TIMEOUT) {
            rooms.push(DiscoveredRoom {
                host_ip: peer.ip.clone(),
                host_hostname: peer.hostname.clone(),
                room,
            });
        }
    }
    Ok(rooms)
}
