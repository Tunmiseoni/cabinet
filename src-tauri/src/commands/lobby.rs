use crate::commands::config::{config_and_provider, load_config};
use crate::commands::launch::{launch_match_inner, LaunchRequest};
use crate::config::Config;
use crate::constants;
use crate::lobby::beacon;
use crate::lobby::results::{self, RoundWatcher};
use crate::lobby::room::{new_room_id, Room};
use crate::lobby::sets::{winner_slot, SetEvent, SetMachine};
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
    // The displayed handle is the same identity the lobby seats by, so the room and its seats agree.
    (
        node_id,
        crate::config::netplay_nickname(cfg, self_peer.as_ref().map(|peer| peer.hostname.as_str())),
    )
}

/// This machine's own netplay nickname — the identity the lobby keys seats by.
fn netplay_identity(cfg: &Config) -> String {
    let tailnet = crate::tailscale::resolve_binary(cfg)
        .ok()
        .and_then(|binary| crate::tailscale::status(&binary).ok())
        .and_then(|tailnet| tailnet.self_peer)
        .map(|peer| peer.hostname);
    crate::config::netplay_nickname(cfg, tailnet.as_deref())
}

/// Resolve the launch role and seat for a joiner. A player always connects as a client
/// (`Role::P2`) and lets RetroArch hand it the first free player device; the room's advertised
/// seats decide whether a slot is free. A full room joins as a spectator rather than erroring —
/// the set-end rotation promotes the longest-waiting spectator when a seat opens.
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
    match room.seats.iter().position(|slot| slot.is_none()) {
        Some(index) => Ok((Role::P2, Some(index as u8 + 1))),
        None => Ok((Role::Spectator, None)),
    }
}

/// The command port of this machine's own instance. The host is always `Role::P1`; a joiner is
/// either `Role::P2` or `Role::Spectator`.
fn own_command_port(app: &AppHandle, host: bool) -> Option<u16> {
    crate::session::status(app)
        .instances
        .iter()
        .find(|instance| (instance.role == Role::P1) == host)
        .and_then(|instance| instance.command_port)
}

fn session_running(app: &AppHandle) -> bool {
    crate::session::status(app).status == "running"
}

/// Toggle this machine's own instance once per advertised rotation: the loser steps out, the
/// queue head steps in. The first room seen only primes `last_rotation`, so an in-flight rotation
/// at join time is never acted on twice.
fn reconcile_rotation(
    last_rotation: &mut Option<u64>,
    primed: &mut bool,
    room: &Room,
    own_nick: &str,
    current_slot: &mut Option<u8>,
    command_port: u16,
) {
    let Some(rotation) = &room.rotation else {
        *primed = true;
        return;
    };
    if !*primed {
        *last_rotation = Some(rotation.id);
        *primed = true;
        return;
    }
    if *last_rotation == Some(rotation.id) {
        return;
    }

    let is_incoming = rotation.incoming.as_deref() == Some(own_nick);
    let is_loser = *current_slot == Some(rotation.loser_slot);
    if !is_incoming && !is_loser {
        *last_rotation = Some(rotation.id);
        return;
    }
    match crate::providers::command::toggle_game_watch(command_port) {
        Ok(()) => {
            log::info!(
                "rotation {}: {own_nick} {}",
                rotation.id,
                if is_loser { "steps out" } else { "steps in" }
            );
            *current_slot = if is_loser {
                None
            } else {
                Some(rotation.loser_slot)
            };
            *last_rotation = Some(rotation.id);
        }
        Err(err) => log::warn!("cannot act on rotation {}: {err}", rotation.id),
    }
}

