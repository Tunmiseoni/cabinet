use crate::commands::config::{config_and_provider, load_config};
use crate::commands::launch::{launch_match_inner, LaunchRequest};
use crate::config::Config;
use crate::constants;
use crate::lobby::beacon;
use crate::lobby::room::{new_room_id, Room};
use crate::lobby::Lobby;
use crate::providers::Role;
use crate::session::MatchState;
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

/// Resolve the launch role and input seat for a joiner. A player always connects as a client
/// (`Role::P2`); the server assigns the seat at connect time, so the *seat* is what varies, not
/// the connect direction. Without an explicit seat, the room's advertised occupancy decides,
/// filling the slots the host has not taken.
fn assign_join(
    spectate: bool,
    requested: Option<u8>,
    room: Option<&Room>,
) -> crate::error::Result<(Role, Option<u8>)> {
    if spectate {
        return Ok((Role::Spectator, None));
    }
    if let Some(slot) = requested {
        if !(1..=2).contains(&slot) {
            return Err(format!("invalid player slot {slot}: expected 1 or 2").into());
        }
        return Ok((Role::P2, Some(slot)));
    }
    let Some(room) = room else {
        // No beacon answered (a manual join by address), so occupancy is unknown. Assume the
        // room is empty; a host that is playing player 1 will refuse the duplicate seat.
        return Ok((Role::P2, Some(1)));
    };
    let held = room.host_seat.is_some() as u8;
    let joiners = room.players.saturating_sub(held);
    room.next_free_seat(joiners)
        .map(|seat| (Role::P2, Some(seat)))
        .ok_or_else(|| "the room is full — join as a spectator, or wait for a seat".into())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyStartRequest {
    pub rom: String,
    #[serde(default = "default_first_to")]
    pub first_to: u8,
    /// The seat the host itself takes (`1`/`2`), or `None` to host as a non-playing spectator
    /// (the "table"). Defaults to seat 1 so a one-friend session is host-vs-joiner.
    #[serde(default)]
    pub host_seat: Option<u8>,
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
    let host_seat = match request.host_seat {
        Some(slot) if (1..=2).contains(&slot) => Some(slot),
        Some(slot) => {
            return Err(format!("invalid host seat {slot}: expected 1 or 2").into());
        }
        None => None,
    };
    let (node_id, handle) = host_identity(&cfg);
    let room = Room::new(
        new_room_id(),
        node_id,
        handle,
        rom.clone(),
        first_to,
        host_seat,
    );
    let room = lobby.start(room, beacon_port(&cfg))?;

    log::info!(
        "hosting room {} ({} first to {first_to}, host seat {host_seat:?})",
        room.room_id,
        room.rom
    );
    if let Err(err) = launch_match_inner(
        &app,
        LaunchRequest {
            rom,
            peer_ip: String::new(),
            role: Role::P1,
            player_slot: host_seat,
            dev: false,
            force: false,
            host_spectating: host_seat.is_none(),
        },
    ) {
        lobby.stop();
        return Err(err.into());
    }
    Ok(room)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbyJoinRequest {
    pub host: String,
    #[serde(default)]
    pub rom: Option<String>,
    #[serde(default)]
    pub spectate: bool,
    #[serde(default)]
    pub player_slot: Option<u8>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinOutcome {
    pub room: Option<Room>,
    pub role: Role,
    pub player_slot: Option<u8>,
    pub state: MatchState,
}

#[tauri::command(async)]
pub fn lobby_join(
    app: AppHandle,
    request: LobbyJoinRequest,
) -> crate::error::CommandResult<JoinOutcome> {
    let cfg = load_config(&app)?;
    let host = request.host.trim().to_string();
    if host.is_empty() {
        return Err("enter a host address to join".into());
    }
    let room = beacon::query(
        &host,
        beacon_port(&cfg),
        constants::LOBBY_BEACON_QUERY_TIMEOUT,
    );
    let rom = request
        .rom
        .as_deref()
        .map(str::trim)
        .filter(|rom| !rom.is_empty())
        .map(str::to_string)
        .or_else(|| room.as_ref().map(|room| room.rom.clone()))
        .ok_or_else(|| {
            "no room answered at that address — enter the ROM to join directly".to_string()
        })?;
    let players = room.as_ref().map(|room| room.players).unwrap_or(0);
    let (role, player_slot) = assign_join(request.spectate, request.player_slot, room.as_ref())?;

    log::info!(
        "joining {host} room={:?} rom={rom} role={} seat={player_slot:?} (advertised players {players})",
        room.as_ref().map(|room| room.room_id.as_str()),
        role.label(),
    );
    let state = launch_match_inner(
        &app,
        LaunchRequest {
            rom,
            peer_ip: host,
            role,
            player_slot,
            dev: false,
            force: request.force,
            host_spectating: false,
        },
    )?;
    Ok(JoinOutcome {
        room,
        role,
        player_slot,
        state,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn room(host_seat: Option<u8>, players: u8) -> Room {
        let mut room = Room::new("r", "node", "player-one", "sfiii3nr1", 2, host_seat);
        room.set_occupancy(players, 0);
        room
    }

    #[test]
    fn the_first_player_joins_on_seat_one() {
        let room = room(None, 0);
        assert_eq!(
            assign_join(false, None, Some(&room)).unwrap(),
            (Role::P2, Some(1))
        );
    }

    #[test]
    fn the_second_player_joins_on_seat_two() {
        let room = room(None, 1);
        assert_eq!(
            assign_join(false, None, Some(&room)).unwrap(),
            (Role::P2, Some(2))
        );
    }

    #[test]
    fn a_joiner_avoids_a_playing_hosts_seat() {
        assert_eq!(
            assign_join(false, None, Some(&room(Some(1), 1))).unwrap(),
            (Role::P2, Some(2))
        );
        assert_eq!(
            assign_join(false, None, Some(&room(Some(2), 1))).unwrap(),
            (Role::P2, Some(1))
        );
    }

    #[test]
    fn a_full_room_must_spectate() {
        let full = room(None, 2);
        assert!(assign_join(false, None, Some(&full)).is_err());
        assert_eq!(
            assign_join(true, None, Some(&full)).unwrap(),
            (Role::Spectator, None)
        );
        let seated_host = room(Some(1), 2);
        assert!(assign_join(false, None, Some(&seated_host)).is_err());
    }

    #[test]
    fn a_manual_join_without_a_beacon_falls_back_to_seat_one() {
        assert_eq!(assign_join(false, None, None).unwrap(), (Role::P2, Some(1)));
    }

    #[test]
    fn an_explicit_seat_wins_over_occupancy() {
        let room = room(None, 0);
        assert_eq!(
            assign_join(false, Some(2), Some(&room)).unwrap(),
            (Role::P2, Some(2))
        );
    }

    #[test]
    fn an_out_of_range_seat_is_rejected() {
        assert!(assign_join(false, Some(0), None).is_err());
        assert!(assign_join(false, Some(3), None).is_err());
    }
}
