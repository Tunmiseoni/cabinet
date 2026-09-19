use crate::player::Player;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

pub const SIDE_CHAMPION: u8 = 0;
pub const SIDE_CHALLENGER: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Lobby,
    Playing,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    pub handle: String,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchSlot {
    pub node_id: String,
    pub handle: String,
    pub ip: String,
    pub side: u8,
}

impl MatchSlot {
    fn from_player(player: &Player, side: u8) -> Self {
        Self {
            node_id: player.node_id.clone(),
            handle: player.handle.clone(),
            ip: player.ip.clone(),
            side,
        }
    }

    fn to_player(&self) -> Player {
        Player::new(self.node_id.clone(), self.handle.clone(), self.ip.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentMatch {
    pub match_id: String,
    pub p1: MatchSlot,
    pub p2: MatchSlot,
    pub started_at_ms: u64,
}

impl CurrentMatch {
    pub fn names(&self, node_id: &str) -> bool {
        self.p1.node_id == node_id || self.p2.node_id == node_id
    }

    pub fn opponent_of(&self, node_id: &str) -> Option<&MatchSlot> {
        if self.p1.node_id == node_id {
            Some(&self.p2)
        } else if self.p2.node_id == node_id {
            Some(&self.p1)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomState {
    pub room_id: String,
    pub host: Player,
    pub rom: String,
    pub revision: u64,
    pub phase: Phase,
    pub champion: Option<Player>,
    pub challenger: Option<Player>,
    pub queue: VecDeque<Player>,
    pub current_match: Option<CurrentMatch>,
    pub ledger: BTreeMap<String, LedgerEntry>,
    pub match_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomError {
    AlreadyQueued(String),
    NotQueued(String),
    NoActiveMatch,
    StaleMatch { expected: String, got: String },
    UnknownPlayer(String),
}

impl std::fmt::Display for RoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoomError::AlreadyQueued(id) => write!(f, "{id} is already in the room"),
            RoomError::NotQueued(id) => write!(f, "{id} is not in the room"),
            RoomError::NoActiveMatch => write!(f, "no active match"),
            RoomError::StaleMatch { expected, got } => {
                write!(f, "stale result for match {got} (current {expected})")
            }
            RoomError::UnknownPlayer(id) => write!(f, "{id} is not in the current match"),
        }
    }
}

impl std::error::Error for RoomError {}

impl RoomState {
    pub fn new(room_id: impl Into<String>, host: Player, rom: impl Into<String>) -> Self {
        Self {
            room_id: room_id.into(),
            host,
            rom: rom.into(),
            revision: 0,
            phase: Phase::Lobby,
            champion: None,
            challenger: None,
            queue: VecDeque::new(),
            current_match: None,
            ledger: BTreeMap::new(),
            match_seq: 0,
        }
    }

    fn bump(&mut self) {
        self.revision += 1;
    }

    fn known(&self, node_id: &str) -> bool {
        self.champion
            .as_ref()
            .is_some_and(|p| p.node_id == node_id)
            || self
                .challenger
                .as_ref()
                .is_some_and(|p| p.node_id == node_id)
            || self.queue.iter().any(|p| p.node_id == node_id)
    }

    fn in_match(&self, node_id: &str) -> bool {
        self.current_match
            .as_ref()
            .is_some_and(|m| m.names(node_id))
    }

    pub fn enqueue(&mut self, player: Player, now_ms: u64) -> Result<(), RoomError> {
        if self.known(&player.node_id) || self.in_match(&player.node_id) {
            return Err(RoomError::AlreadyQueued(player.node_id));
        }

        let entry = self.ledger.entry(player.node_id.clone()).or_default();
        entry.handle = player.handle.clone();

        if self.champion.is_none() {
            self.champion = Some(player);
        } else if self.challenger.is_none() {
            self.challenger = Some(player);
        } else {
            self.queue.push_back(player);
        }

        self.bump();
        self.schedule(now_ms);
        Ok(())
    }

    pub fn leave(&mut self, node_id: &str, now_ms: u64) -> Result<(), RoomError> {
        let was_match = self.in_match(node_id);
        let mut removed = false;

        if self.champion.as_ref().is_some_and(|p| p.node_id == node_id) {
            self.champion = None;
            removed = true;
        }
        if self.challenger.as_ref().is_some_and(|p| p.node_id == node_id) {
            self.challenger = None;
            removed = true;
        }
        let before = self.queue.len();
        self.queue.retain(|p| p.node_id != node_id);
        removed |= self.queue.len() != before;

        if !removed && !was_match {
            return Err(RoomError::NotQueued(node_id.to_string()));
        }

        if was_match {
            self.current_match = None;
            self.phase = Phase::Lobby;
        }

        if self.champion.is_none() {
            self.champion = self.queue.pop_front();
        }
        if self.challenger.is_none() {
            self.challenger = self.queue.pop_front();
        }

        self.bump();
        self.schedule(now_ms);
        Ok(())
    }

    pub fn report_result(
        &mut self,
        match_id: &str,
        winner_node_id: &str,
        now_ms: u64,
    ) -> Result<(), RoomError> {
        let current = self.current_match.clone().ok_or(RoomError::NoActiveMatch)?;
        if current.match_id != match_id {
            return Err(RoomError::StaleMatch {
                expected: current.match_id,
                got: match_id.to_string(),
            });
        }

        let winner = if current.p1.node_id == winner_node_id {
            current.p1.clone()
        } else if current.p2.node_id == winner_node_id {
            current.p2.clone()
        } else {
            return Err(RoomError::UnknownPlayer(winner_node_id.to_string()));
        };
        let loser = if winner.side == current.p1.side {
            current.p2.clone()
        } else {
            current.p1.clone()
        };

        self.record(&winner, true);
        self.record(&loser, false);

        self.current_match = None;
        self.champion = Some(winner.to_player());
        self.queue.push_back(loser.to_player());
        self.challenger = self.queue.pop_front();

        self.bump();
        self.schedule(now_ms);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn report_draw(&mut self, match_id: &str, now_ms: u64) -> Result<(), RoomError> {
        let current = self.current_match.clone().ok_or(RoomError::NoActiveMatch)?;
        if current.match_id != match_id {
            return Err(RoomError::StaleMatch {
                expected: current.match_id,
                got: match_id.to_string(),
            });
        }

        self.current_match = None;
        self.bump();
        self.schedule(now_ms);
        Ok(())
    }

    fn record(&mut self, slot: &MatchSlot, won: bool) {
        let entry = self.ledger.entry(slot.node_id.clone()).or_default();
        entry.handle = slot.handle.clone();
        if won {
            entry.wins += 1;
        } else {
            entry.losses += 1;
        }
        entry.games += 1;
    }

    fn schedule(&mut self, now_ms: u64) {
        if self.current_match.is_some() {
            return;
        }

        let (Some(champion), Some(challenger)) =
            (self.champion.clone(), self.challenger.clone())
        else {
            self.phase = Phase::Lobby;
            return;
        };

        self.match_seq += 1;
        self.current_match = Some(CurrentMatch {
            match_id: format!("{}-{}", self.room_id, self.match_seq),
            p1: MatchSlot::from_player(&champion, SIDE_CHAMPION),
            p2: MatchSlot::from_player(&challenger, SIDE_CHALLENGER),
            started_at_ms: now_ms,
        });
        self.phase = Phase::Playing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(id: &str) -> Player {
        Player::new(id, format!("handle-{id}"), format!("100.0.0.{id}"))
    }

    fn room() -> RoomState {
        RoomState::new("room", player("host"), "sfiii3nr1")
    }

    #[test]
    fn first_enqueue_becomes_champion_without_a_match() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        assert_eq!(room.champion.as_ref().unwrap().node_id, "a");
        assert!(room.challenger.is_none());
        assert!(room.current_match.is_none());
        assert_eq!(room.phase, Phase::Lobby);
    }

    #[test]
    fn second_enqueue_starts_the_first_match() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        room.enqueue(player("b"), 10).unwrap();

        let current = room.current_match.as_ref().expect("match scheduled");
        assert_eq!(current.p1.node_id, "a");
        assert_eq!(current.p1.side, SIDE_CHAMPION);
        assert_eq!(current.p2.node_id, "b");
        assert_eq!(current.p2.side, SIDE_CHALLENGER);
        assert_eq!(current.started_at_ms, 10);
        assert_eq!(room.phase, Phase::Playing);
    }

    #[test]
    fn third_enqueue_joins_the_queue() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        room.enqueue(player("b"), 0).unwrap();
        room.enqueue(player("c"), 0).unwrap();
        assert_eq!(room.queue.len(), 1);
        assert_eq!(room.queue[0].node_id, "c");
    }

    #[test]
    fn champion_win_keeps_throne_and_loser_goes_to_back() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        room.enqueue(player("b"), 0).unwrap();
        room.enqueue(player("c"), 0).unwrap();

        room.report_result("room-1", "a", 100).unwrap();

        assert_eq!(room.champion.as_ref().unwrap().node_id, "a");
        assert_eq!(room.challenger.as_ref().unwrap().node_id, "c");
        assert_eq!(room.queue.len(), 1);
        assert_eq!(room.queue[0].node_id, "b");

        let current = room.current_match.as_ref().unwrap();
        assert_eq!(current.p1.node_id, "a");
        assert_eq!(current.p2.node_id, "c");
        assert_eq!(current.match_id, "room-2");

        assert_eq!(room.ledger["a"].wins, 1);
        assert_eq!(room.ledger["b"].losses, 1);
        assert_eq!(room.ledger["a"].games, 1);
    }

    #[test]
    fn challenger_win_promotes_and_requeues_former_champion() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        room.enqueue(player("b"), 0).unwrap();

        room.report_result("room-1", "b", 50).unwrap();

        assert_eq!(room.champion.as_ref().unwrap().node_id, "b");
        assert_eq!(room.challenger.as_ref().unwrap().node_id, "a");
        assert_eq!(room.ledger["b"].wins, 1);
        assert_eq!(room.ledger["a"].losses, 1);
    }

    #[test]
    fn draw_replays_the_same_pairing_without_touching_the_ledger() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        room.enqueue(player("b"), 0).unwrap();
        let first_id = room.current_match.as_ref().unwrap().match_id.clone();

        room.report_draw(&first_id, 20).unwrap();

        let current = room.current_match.as_ref().expect("replayed match");
        assert_ne!(current.match_id, first_id);
        assert_eq!(current.p1.node_id, "a");
        assert_eq!(current.p2.node_id, "b");
        assert!(room.ledger.values().all(|entry| entry.games == 0));
    }