/// Host-side set machine: poll the host instance's RAM, roll rounds up to games and sets, publish
/// the score, and announce a rotation when a set ends. Only ROMs with a known address table (see
/// `lobby::results`) are watched, so unsupported ROMs still play but do not rotate automatically.
fn spawn_set_runner(
    app: AppHandle,
    room_id: String,
    rom: String,
    first_to: u8,
    own_nick: String,
    initial_slot: Option<u8>,
) {
    let Some(addresses) = results::addresses_for(&rom) else {
        log::info!("no result addresses for {rom}; automatic rotation is off for this room");
        return;
    };
    std::thread::spawn(move || {
        let mut watcher = RoundWatcher::new();
        let mut machine = SetMachine::new(first_to);
        let mut current_slot = initial_slot;
        let mut last_rotation: Option<u64> = None;
        let mut primed = false;
        loop {
            std::thread::sleep(constants::LOBBY_SET_POLL);
            if !session_running(&app) {
                break;
            }
            let Some(lobby) = app.try_state::<Lobby>() else {
                break;
            };
            let Some(room) = lobby.room() else {
                break;
            };
            if room.room_id != room_id {
                break;
            }
            let Some(port) = own_command_port(&app, true) else {
                continue;
            };
            match results::read_snapshot(port, addresses) {
                Ok(snapshot) => {
                    if let Some(outcome) = watcher.observe(snapshot) {
                        let event = machine.observe(outcome);
                        lobby.set_score(machine.score());
                        if let SetEvent::SetWon(winner) = event {
                            if let Some(slot) = winner_slot(winner) {
                                if let Some(rotation) = lobby.announce_rotation(slot) {
                                    log::info!(
                                        "set won in slot {slot}; rotation {} -> {:?}",
                                        rotation.id,
                                        rotation.incoming
                                    );
                                }
                            }
                        }
                    }
                }
                Err(err) => log::debug!("set watcher read failed: {err}"),
            }
            if let Some(room) = lobby.room() {
                reconcile_rotation(
                    &mut last_rotation,
                    &mut primed,
                    &room,
                    &own_nick,
                    &mut current_slot,
                    port,
                );
            }
        }
    });
}

