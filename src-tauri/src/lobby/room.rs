use serde::{Deserialize, Serialize};

use super::sets::{Rotation, Seats, SetScore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RoomPhase {
    #[default]
    Waiting,
    Playing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Room {
    pub room_id: String,
    pub host_node_id: String,
    pub host_handle: String,
    pub rom: String,
    pub first_to: u8,
    pub phase: RoomPhase,
    pub players: u8,
    pub spectators: u8,
    /// The seat the room's host held when it created the room, if it plays. Kept as the host's
    /// declared intent; the live occupant of each slot is `seats`.
    pub host_seat: Option<u8>,
    /// The netplay nickname holding each player slot (`[P1, P2]`), as the host observes it.
    /// Defaulted so a beacon from an older app version still parses.
    #[serde(default)]
    pub seats: [Option<String>; 2],
    /// Netplay nicknames waiting for a player slot, longest-waiting first.
    #[serde(default)]
    pub queue: Vec<String>,
    /// The set in progress. A win is a whole game (first to two rounds); the set is first to N.
    #[serde(default)]
    pub set: SetScore,
    /// The set-end rotation each machine should act on once, if a spectator is waiting.
    #[serde(default)]
    pub rotation: Option<Rotation>,
    pub revision: u64,
}

impl Room {
    pub(crate) fn new(
        room_id: impl Into<String>,
        host_node_id: impl Into<String>,
        host_handle: impl Into<String>,
        rom: impl Into<String>,
        first_to: u8,
        host_seat: Option<u8>,
    ) -> Self {
        Self {
            room_id: room_id.into(),
            host_node_id: host_node_id.into(),
            host_handle: host_handle.into(),
            rom: rom.into(),
            first_to,
            phase: RoomPhase::Waiting,
            players: 0,
            spectators: 0,
            host_seat,
            seats: [None, None],
            queue: Vec::new(),
            set: SetScore::default(),
            rotation: None,
            revision: 0,
        }
    }

    pub(crate) fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Replace the seated players and the waiting queue, then refresh the derived counters.
    /// Seat-only (no queue) when the host is seeding its own slot at room creation.
    pub(crate) fn set_seats(&mut self, seats: &Seats) {
        if self.seats == seats.slots && self.queue == seats.queue {
            return;
        }
        self.seats = seats.slots.clone();
        self.queue = seats.queue.clone();
        self.refresh_occupancy();
    }

    pub(crate) fn set_score(&mut self, score: SetScore) {
        if self.set == score {
            return;
        }
        self.set = score;
        self.bump();
    }

    pub(crate) fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = Some(rotation);
        self.bump();
    }

    /// Recompute `players`/`spectators`/`phase` from the live seat map and queue.
    pub(crate) fn refresh_occupancy(&mut self) {
        let players = self.seats.iter().filter(|slot| slot.is_some()).count() as u8;
        let spectators = self.queue.len() as u8;
        let phase = if players >= 2 {
            RoomPhase::Playing
        } else {
            RoomPhase::Waiting
        };
        if self.players == players && self.spectators == spectators && self.phase == phase {
            return;
        }
        self.players = players;
        self.spectators = spectators;
        self.phase = phase;
        self.bump();
    }

    /// The live seat map / queue, as a `Seats` value for the rotation logic.
    pub(crate) fn seats(&self) -> Seats {
        Seats {
            slots: self.seats.clone(),
            queue: self.queue.clone(),
        }
    }
}

pub(crate) fn new_room_id() -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{}-{seq}", crate::time::now_ms(), std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_room_is_waiting() {
        let room = Room::new("r", "node", "player-one", "sfiii3nr1", 2, Some(1));
        assert_eq!(room.phase, RoomPhase::Waiting);
        assert_eq!(room.players, 0);
        assert_eq!(room.revision, 0);
        assert_eq!(room.host_seat, Some(1));
    }

    #[test]
    fn seats_drive_the_phase_and_bump_the_revision() {
        let mut room = Room::new("r", "node", "player-one", "sfiii3nr1", 2, None);
        let mut seats = Seats::default();
        seats.set(1, Some("player-one".into()));
        room.set_seats(&seats);
        assert_eq!(room.phase, RoomPhase::Waiting);
        assert_eq!(room.players, 1);
        assert_eq!(room.revision, 1);

        seats.set(2, Some("player-two".into()));
        seats.enqueue("player-three".into());
        room.set_seats(&seats);
        assert_eq!(room.phase, RoomPhase::Playing);
        assert_eq!(room.players, 2);
        assert_eq!(room.spectators, 1);
        assert_eq!(room.revision, 2);
    }

    #[test]
    fn setting_identical_seats_does_not_bump_the_revision() {
        let mut room = Room::new("r", "node", "player-one", "sfiii3nr1", 2, None);
        let mut seats = Seats::default();
        seats.set(1, Some("player-one".into()));
        room.set_seats(&seats);
        let revision = room.revision;
        room.set_seats(&seats);
        assert_eq!(room.revision, revision);
    }

    #[test]
    fn the_score_is_tracked_and_bumps_the_revision() {
        let mut room = Room::new("r", "node", "player-one", "sfiii3nr1", 2, None);
        room.set_score(SetScore {
            p1_games: 1,
            ..Default::default()
        });
        assert_eq!(room.set.p1_games, 1);
        assert_eq!(room.revision, 1);
        room.set_score(SetScore {
            p1_games: 1,
            ..Default::default()
        });
        assert_eq!(room.revision, 1);
    }

    #[test]
    fn round_trips_through_json() {
        let mut room = Room::new("r", "node", "player-one", "sfiii3nr1", 3, Some(2));
        let mut seats = Seats::default();
        seats.set(1, Some("player-two".into()));
        seats.set(2, Some("player-one".into()));
        seats.enqueue("player-three".into());
        room.set_seats(&seats);
        let json = serde_json::to_string(&room).unwrap();
        assert!(json.contains("\"phase\":\"playing\""));
        assert!(json.contains("\"firstTo\":3"));
        assert!(json.contains("\"hostSeat\":2"));
        assert!(json.contains("\"seats\":[\"player-two\",\"player-one\"]"));
        assert!(json.contains("\"queue\":[\"player-three\"]"));
        let loaded: Room = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, room);
    }

    #[test]
    fn room_ids_are_distinct() {
        assert_ne!(new_room_id(), new_room_id());
    }
}
