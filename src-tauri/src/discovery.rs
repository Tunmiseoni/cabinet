use serde::{Deserialize, Serialize};
use std::io;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MAGIC: &str = "cabinet/1";
pub const KIND_PROBE: &str = "probe";
pub const KIND_ROOM: &str = "room";
pub const DEFAULT_PORT: u16 = 47810;
const MAX_DATAGRAM: usize = 2048;

pub const PROBE_DATAGRAM: &str = r#"{"magic":"cabinet/1","kind":"probe"}"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomAdvert {
    pub room_id: String,
    pub host: String,
    pub rom: String,
    pub phase: String,
    pub champion: Option<String>,
    pub queue: u32,
    pub players: u32,
    pub secret_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inbound {
    Probe,
    Room(RoomAdvert),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredRoom {
    pub ip: String,
    #[serde(flatten)]
    pub advert: RoomAdvert,
}

pub fn encode_room(advert: &RoomAdvert) -> Option<String> {
    let mut value = serde_json::to_value(advert).ok()?;
    let object = value.as_object_mut()?;
    object.insert("magic".to_string(), MAGIC.into());
    object.insert("kind".to_string(), KIND_ROOM.into());
    serde_json::to_string(&value).ok()
}

pub fn parse_datagram(raw: &str) -> Option<Inbound> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    if value.get("magic").and_then(|m| m.as_str())? != MAGIC {
        return None;
    }
    match value.get("kind").and_then(|k| k.as_str())? {
        KIND_PROBE => Some(Inbound::Probe),
        KIND_ROOM => serde_json::from_value(value).ok().map(Inbound::Room),
        _ => None,
    }
}

pub fn probe(ip: &str, port: u16, timeout: Duration) -> Vec<RoomAdvert> {
    let socket = match UdpSocket::bind(("0.0.0.0", 0)) {
        Ok(socket) => socket,
        Err(_) => return Vec::new(),
    };
    if socket.send_to(PROBE_DATAGRAM.as_bytes(), (ip, port)).is_err() {
        return Vec::new();
    }

    let deadline = Instant::now() + timeout;
    let mut rooms = Vec::new();
    let mut buffer = [0u8; MAX_DATAGRAM];

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        socket.set_read_timeout(Some(remaining)).ok();
        match socket.recv_from(&mut buffer) {
            Ok((size, _)) => {
                let raw = String::from_utf8_lossy(&buffer[..size]);
                if let Some(Inbound::Room(advert)) = parse_datagram(&raw) {
                    if !rooms.contains(&advert) {
                        rooms.push(advert);
                    }
                }
            }
            Err(_) => break,
        }
    }

    rooms
}

pub fn probe_many(ips: &[String], port: u16, timeout: Duration) -> Vec<DiscoveredRoom> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = ips
            .iter()
            .map(|ip| scope.spawn(|| probe(ip, port, timeout)))
            .collect();
        handles
            .into_iter()
            .zip(ips)
            .flat_map(|(handle, ip)| {
                handle
                    .join()
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |advert| DiscoveredRoom {
                        ip: ip.clone(),
                        advert,
                    })
            })
            .collect()
    })
}

pub struct Advertiser {
    socket: UdpSocket,
    state: Arc<Mutex<Option<RoomAdvert>>>,
}

impl Advertiser {
    pub fn bind(port: u16) -> io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", port))?;
        Ok(Self {
            socket,
            state: Arc::new(Mutex::new(None)),
        })
    }

    #[allow(dead_code)]
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.socket.local_addr()
    }

    pub fn set(&self, advert: Option<RoomAdvert>) {
        *self.state.lock().unwrap() = advert;
    }

    pub fn snapshot(&self) -> Option<RoomAdvert> {
        self.state.lock().unwrap().clone()
    }

    pub fn serve(self: Arc<Self>) {
        let mut buffer = [0u8; MAX_DATAGRAM];
        while let Ok((size, from)) = self.socket.recv_from(&mut buffer) {
            let raw = String::from_utf8_lossy(&buffer[..size]);
            if !matches!(parse_datagram(&raw), Some(Inbound::Probe)) {
                continue;
            }
            let reply = self.snapshot().and_then(|advert| encode_room(&advert));
            if let Some(reply) = reply {
                let _ = self.socket.send_to(reply.as_bytes(), from);
            }
        }
    }

    pub fn spawn(self) -> Arc<Self> {
        let shared = Arc::new(self);
        let worker = Arc::clone(&shared);
        std::thread::spawn(move || worker.serve());
        shared
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advert() -> RoomAdvert {
        RoomAdvert {
            room_id: "room-abc".to_string(),
            host: "Tunmise".to_string(),
            rom: "sfiii3nr1".to_string(),
            phase: "lobby".to_string(),
            champion: Some("Tunmise".to_string()),
            queue: 1,
            players: 3,
            secret_required: true,
        }
    }

    #[test]
    fn room_round_trips_through_the_wire() {
        let encoded = encode_room(&advert()).expect("encode");
        let decoded = parse_datagram(&encoded).expect("decode");
        assert_eq!(decoded, Inbound::Room(advert()));
    }

    #[test]
    fn probe_is_recognized() {
        assert_eq!(parse_datagram(PROBE_DATAGRAM), Some(Inbound::Probe));
    }

    #[test]
    fn ignores_wrong_magic_kind_and_garbage() {
        assert_eq!(parse_datagram(""), None);
        assert_eq!(parse_datagram("not json"), None);
        assert_eq!(
            parse_datagram(r#"{"magic":"other","kind":"probe"}"#),
            None
        );
        assert_eq!(
            parse_datagram(r#"{"magic":"cabinet/1","kind":"wat"}"#),
            None
        );
        assert_eq!(
            parse_datagram(r#"{"magic":"cabinet/1","kind":"room"}"#),
            None
        );
    }

    #[test]
    fn encoded_room_carries_magic_and_kind() {
        let encoded = encode_room(&advert()).expect("encode");
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(value["magic"], MAGIC);
        assert_eq!(value["kind"], KIND_ROOM);
        assert_eq!(value["roomId"], "room-abc");
    }

    #[test]
    fn advertiser_answers_probes_when_hosting() {
        let advertiser = Advertiser::bind(0).expect("bind ephemeral").spawn();
        let port = advertiser.local_addr().unwrap().port();
        advertiser.set(Some(advert()));

        let rooms = probe("127.0.0.1", port, Duration::from_millis(500));
        assert_eq!(rooms, vec![advert()]);
    }

    #[test]
    fn advertiser_stays_silent_when_not_hosting() {
        let advertiser = Advertiser::bind(0).expect("bind ephemeral").spawn();
        let port = advertiser.local_addr().unwrap().port();

        let rooms = probe("127.0.0.1", port, Duration::from_millis(150));
        assert!(rooms.is_empty());
    }

    #[test]
    fn advertiser_clears_when_host_stops() {
        let advertiser = Advertiser::bind(0).expect("bind ephemeral").spawn();
        let port = advertiser.local_addr().unwrap().port();
        advertiser.set(Some(advert()));
        assert_eq!(probe("127.0.0.1", port, Duration::from_millis(500)).len(), 1);

        advertiser.set(None);
        assert!(probe("127.0.0.1", port, Duration::from_millis(150)).is_empty());
    }

    #[test]
    fn probing_a_dead_port_returns_empty() {
        assert!(probe("127.0.0.1", 1, Duration::from_millis(100)).is_empty());
    }
}
