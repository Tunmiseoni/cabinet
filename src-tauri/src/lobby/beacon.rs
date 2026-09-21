use super::room::Room;
use crate::constants;
use crate::sync::MutexExt;
use std::net::{Ipv4Addr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server};

const ROOM_PATH: &str = "/room";

pub(crate) struct Beacon {
    server: Arc<Server>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Beacon {
    pub(crate) fn start(port: u16, room: Arc<Mutex<Room>>) -> crate::error::Result<Self> {
        Self::start_on((Ipv4Addr::UNSPECIFIED, port), room)
    }

    fn start_on<A: ToSocketAddrs>(
        address: A,
        room: Arc<Mutex<Room>>,
    ) -> crate::error::Result<Self> {
        let server =
            Server::http(address).map_err(|err| format!("cannot bind the room beacon: {err}"))?;
        let server = Arc::new(server);
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let server = Arc::clone(&server);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || serve(&server, &stop, &room))
        };
        Ok(Self {
            server,
            stop,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub(crate) fn local_port(&self) -> Option<u16> {
        self.server.server_addr().to_ip().map(|addr| addr.port())
    }

    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.lock_or_recover().take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Beacon {
    fn drop(&mut self) {
        self.stop();
    }
}

fn serve(server: &Server, stop: &AtomicBool, room: &Arc<Mutex<Room>>) {
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match server.recv_timeout(constants::LOBBY_BEACON_TICK) {
            Ok(Some(request)) => respond(request, room),
            Ok(None) => {}
            Err(err) => {
                log::warn!("beacon stopped: {err}");
                break;
            }
        }
    }
}

fn respond(request: Request, room: &Arc<Mutex<Room>>) {
    let path = request.url().split('?').next().unwrap_or("");
    if request.method() == &Method::Get && path == ROOM_PATH {
        let body = serde_json::to_vec(&*room.lock_or_recover()).unwrap_or_else(|_| b"{}".to_vec());
        let header = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
            .expect("static header");
        let response = Response::from_data(body)
            .with_status_code(200)
            .with_header(header);
        let _ = request.respond(response);
    } else {
        let _ = request.respond(Response::from_string("not found").with_status_code(404));
    }
}

pub(crate) fn query(host: &str, port: u16, timeout: Duration) -> Option<Room> {
    let url = format!("http://{host}:{port}{ROOM_PATH}");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut response = agent.get(&url).call().ok()?;
    let text = response.body_mut().read_to_string().ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beacon_with_room(players: u8, spectators: u8) -> (Arc<Mutex<Room>>, Beacon) {
        let room = Arc::new(Mutex::new(Room::new(
            "r1",
            "node-1",
            "player-one",
            "sfiii3nr1",
            2,
            None,
        )));
        room.lock_or_recover().set_occupancy(players, spectators);
        let beacon = Beacon::start_on((Ipv4Addr::LOCALHOST, 0), Arc::clone(&room)).unwrap();
        (room, beacon)
    }

    #[test]
    fn serves_the_room_as_json() {
        let (_room, beacon) = beacon_with_room(2, 1);
        let port = beacon.local_port().unwrap();
        let fetched = query("127.0.0.1", port, constants::LOBBY_BEACON_QUERY_TIMEOUT).unwrap();
        assert_eq!(fetched.rom, "sfiii3nr1");
        assert_eq!(fetched.host_handle, "player-one");
        assert_eq!(fetched.first_to, 2);
        assert_eq!(fetched.players, 2);
        assert_eq!(fetched.spectators, 1);
        assert_eq!(fetched.phase, crate::lobby::room::RoomPhase::Playing);
    }

    #[test]
    fn reflects_updates_to_the_shared_room() {
        let (room, beacon) = beacon_with_room(0, 0);
        let port = beacon.local_port().unwrap();
        room.lock_or_recover().set_occupancy(2, 3);
        let fetched = query("127.0.0.1", port, constants::LOBBY_BEACON_QUERY_TIMEOUT).unwrap();
        assert_eq!(fetched.players, 2);
        assert_eq!(fetched.spectators, 3);
    }

    #[test]
    fn answers_none_when_nothing_is_hosting() {
        let socket = std::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        assert!(query("127.0.0.1", port, Duration::from_millis(200)).is_none());
    }

    #[test]
    fn stops_serving_after_stop() {
        let (_room, beacon) = beacon_with_room(2, 0);
        let port = beacon.local_port().unwrap();
        assert!(query("127.0.0.1", port, constants::LOBBY_BEACON_QUERY_TIMEOUT).is_some());
        beacon.stop();
        assert!(query("127.0.0.1", port, constants::LOBBY_BEACON_QUERY_TIMEOUT).is_none());
    }
}
