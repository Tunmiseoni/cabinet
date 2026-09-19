use crate::player::Player;
use crate::room::{RoomError, RoomState};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 47811;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Want {
    Play,
    Spectate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Win,
    Loss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Overlay,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        v: u32,
        room_id: String,
        node_id: String,
        handle: String,
        #[serde(default)]
        secret: Option<String>,
        want: Want,
    },
    Enqueue,
    Leave,
    Result {
        match_id: String,
        outcome: Outcome,
        confidence: Confidence,
    },
    Pong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        player_id: String,
        state: RoomState,
    },
    State {
        state: RoomState,
    },
    Error {
        message: String,
    },
    Ping,
}

pub fn encode_line<T: Serialize>(message: &T) -> std::io::Result<String> {
    let mut line = serde_json::to_string(message).map_err(std::io::Error::other)?;
    line.push('\n');
    Ok(line)
}

pub fn decode_line<T: DeserializeOwned>(line: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(line.trim_end())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

type ClientWriters = Mutex<HashMap<u64, Arc<Mutex<TcpStream>>>>;

pub struct ControlServer {
    listener: TcpListener,
    room: Arc<Mutex<RoomState>>,
    secret: Option<String>,
    clients: Arc<ClientWriters>,
    next_client_id: AtomicU64,
}

impl ControlServer {
    pub fn bind(room: RoomState, secret: Option<String>, port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        Ok(Self {
            listener,
            room: Arc::new(Mutex::new(room)),
            secret: secret.filter(|s| !s.is_empty()),
            clients: Arc::new(Mutex::new(HashMap::new())),
            next_client_id: AtomicU64::new(1),
        })
    }

    #[allow(dead_code)]
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    pub fn state(&self) -> RoomState {
        self.room.lock().unwrap().clone()
    }

    pub fn spawn(self) -> Arc<Self> {
        let shared = Arc::new(self);
        let worker = Arc::clone(&shared);
        std::thread::spawn(move || worker.serve());
        shared
    }

    pub fn serve(self: Arc<Self>) {
        while let Ok((stream, _)) = self.listener.accept() {
            let server = Arc::clone(&self);
            std::thread::spawn(move || {
                if let Err(err) = server.handle(stream) {
                    eprintln!("control client error: {err}");
                }
            });
        }
    }

    fn handle(self: &Arc<Self>, stream: TcpStream) -> std::io::Result<()> {
        stream.set_nodelay(true).ok();
        let writer = Arc::new(Mutex::new(stream.try_clone()?));
        let client_id = self.next_client_id.fetch_add(1, Ordering::Relaxed);
        let mut reader = BufReader::new(stream);

        let hello = match Self::read_client(&mut reader)? {
            Some(message) => message,
            None => return Ok(()),
        };

        let (node_id, handle) = match hello {
            ClientMessage::Hello {
                v,
                room_id,
                node_id,
                handle,
                secret,
                want: _,
            } => {
                if v != VERSION {
                    Self::send_error(&writer, &format!("unsupported protocol version {v}"))?;
                    return Ok(());
                }
                let expected_room = self.room.lock().unwrap().room_id.clone();
                if room_id != expected_room {
                    Self::send_error(&writer, &format!("unknown room {room_id}"))?;
                    return Ok(());
                }
                if self.secret.as_deref() != secret.as_deref() {
                    Self::send_error(&writer, "invalid room secret")?;
                    return Ok(());
                }
                (node_id, handle)
            }
            _ => {
                Self::send_error(&writer, "expected hello")?;
                return Ok(());
            }
        };

        self.clients
            .lock()
            .unwrap()
            .insert(client_id, Arc::clone(&writer));

        let welcome = ServerMessage::Welcome {
            player_id: node_id.clone(),
            state: self.state(),
        };
        Self::write(&writer, &welcome)?;

        let player = Player::new(node_id.clone(), handle, String::new());
        let result = self.session(&mut reader, &writer, &player);

        self.clients.lock().unwrap().remove(&client_id);
        result
    }

    fn session(
        self: &Arc<Self>,
        reader: &mut BufReader<TcpStream>,
        writer: &Arc<Mutex<TcpStream>>,
        player: &Player,
    ) -> std::io::Result<()> {
        while let Some(message) = Self::read_client(reader)? {
            match message {
                ClientMessage::Enqueue => {
                    self.mutate(writer, |room| room.enqueue(player.clone(), now_ms()))?;
                }
                ClientMessage::Leave => {
                    self.mutate(writer, |room| room.leave(&player.node_id, now_ms()))?;
                }
                ClientMessage::Result {
                    match_id,
                    outcome,
                    confidence: _,
                } => {
                    self.mutate(writer, |room| {
                        let winner = match outcome {
                            Outcome::Win => player.node_id.clone(),
                            Outcome::Loss => room
                                .current_match
                                .as_ref()
                                .and_then(|current| current.opponent_of(&player.node_id))
                                .map(|slot| slot.node_id.clone())
                                .ok_or_else(|| RoomError::UnknownPlayer(player.node_id.clone()))?,
                        };
                        room.report_result(&match_id, &winner, now_ms())
                    })?;
                }
                ClientMessage::Pong => {}
                ClientMessage::Hello { .. } => Self::send_error(writer, "already greeted")?,
            }
        }
        Ok(())
    }

    fn mutate<F>(self: &Arc<Self>, writer: &Arc<Mutex<TcpStream>>, action: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut RoomState) -> Result<(), RoomError>,
    {
        let outcome = {
            let mut room = self.room.lock().unwrap();
            action(&mut room)
        };
        match outcome {
            Ok(()) => self.broadcast(),
            Err(err) => Self::send_error(writer, &err.to_string()),
        }
    }

    fn broadcast(self: &Arc<Self>) -> std::io::Result<()> {
        let state = self.state();
        let message = ServerMessage::State { state };
        let clients = self.clients.lock().unwrap();
        let mut dead = Vec::new();
        for (id, writer) in clients.iter() {
            if Self::write(writer, &message).is_err() {
                dead.push(*id);
            }
        }
        drop(clients);
        if !dead.is_empty() {
            let mut clients = self.clients.lock().unwrap();
            for id in dead {
                clients.remove(&id);
            }
        }
        Ok(())
    }

    fn read_client(reader: &mut BufReader<TcpStream>) -> std::io::Result<Option<ClientMessage>> {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        decode_line(&line)
            .map(Some)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
    }

    fn write(writer: &Arc<Mutex<TcpStream>>, message: &ServerMessage) -> std::io::Result<()> {
        let line = encode_line(message)?;
        let mut stream = writer.lock().unwrap();
        stream.write_all(line.as_bytes())?;
        stream.flush()
    }

    fn send_error(
        writer: &Arc<Mutex<TcpStream>>,
        message: &str,
    ) -> std::io::Result<()> {
        Self::write(
            writer,
            &ServerMessage::Error {
                message: message.to_string(),
            },
        )
    }
}

#[derive(Debug)]
pub struct ControlClient {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    #[allow(dead_code)]
    player_id: String,
    state: RoomState,
}

impl ControlClient {
    pub fn connect(
        ip: &str,
        port: u16,
        room_id: &str,
        player: &Player,
        secret: Option<&str>,
        want: Want,
        timeout: Duration,
    ) -> std::io::Result<Self> {
        let stream = TcpStream::connect((ip, port))?;
        stream.set_nodelay(true).ok();
        stream.set_read_timeout(Some(timeout)).ok();
        let writer = stream.try_clone()?;
        let mut reader = BufReader::new(stream);

        let hello = ClientMessage::Hello {
            v: VERSION,
            room_id: room_id.to_string(),
            node_id: player.node_id.clone(),
            handle: player.handle.clone(),
            secret: secret.map(str::to_string),
            want,
        };
        write_line(&writer, &hello)?;

        let mut line = String::new();
        reader.read_line(&mut line)?;
        let welcome: ServerMessage = decode_line(&line)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        match welcome {
            ServerMessage::Welcome { player_id, state } => Ok(Self {
                stream: writer,
                reader,
                player_id,
                state,
            }),
            ServerMessage::Error { message } => {
                Err(std::io::Error::other(message))
            }
            _ => Err(std::io::Error::other("unexpected greeting")),
        }
    }

    #[allow(dead_code)]
    pub fn player_id(&self) -> &str {
        &self.player_id
    }

    pub fn state(&self) -> &RoomState {
        &self.state
    }

    pub fn send(&mut self, message: &ClientMessage) -> std::io::Result<()> {
        write_line(&self.stream, message)
    }

    pub fn poll(&mut self, timeout: Duration) -> std::io::Result<Option<RoomState>> {
        self.reader
            .get_ref()
            .set_read_timeout(Some(timeout))
            .ok();
        let mut line = String::new();
        let read = self.reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        let message: ServerMessage = decode_line(&line)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        match message {
            ServerMessage::State { state } => {
                self.state = state.clone();
                Ok(Some(state))
            }
            ServerMessage::Welcome { state, .. } => {
                self.state = state.clone();
                Ok(Some(state))
            }
            ServerMessage::Error { message } => Err(std::io::Error::other(message)),
            ServerMessage::Ping => Ok(None),
        }
    }
}

fn write_line<T: Serialize>(mut stream: &TcpStream, message: &T) -> std::io::Result<()> {
    let line = encode_line(message)?;
    stream.write_all(line.as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::Phase;
    use std::time::Instant;

    fn host() -> Player {
        Player::new("n-host", "Host", "100.0.0.1")
    }

    fn server() -> Arc<ControlServer> {
        let room = RoomState::new("room-1", host(), "sfiii3nr1");
        ControlServer::bind(room, Some("s3cret".to_string()), 0)
            .expect("bind")
            .spawn()
    }

    fn connect(server: &ControlServer, node: &str, secret: Option<&str>) -> std::io::Result<ControlClient> {
        let port = server.local_addr().unwrap().port();
        let player = Player::new(node, format!("handle-{node}"), String::new());
        ControlClient::connect(
            "127.0.0.1",
            port,
            "room-1",
            &player,
            secret,
            Want::Play,
            Duration::from_millis(500),
        )
    }

    fn wait_state(
        client: &mut ControlClient,
        predicate: impl Fn(&RoomState) -> bool,
        timeout: Duration,
    ) -> RoomState {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "timed out waiting for expected state");
            match client.poll(remaining) {
                Ok(Some(state)) if predicate(&state) => return state,
                Ok(_) | Err(_) => {}
            }
        }
    }

    #[test]
    fn messages_round_trip_through_a_line() {
        let original = ClientMessage::Hello {
            v: VERSION,
            room_id: "room-1".to_string(),
            node_id: "n-a".to_string(),
            handle: "A".to_string(),
            secret: Some("s".to_string()),
            want: Want::Play,
        };
        let line = encode_line(&original).unwrap();
        assert!(line.ends_with('\n'));
        assert_eq!(decode_line::<ClientMessage>(&line).unwrap(), original);
    }

    #[test]
    fn malformed_lines_are_rejected() {
        assert!(decode_line::<ClientMessage>("not json\n").is_err());
        assert!(decode_line::<ClientMessage>(r#"{"kind":"wat"}"#).is_err());
    }

    #[test]
    fn server_accepts_a_client_with_the_secret() {
        let server = server();
        let client = connect(&server, "n-a", Some("s3cret")).expect("connect");
        assert_eq!(client.player_id(), "n-a");
        assert_eq!(client.state().room_id, "room-1");
    }

    #[test]
    fn server_rejects_a_wrong_secret() {
        let server = server();
        let err = connect(&server, "n-a", Some("nope")).unwrap_err();
        assert!(err.to_string().contains("secret"));
    }

    #[test]
    fn server_rejects_a_missing_secret() {
        let server = server();
        let err = connect(&server, "n-a", None).unwrap_err();
        assert!(err.to_string().contains("secret"));
    }

    #[test]
    fn enqueue_broadcasts_state_to_all_clients() {
        let server = server();
        let mut a = connect(&server, "n-a", Some("s3cret")).expect("a");
        let mut b = connect(&server, "n-b", Some("s3cret")).expect("b");

        a.send(&ClientMessage::Enqueue).unwrap();

        let state_a = wait_state(
            &mut a,
            |state| state.champion.as_ref().is_some_and(|p| p.node_id == "n-a"),
            Duration::from_millis(500),
        );
        assert_eq!(state_a.ledger["n-a"].handle, "handle-n-a");

        let state_b = wait_state(
            &mut b,
            |state| state.champion.as_ref().is_some_and(|p| p.node_id == "n-a"),
            Duration::from_millis(500),
        );
        assert_eq!(state_b.champion.as_ref().unwrap().node_id, "n-a");
    }

    #[test]
    fn two_enqueues_start_a_match() {
        let server = server();
        let mut a = connect(&server, "n-a", Some("s3cret")).expect("a");
        let mut b = connect(&server, "n-b", Some("s3cret")).expect("b");

        a.send(&ClientMessage::Enqueue).unwrap();
        b.send(&ClientMessage::Enqueue).unwrap();

        let state = wait_state(
            &mut b,
            |state| state.phase == Phase::Playing,
            Duration::from_millis(500),
        );
        let current = state.current_match.as_ref().expect("match");
        assert!(current.names("n-a") && current.names("n-b"));
    }

    #[test]
    fn result_winner_keeps_throne_and_ledger_updates() {
        let server = server();
        let mut a = connect(&server, "n-a", Some("s3cret")).expect("a");
        let mut b = connect(&server, "n-b", Some("s3cret")).expect("b");

        a.send(&ClientMessage::Enqueue).unwrap();
        b.send(&ClientMessage::Enqueue).unwrap();
        let state = wait_state(
            &mut a,
            |state| state.current_match.is_some(),
            Duration::from_millis(500),
        );
        let match_id = state.current_match.as_ref().unwrap().match_id.clone();

        a.send(&ClientMessage::Result {
            match_id,
            outcome: Outcome::Win,
            confidence: Confidence::Overlay,
        })
        .unwrap();

        let state = wait_state(
            &mut a,
            |state| state.ledger.get("n-a").is_some_and(|entry| entry.wins == 1),
            Duration::from_millis(500),
        );
        assert_eq!(state.ledger["n-b"].losses, 1);
        assert_eq!(state.champion.as_ref().unwrap().node_id, "n-a");
        assert_eq!(state.phase, Phase::Playing);
    }

    #[test]
    fn reporting_a_loss_finds_the_opponent_as_winner() {
        let server = server();
        let mut a = connect(&server, "n-a", Some("s3cret")).expect("a");
        let mut b = connect(&server, "n-b", Some("s3cret")).expect("b");

        a.send(&ClientMessage::Enqueue).unwrap();
        b.send(&ClientMessage::Enqueue).unwrap();
        let state = wait_state(
            &mut a,
            |state| state.current_match.is_some(),
            Duration::from_millis(500),
        );
        let match_id = state.current_match.as_ref().unwrap().match_id.clone();

        a.send(&ClientMessage::Result {
            match_id,
            outcome: Outcome::Loss,
            confidence: Confidence::Manual,
        })
        .unwrap();

        let state = wait_state(
            &mut a,
            |state| state.ledger.get("n-b").is_some_and(|entry| entry.wins == 1),
            Duration::from_millis(500),
        );
        assert_eq!(state.ledger["n-a"].losses, 1);
    }

    #[test]
    fn a_stale_result_is_reported_as_an_error() {
        let server = server();
        let mut a = connect(&server, "n-a", Some("s3cret")).expect("a");
        a.send(&ClientMessage::Enqueue).unwrap();
        wait_state(
            &mut a,
            |state| state.champion.is_some(),
            Duration::from_millis(500),
        );

        a.send(&ClientMessage::Result {
            match_id: "room-1-999".to_string(),
            outcome: Outcome::Win,
            confidence: Confidence::Manual,
        })
        .unwrap();

        let err = a.poll(Duration::from_millis(500)).unwrap_err();
        assert!(err.to_string().contains("no active match"));
    }
}
