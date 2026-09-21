use serde::{Deserialize, Serialize};

use super::results::{RoundOutcome, Winner};

/// Round wins needed to take a game. SF3 is best-of-three, so two.
pub(crate) const ROUND_WINS_PER_GAME: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetScore {
    pub p1_games: u8,
    pub p2_games: u8,
    pub p1_rounds: u8,
    pub p2_rounds: u8,
}

/// A round outcome rolled up to the game/set level. A draw is surfaced but never advances
/// anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SetEvent {
    Round(Winner),
    GameWon(Winner),
    SetWon(Winner),
}

/// First-to-N **games**, where a game is first to two round wins (SF3's own rule). A draw does not
/// count as a round for either side, so it neither advances the game nor rotates anyone. The set
/// counter resets once a set is won, ready for the next one.
#[derive(Debug, Clone)]
pub(crate) struct SetMachine {
    first_to: u8,
    score: SetScore,
}

impl SetMachine {
    pub(crate) fn new(first_to: u8) -> Self {
        Self {
            first_to: first_to.clamp(1, 9),
            score: SetScore::default(),
        }
    }

    pub(crate) fn score(&self) -> SetScore {
        self.score
    }

    pub(crate) fn observe(&mut self, outcome: RoundOutcome) -> SetEvent {
        match outcome.winner {
            Winner::Draw => return SetEvent::Round(Winner::Draw),
            Winner::P1 => self.score.p1_rounds = self.score.p1_rounds.saturating_add(1),
            Winner::P2 => self.score.p2_rounds = self.score.p2_rounds.saturating_add(1),
        }

        if self.score.p1_rounds < ROUND_WINS_PER_GAME && self.score.p2_rounds < ROUND_WINS_PER_GAME
        {
            return SetEvent::Round(outcome.winner);
        }

        let game_winner = if self.score.p1_rounds > self.score.p2_rounds {
            Winner::P1
        } else {
            Winner::P2
        };
        match game_winner {
            Winner::P1 => self.score.p1_games = self.score.p1_games.saturating_add(1),
            Winner::P2 => self.score.p2_games = self.score.p2_games.saturating_add(1),
            Winner::Draw => unreachable!("a game cannot be won by a draw"),
        }
        self.score.p1_rounds = 0;
        self.score.p2_rounds = 0;

        if self.score.p1_games >= self.first_to || self.score.p2_games >= self.first_to {
            let set_winner = if self.score.p1_games > self.score.p2_games {
                Winner::P1
            } else {
                Winner::P2
            };
            self.score = SetScore::default();
            return SetEvent::SetWon(set_winner);
        }

        SetEvent::GameWon(game_winner)
    }
}

/// The player slots and the FIFO of spectators waiting for one. Seats are keyed by netplay
/// nickname, the identity both the host (via its netplay log) and each client (via its config)
/// can observe independently.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Seats {
    pub slots: [Option<String>; 2],
    pub queue: Vec<String>,
}

impl Seats {
    pub(crate) fn slot_of(&self, nick: &str) -> Option<u8> {
        self.slots
            .iter()
            .position(|slot| slot.as_deref() == Some(nick))
            .map(|index| index as u8 + 1)
    }

    pub(crate) fn set(&mut self, slot: u8, nick: Option<String>) {
        if !(1..=2).contains(&slot) {
            return;
        }
        if let Some(nick) = nick.as_deref() {
            self.queue.retain(|entry| entry != nick);
        }
        self.slots[(slot - 1) as usize] = nick;
    }

    pub(crate) fn enqueue(&mut self, nick: String) {
        if !self.queue.iter().any(|entry| entry == &nick) {
            self.queue.push(nick);
        }
    }
}

/// An advertised set-end rotation: the losing slot must step out and the longest-waiting spectator
/// (`incoming`) steps in. Each machine acts on it by toggling its *own* instance once, and only
/// once per `id`. The queue and seats themselves always come from netplay observation, so this is
/// an event, not authoritative state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Rotation {
    pub id: u64,
    pub loser_slot: u8,
    pub incoming: Option<String>,
}

/// Plan the rotation that follows a set won in `winner_slot`: the loser's slot changes hands to the
/// queue head. `None` when nobody is waiting — the same two players simply start the next set.
pub(crate) fn plan_rotation(seats: &Seats, winner_slot: u8, id: u64) -> Option<Rotation> {
    if !(1..=2).contains(&winner_slot) {
        return None;
    }
    let incoming = seats.queue.first().cloned()?;
    Some(Rotation {
        id,
        loser_slot: if winner_slot == 1 { 2 } else { 1 },
        incoming: Some(incoming),
    })
}