/// Joiner-side seat follower: poll the host's beacon and act on the advertised rotation, so a
/// spectator is promoted into a freed seat and a losing player steps out without a cross-network
/// control channel.
fn spawn_seat_follower(app: AppHandle, host: String, own_nick: String, initial_slot: Option<u8>) {
    std::thread::spawn(move || {
        let mut current_slot = initial_slot;
        let mut last_rotation: Option<u64> = None;
        let mut primed = false;
        loop {
            std::thread::sleep(constants::LOBBY_SEAT_POLL);
            if !session_running(&app) {
                break;
            }
            let Some(port) = own_command_port(&app, false) else {
                continue;
            };
            let Ok(cfg) = load_config(&app) else {
                continue;
            };
            let Some(room) = beacon::query(
                &host,
                beacon_port(&cfg),
                constants::LOBBY_BEACON_QUERY_TIMEOUT,
            ) else {
                continue;
            };
            reconcile_rotation(
                &mut last_rotation,
                &mut primed,
                &room,
                &own_nick,
                &mut current_slot,
                port,
            );
        }
    });
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
    let own_nick = netplay_identity(&cfg);
    let mut room = Room::new(
        new_room_id(),
        node_id,
        handle,
        rom.clone(),
        first_to,
        host_seat,
    );
    if let Some(seat) = host_seat {
        let mut seats = room.seats();
        seats.set(seat, Some(own_nick.clone()));
        room.set_seats(&seats);
    }
    let room = lobby.start(room, beacon_port(&cfg), own_nick.clone())?;

    log::info!(
        "hosting room {} ({} first to {first_to}, host seat {host_seat:?})",
        room.room_id,
        room.rom
    );
    if let Err(err) = launch_match_inner(
        &app,
        LaunchRequest {
            rom: rom.clone(),
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
    spawn_set_runner(
        app.clone(),
        room.room_id.clone(),
        rom,
        room.first_to,
        own_nick,
        host_seat,
    );
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
    let own_nick = netplay_identity(&cfg);
    let state = launch_match_inner(
        &app,
        LaunchRequest {
            rom,
            peer_ip: host.clone(),
            role,
            player_slot,
            dev: false,
            force: request.force,
            host_spectating: false,
        },
    )?;
    spawn_seat_follower(app.clone(), host, own_nick, player_slot);
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

/// The room this machine is currently in: the hosted room, or the room of the match it joined
/// (queried from the host's beacon). `None` when idle. The UI polls this for the live seat/score
/// view.
#[tauri::command(async)]
pub fn lobby_room(app: AppHandle) -> crate::error::CommandResult<Option<Room>> {
    let lobby = app.state::<Lobby>();
    if let Some(room) = lobby.room() {
        return Ok(Some(room));
    }
    let state = crate::session::status(&app);
    if state.status != "running" {
        return Ok(None);
    }
    let Some(host) = state
        .peer_ip
        .as_deref()
        .map(str::trim)
        .filter(|ip| !ip.is_empty() && *ip != "127.0.0.1")
    else {
        return Ok(None);
    };
    let cfg = load_config(&app)?;
    Ok(beacon::query(
        host,
        beacon_port(&cfg),
        constants::LOBBY_BEACON_QUERY_TIMEOUT,
    ))
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
    use crate::lobby::sets::{Rotation, Seats};

    fn room_with(pairs: &[(u8, &str)]) -> Room {
        let mut room = Room::new("r", "node", "player-one", "sfiii3nr1", 2, None);
        let mut seats = Seats::default();
        for (slot, nick) in pairs {
            seats.set(*slot, Some((*nick).to_string()));
        }
        room.set_seats(&seats);
        room
    }

    #[test]
    fn the_first_player_joins_on_seat_one() {
        let room = room_with(&[]);
        assert_eq!(
            assign_join(false, None, Some(&room)).unwrap(),
            (Role::P2, Some(1))
        );
    }

    #[test]
    fn the_second_player_joins_on_seat_two() {
        let room = room_with(&[(1, "player-one")]);
        assert_eq!(
            assign_join(false, None, Some(&room)).unwrap(),
            (Role::P2, Some(2))
        );
    }

    #[test]
    fn a_joiner_avoids_a_playing_hosts_seat() {
        assert_eq!(
            assign_join(false, None, Some(&room_with(&[(1, "player-one")]))).unwrap(),
            (Role::P2, Some(2))
        );
        assert_eq!(
            assign_join(false, None, Some(&room_with(&[(2, "player-one")]))).unwrap(),
            (Role::P2, Some(1))
        );
    }

    #[test]
    fn a_full_room_joins_as_a_spectator_instead_of_erroring() {
        let full = room_with(&[(1, "player-one"), (2, "player-two")]);
        assert_eq!(
            assign_join(false, None, Some(&full)).unwrap(),
            (Role::Spectator, None)
        );
        assert_eq!(
            assign_join(true, None, Some(&full)).unwrap(),
            (Role::Spectator, None)
        );
    }

    #[test]
    fn a_manual_join_without_a_beacon_falls_back_to_seat_one() {
        assert_eq!(assign_join(false, None, None).unwrap(), (Role::P2, Some(1)));
    }

    #[test]
    fn an_explicit_seat_wins_over_occupancy() {
        let room = room_with(&[]);
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

    fn rotation(loser_slot: u8, incoming: &str) -> Room {
        let mut room = room_with(&[(1, "player-one"), (2, "player-two")]);
        room.set_rotation(Rotation {
            id: 3,
            loser_slot,
            incoming: Some(incoming.to_string()),
        });
        room
    }

    #[test]
    fn the_loser_steps_out_and_clears_its_slot() {
        let room = rotation(2, "watcher");
        let mut current = Some(2);
        let mut last = Some(2); // already primed; rotation 3 is new
        let mut primed = true;
        let server = std::net::UdpSocket::bind(("127.0.0.1", 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            let (len, _) = server.recv_from(&mut buffer).unwrap();
            assert_eq!(&buffer[..len], b"NETPLAY_GAME_WATCH\n");
        });
        reconcile_rotation(
            &mut last,
            &mut primed,
            &room,
            "player-two",
            &mut current,
            port,
        );
        handle.join().unwrap();
        assert_eq!(current, None);
    }

    #[test]
    fn the_queue_head_steps_in() {
        let room = rotation(2, "watcher");
        let mut current = None;
        let mut last = Some(2); // already primed; rotation 3 is new
        let mut primed = true;
        let server = std::net::UdpSocket::bind(("127.0.0.1", 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            let (len, _) = server.recv_from(&mut buffer).unwrap();
            assert_eq!(&buffer[..len], b"NETPLAY_GAME_WATCH\n");
        });
        reconcile_rotation(&mut last, &mut primed, &room, "watcher", &mut current, port);
        handle.join().unwrap();
        assert_eq!(current, Some(2));
    }

    #[test]
    fn an_uninvolved_player_does_not_toggle() {
        let room = rotation(2, "watcher");
        let mut current = Some(1);
        let mut last = Some(2); // already primed; rotation 3 is new
        let mut primed = true;
        // A port with no listener: sending must not happen, or the test's socket stays silent.
        let server = std::net::UdpSocket::bind(("127.0.0.1", 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        server
            .set_read_timeout(Some(std::time::Duration::from_millis(50)))
            .unwrap();
        reconcile_rotation(
            &mut last,
            &mut primed,
            &room,
            "player-one",
            &mut current,
            port,
        );
        let mut buffer = [0u8; 8];
        assert!(server.recv_from(&mut buffer).is_err());
    }

    #[test]
    fn the_first_room_only_primes_the_rotation() {
        let room = rotation(2, "watcher");
        let mut current = Some(2);
        let mut last = None;
        let mut primed = false;
        let server = std::net::UdpSocket::bind(("127.0.0.1", 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        reconcile_rotation(
            &mut last,
            &mut primed,
            &room,
            "player-two",
            &mut current,
            port,
        );
        assert_eq!(last, Some(3));
        assert_eq!(current, Some(2));
    }
}