    #[test]
    fn stale_result_is_rejected() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        room.enqueue(player("b"), 0).unwrap();

        let err = room.report_result("room-999", "a", 0).unwrap_err();
        assert!(matches!(err, RoomError::StaleMatch { .. }));
        assert_eq!(room.current_match.as_ref().unwrap().match_id, "room-1");
    }

    #[test]
    fn result_for_a_bystander_is_rejected() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        room.enqueue(player("b"), 0).unwrap();

        let err = room.report_result("room-1", "c", 0).unwrap_err();
        assert_eq!(err, RoomError::UnknownPlayer("c".to_string()));
    }

    #[test]
    fn duplicate_enqueue_is_rejected() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        let err = room.enqueue(player("a"), 0).unwrap_err();
        assert_eq!(err, RoomError::AlreadyQueued("a".to_string()));
    }

    #[test]
    fn leaving_mid_match_voids_it_and_backfills_from_the_queue() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        room.enqueue(player("b"), 0).unwrap();
        room.enqueue(player("c"), 0).unwrap();

        room.leave("a", 0).unwrap();

        assert_eq!(room.champion.as_ref().unwrap().node_id, "c");
        assert_eq!(room.challenger.as_ref().unwrap().node_id, "b");
        assert!(room.current_match.is_some());
    }

    #[test]
    fn leaving_alone_is_rejected() {
        let mut room = room();
        let err = room.leave("ghost", 0).unwrap_err();
        assert_eq!(err, RoomError::NotQueued("ghost".to_string()));
    }

    #[test]
    fn enqueue_creates_a_zeroed_ledger_entry() {
        let mut room = room();
        room.enqueue(player("a"), 0).unwrap();
        let entry = &room.ledger["a"];
        assert_eq!(entry.handle, "handle-a");
        assert_eq!(entry.games, 0);
    }

    #[test]
    fn revision_advances_on_every_change() {
        let mut room = room();
        let start = room.revision;
        room.enqueue(player("a"), 0).unwrap();
        assert!(room.revision > start);
        let after = room.revision;
        room.enqueue(player("b"), 0).unwrap();
        assert!(room.revision > after);
    }
}