pub(crate) fn winner_slot(winner: Winner) -> Option<u8> {
    match winner {
        Winner::P1 => Some(1),
        Winner::P2 => Some(2),
        Winner::Draw => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(winner: Winner) -> RoundOutcome {
        RoundOutcome { winner }
    }

    #[test]
    fn a_game_is_first_to_two_rounds() {
        let mut set = SetMachine::new(2);
        assert_eq!(set.observe(round(Winner::P1)), SetEvent::Round(Winner::P1));
        assert_eq!(
            set.observe(round(Winner::P1)),
            SetEvent::GameWon(Winner::P1)
        );
        assert_eq!(
            set.score(),
            SetScore {
                p1_games: 1,
                p2_games: 0,
                ..Default::default()
            }
        );
    }

    #[test]
    fn a_loss_does_not_reset_the_round_count() {
        let mut set = SetMachine::new(2);
        set.observe(round(Winner::P1));
        set.observe(round(Winner::P2));
        assert_eq!(set.score().p1_rounds, 1);
        assert_eq!(set.score().p2_rounds, 1);
        assert_eq!(
            set.observe(round(Winner::P2)),
            SetEvent::GameWon(Winner::P2)
        );
    }

    #[test]
    fn the_set_is_first_to_n_games() {
        let mut set = SetMachine::new(2);
        assert_eq!(set.observe(round(Winner::P1)), SetEvent::Round(Winner::P1));
        assert_eq!(
            set.observe(round(Winner::P1)),
            SetEvent::GameWon(Winner::P1)
        );
        assert_eq!(set.observe(round(Winner::P1)), SetEvent::Round(Winner::P1));
        assert_eq!(set.observe(round(Winner::P1)), SetEvent::SetWon(Winner::P1));
        assert_eq!(set.score(), SetScore::default());
    }

    #[test]
    fn a_new_set_clears_the_score() {
        let mut set = SetMachine::new(1);
        assert_eq!(set.observe(round(Winner::P2)), SetEvent::Round(Winner::P2));
        assert_eq!(set.observe(round(Winner::P2)), SetEvent::SetWon(Winner::P2));
        assert_eq!(set.score(), SetScore::default());
    }

    #[test]
    fn a_draw_advances_nothing() {
        let mut set = SetMachine::new(2);
        set.observe(round(Winner::P1));
        assert_eq!(
            set.observe(round(Winner::Draw)),
            SetEvent::Round(Winner::Draw)
        );
        assert_eq!(
            set.score(),
            SetScore {
                p1_rounds: 1,
                ..Default::default()
            }
        );
    }

    #[test]
    fn a_trailing_draw_does_not_take_the_game() {
        let mut set = SetMachine::new(2);
        set.observe(round(Winner::P1));
        set.observe(round(Winner::Draw));
        assert_eq!(
            set.observe(round(Winner::P1)),
            SetEvent::GameWon(Winner::P1)
        );
    }

    #[test]
    fn first_to_is_clamped_to_a_sane_range() {
        assert_eq!(SetMachine::new(0).first_to, 1);
        assert_eq!(SetMachine::new(99).first_to, 9);
    }

    #[test]
    fn seats_fill_and_lookup_by_nick() {
        let mut seats = Seats::default();
        seats.set(1, Some("player-one".into()));
        seats.set(2, Some("player-two".into()));
        assert_eq!(seats.slots[0].as_deref(), Some("player-one"));
        assert_eq!(seats.slot_of("player-two"), Some(2));
        assert_eq!(seats.slot_of("nobody"), None);
    }

    #[test]
    fn the_queue_is_fifo_and_deduplicated() {
        let mut seats = Seats::default();
        seats.enqueue("player-two".into());
        seats.enqueue("player-three".into());
        seats.enqueue("player-two".into());
        assert_eq!(seats.queue, vec!["player-two", "player-three"]);
    }

    #[test]
    fn seating_a_nick_removes_it_from_the_queue() {
        let mut seats = Seats::default();
        seats.enqueue("player-two".into());
        seats.set(2, Some("player-two".into()));
        assert!(seats.queue.is_empty());
    }

    #[test]
    fn rotation_plans_the_loser_slot_and_the_queue_head() {
        let mut seats = Seats::default();
        seats.set(1, Some("winner".into()));
        seats.set(2, Some("loser".into()));
        seats.enqueue("next-up".into());
        let rotation = plan_rotation(&seats, 1, 7).unwrap();
        assert_eq!(rotation.id, 7);
        assert_eq!(rotation.loser_slot, 2);
        assert_eq!(rotation.incoming.as_deref(), Some("next-up"));
    }

    #[test]
    fn rotation_does_nothing_without_a_waiting_spectator() {
        let mut seats = Seats::default();
        seats.set(1, Some("player-one".into()));
        seats.set(2, Some("player-two".into()));
        assert_eq!(plan_rotation(&seats, 2, 1), None);
    }

    #[test]
    fn rotation_ignores_a_draw_winner() {
        assert_eq!(winner_slot(Winner::Draw), None);
        assert_eq!(winner_slot(Winner::P1), Some(1));
        assert_eq!(winner_slot(Winner::P2), Some(2));
    }
}
