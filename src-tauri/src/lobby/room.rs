use serde::{Deserialize, Serialize};

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
    pub revision: u64,
}

impl Room {
    pub(crate) fn new(
        room_id: impl Into<String>,
        host_node_id: impl Into<String>,
        host_handle: impl Into<String>,
        rom: impl Into<String>,
        first_to: u8,
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
            revision: 0,
        }
    }

    pub(crate) fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(crate) fn set_occupancy(&mut self, players: u8, spectators: u8) {
        self.players = players;
        self.spectators = spectators;
        self.phase = if players >= 2 {
            RoomPhase::Playing
        } else {
            RoomPhase::Waiting
        };
        self.bump();
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
        let room = Room::new("r", "node", "player-one", "sfiii3nr1", 2);
        assert_eq!(room.phase, RoomPhase::Waiting);
        assert_eq!(room.players, 0);
        assert_eq!(room.revision, 0);
    }

    #[test]
    fn occupancy_flips_the_phase_and_bumps_the_revision() {
        let mut room = Room::new("r", "node", "player-one", "sfiii3nr1", 2);
        room.set_occupancy(1, 0);
        assert_eq!(room.phase, RoomPhase::Waiting);
        assert_eq!(room.revision, 1);
        room.set_occupancy(2, 1);
        assert_eq!(room.phase, RoomPhase::Playing);
        assert_eq!(room.spectators, 1);
        assert_eq!(room.revision, 2);
    }

    #[test]
    fn round_trips_through_json() {
        let mut room = Room::new("r", "node", "player-one", "sfiii3nr1", 3);
        room.set_occupancy(2, 1);
        let json = serde_json::to_string(&room).unwrap();
        assert!(json.contains("\"phase\":\"playing\""));
        assert!(json.contains("\"firstTo\":3"));
        let loaded: Room = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, room);
    }

    #[test]
    fn room_ids_are_distinct() {
        assert_ne!(new_room_id(), new_room_id());
    }
}
