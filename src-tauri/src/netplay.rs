use crate::providers::Role;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum NetplayConnection {
    #[default]
    Connecting,
    Connected,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetplayPlayer {
    pub nick: String,
    pub player: u8,
    pub ping_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetplayEvent {
    pub at_ms: u64,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NetplayInfo {
    pub connection: NetplayConnection,
    pub self_player: Option<u8>,
    pub host: Option<String>,
    pub players: Vec<NetplayPlayer>,
    pub ping_ms: Option<u64>,
    pub core_warning: bool,
    pub last_event: Option<String>,
    pub events: Vec<NetplayEvent>,
}

impl NetplayInfo {
    pub fn connecting() -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
pub struct NetplayTracker {
    info: NetplayInfo,
    dirty: bool,
}

impl NetplayTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn info(&self) -> &NetplayInfo {
        &self.info
    }

    pub fn feed(&mut self, role: Role, line: &str) -> bool {
        if let Some(message) = netplay_message(line) {
            self.apply(role, message, crate::time::now_ms());
        }
        std::mem::take(&mut self.dirty)
    }

    fn apply(&mut self, role: Role, message: &str, at_ms: u64) {
        // F1: the host reverse-probe with `netplay_nat_traversal=false` logs these on
        // every join. They are benign and must never surface as an error.
        if message == "Failed to connect to client."
            || message == "A netplay client has disconnected"
        {
            return;
        }

        if let Some(rest) = message.strip_prefix("You have joined as player ") {
            if let Ok(player) = rest.trim().parse::<u8>() {
                self.set_self_player(Some(player));
                self.set_connection(NetplayConnection::Connected);
                self.push_event(at_ms, "joined", message);
                return;
            }
        }

        if let Some(rest) = message.strip_prefix("Connected to: ") {
            if let Some(nick) = quoted(rest) {
                self.set_host(nick);
                self.set_connection(NetplayConnection::Connected);
                self.ensure_self_slot(role);
                self.push_event(at_ms, "connected", message);
                return;
            }
        }

        if let Some(rest) = message.strip_prefix("Got connection from: ") {
            if quoted(rest).is_some() {
                self.push_event(at_ms, "peer", message);
                return;
            }
        }

        if let Some(rest) = message.strip_prefix("Connection slot ") {
            if rest.trim().parse::<u8>().is_ok() {
                self.push_event(at_ms, "slot", message);
                return;
            }
        }

        if let Some((nick, rest)) = message.split_once(" has joined as player ") {
            if let Some(player) = rest.split_whitespace().next().and_then(|t| t.parse().ok()) {
                self.upsert_player(nick.trim(), player, parse_ping(rest));
                self.set_connection(NetplayConnection::Connected);
                self.push_event(at_ms, "joined", message);
                return;
            }
        }

        if let Some(nick) = message
            .strip_prefix("Player ")
            .and_then(|rest| rest.strip_suffix(" has left the game"))
        {
            self.remove_player(nick);
            self.push_event(at_ms, "left", message);
            return;
        }

        if message == "You have left the game" {
            self.set_connection(NetplayConnection::Disconnected);
            self.set_self_player(None);
            self.push_event(at_ms, "left", message);
            return;
        }

        if message.ends_with(" has disconnected") {
            if let Some(nick) = quoted(message) {
                self.remove_player(nick);
                self.push_event(at_ms, "disconnected", message);
                return;
            }
        }

        if message == "Netplay disconnected" {
            self.set_connection(NetplayConnection::Disconnected);
            self.set_self_player(None);
            self.push_event(at_ms, "disconnected", message);
            return;
        }

        if message.starts_with("Failed to initialize netplay")
            || message.starts_with("Failed to set up netplay sockets")
        {
            self.set_connection(NetplayConnection::Failed);
            self.push_event(at_ms, "failed", message);
            return;
        }

        if message.starts_with("WARNING: A netplay peer is running a different version of the core")
            && !self.info.core_warning
        {
            self.info.core_warning = true;
            self.push_event(at_ms, "warning", message);
        }
    }

    fn set_connection(&mut self, connection: NetplayConnection) {
        if self.info.connection != connection {
            self.info.connection = connection;
            self.dirty = true;
        }
    }

    fn set_self_player(&mut self, player: Option<u8>) {
        if self.info.self_player != player {
            self.info.self_player = player;
            self.dirty = true;
        }
    }

    fn set_host(&mut self, nick: &str) {
        if self.info.host.as_deref() != Some(nick) {
            self.info.host = Some(nick.to_string());
            self.dirty = true;
        }
    }

    fn ensure_self_slot(&mut self, role: Role) {
        if self.info.self_player.is_none() {
            if let Some(side) = role.side() {
                self.set_self_player(Some(side + 1));
            }
        }
    }

    fn upsert_player(&mut self, nick: &str, player: u8, ping_ms: Option<u64>) {
        let mut changed = false;
        if let Some(entry) = self
            .info
            .players
            .iter_mut()
            .find(|entry| entry.nick == nick)
        {
            if entry.player != player || entry.ping_ms != ping_ms {
                entry.player = player;
                entry.ping_ms = ping_ms;
                changed = true;
            }
        } else {
            self.info.players.push(NetplayPlayer {
                nick: nick.to_string(),
                player,
                ping_ms,
            });
            self.info.players.sort_by_key(|entry| entry.player);
            changed = true;
        }
        if changed {
            self.dirty = true;
        }
        self.refresh_ping();
    }

    fn remove_player(&mut self, nick: &str) {
        let before = self.info.players.len();
        self.info.players.retain(|entry| entry.nick != nick);
        if self.info.players.len() != before {
            self.dirty = true;
        }
        self.refresh_ping();
    }

    fn refresh_ping(&mut self) {
        let ping = self
            .info
            .players
            .iter()
            .filter_map(|entry| entry.ping_ms)
            .max();
        if self.info.ping_ms != ping {
            self.info.ping_ms = ping;
            self.dirty = true;
        }
    }

    fn push_event(&mut self, at_ms: u64, kind: &str, text: &str) {
        self.info.events.push(NetplayEvent {
            at_ms,
            kind: kind.to_string(),
            text: text.to_string(),
        });
        let limit = crate::constants::NETPLAY_EVENT_LIMIT;
        if self.info.events.len() > limit {
            let excess = self.info.events.len() - limit;
            self.info.events.drain(0..excess);
        }
        self.info.last_event = Some(text.to_string());
        self.dirty = true;
    }
}

fn netplay_message(line: &str) -> Option<&str> {
    let index = line.find("[Netplay]")?;
    let message = line[index + "[Netplay]".len()..].trim();
    if message.is_empty() {
        None
    } else {
        Some(message)
    }
}

fn quoted(text: &str) -> Option<&str> {
    let start = text.find('"')? + 1;
    let end = text[start..].find('"')? + start;
    Some(&text[start..end])
}

fn parse_ping(text: &str) -> Option<u64> {
    let start = text.find("(ping: ")? + "(ping: ".len();
    let tail = &text[start..];
    let end = tail.find(" ms)")?;
    tail[..end].trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_tracker() -> NetplayTracker {
        NetplayTracker::new()
    }

    #[test]
    fn host_tracks_a_peer_join_and_leave() {
        let mut tracker = new_tracker();
        assert!(tracker.feed(Role::P1, "[INFO] [Netplay] You have joined as player 1"));
        assert_eq!(tracker.info().connection, NetplayConnection::Connected);
        assert_eq!(tracker.info().self_player, Some(1));

        assert!(!tracker.feed(Role::P1, "[ERROR] [Netplay] Failed to connect to client."));
        assert!(!tracker.feed(
            Role::P1,
            "[INFO] [Netplay] A netplay client has disconnected"
        ));
        assert_eq!(tracker.info().connection, NetplayConnection::Connected);

        assert!(tracker.feed(
            Role::P1,
            "[INFO] [Netplay] Got connection from: \"player-two\""
        ));
        assert!(tracker.feed(
            Role::P1,
            "[INFO] [Netplay] player-two has joined as player 2 (ping: 82 ms)"
        ));
        assert_eq!(
            tracker.info().players,
            vec![NetplayPlayer {
                nick: "player-two".into(),
                player: 2,
                ping_ms: Some(82)
            }]
        );
        assert_eq!(tracker.info().ping_ms, Some(82));

        assert!(tracker.feed(
            Role::P1,
            "[INFO] [Netplay] Player player-two has left the game"
        ));
        assert!(tracker.info().players.is_empty());
        assert_eq!(tracker.info().ping_ms, None);
    }

    #[test]
    fn spectator_sees_the_host_and_no_ping_joins() {
        let mut tracker = new_tracker();
        assert!(tracker.feed(
            Role::Spectator,
            "[INFO] [Netplay] Connected to: \"player-one\""
        ));
        assert_eq!(tracker.info().host.as_deref(), Some("player-one"));
        assert_eq!(tracker.info().connection, NetplayConnection::Connected);
        assert_eq!(tracker.info().self_player, None);

        assert!(tracker.feed(
            Role::Spectator,
            "[INFO] [Netplay] player-one has joined as player 1"
        ));
        assert!(tracker.feed(
            Role::Spectator,
            "[INFO] [Netplay] player-two has joined as player 2"
        ));
        assert_eq!(tracker.info().players.len(), 2);
        assert_eq!(tracker.info().ping_ms, None);

        assert!(tracker.feed(Role::Spectator, "[INFO] [Netplay] Netplay disconnected"));
        assert_eq!(tracker.info().connection, NetplayConnection::Disconnected);
    }

    #[test]
    fn client_defaults_its_self_slot_from_the_role() {
        let mut tracker = new_tracker();
        assert!(tracker.feed(Role::P2, "[INFO] [Netplay] Connected to: \"player-one\""));
        assert_eq!(tracker.info().self_player, Some(2));
    }

    #[test]
    fn failure_lines_mark_the_connection_failed() {
        let mut tracker = new_tracker();
        assert!(tracker.feed(Role::P2, "[ERROR] [Netplay] Failed to initialize netplay."));
        assert_eq!(tracker.info().connection, NetplayConnection::Failed);

        let mut tracker = new_tracker();
        assert!(tracker.feed(
            Role::P2,
            "[ERROR] [Netplay] Failed to set up netplay sockets."
        ));
        assert_eq!(tracker.info().connection, NetplayConnection::Failed);
    }

    #[test]
    fn quoted_disconnect_removes_the_player() {
        let mut tracker = new_tracker();
        tracker.feed(
            Role::P1,
            "[INFO] [Netplay] player-two has joined as player 2 (ping: 50 ms)",
        );
        assert!(tracker.feed(Role::P1, "[INFO] [Netplay] \"player-two\" has disconnected"));
        assert!(tracker.info().players.is_empty());
        assert_eq!(tracker.info().ping_ms, None);
    }

    #[test]
    fn core_warning_is_recorded_once() {
        let mut tracker = new_tracker();
        let warning = "[WARN] [Netplay] WARNING: A netplay peer is running a different version of the core. If problems occur, use the same version.";
        assert!(tracker.feed(Role::P1, warning));
        assert!(tracker.info().core_warning);
        let events = tracker.info().events.len();
        assert!(!tracker.feed(Role::P1, warning));
        assert_eq!(tracker.info().events.len(), events);
    }

    #[test]
    fn events_are_capped_and_keep_the_newest() {
        let mut tracker = new_tracker();
        let limit = crate::constants::NETPLAY_EVENT_LIMIT;
        for slot in 0..limit + 5 {
            tracker.feed(
                Role::P1,
                &format!("[INFO] [Netplay] Connection slot {slot}"),
            );
        }
        assert_eq!(tracker.info().events.len(), limit);
        let last = format!("Connection slot {}", limit + 4);
        assert_eq!(tracker.info().last_event.as_deref(), Some(last.as_str()));
    }

    #[test]
    fn non_netplay_lines_are_ignored() {
        let mut tracker = new_tracker();
        assert!(!tracker.feed(Role::P1, "[INFO] [Content] CRC32: 0x46119843."));
        assert!(!tracker.feed(Role::P1, "noise"));
        assert_eq!(tracker.info(), &NetplayInfo::connecting());
    }
}
