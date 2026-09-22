use crate::providers::command;

const KO_HEALTH: u8 = 0xFF;

/// The game's own round state (community `game_phase` `0x020154A7`, 3 bytes lower in our mapping —
/// see docs/10-lobby-spike.md §6 L7): `0x01` is character select / round intro, `0x02` is a live
/// round, `0x06`–`0x09` is the round-end/continue sequence. This is the live-match signal L12
/// asked for.
pub(crate) const PHASE_INTRO: u8 = 0x01;
pub(crate) const PHASE_LIVE: u8 = 0x02;

/// Player-struct control type (struct offset `+0x03`; P2 lives at `0x069104`): the game's own
/// "this fighter is human-controlled" flag. Live 2026-09-22: `0x01` for P1 all session and for P2
/// through the 2P match, `0x00` for P2 in the arcade/CPU rounds and for both players in the
/// attract demo — see docs/10-lobby-spike.md §6 L15.
pub(crate) const CONTROL_HUMAN: u8 = 0x01;

pub(crate) fn phase_is_round_end(phase: u8) -> bool {
    (0x06..=0x09).contains(&phase)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RomAddresses {
    pub p1_health: u32,
    pub p2_health: u32,
    pub rounds: u32,
    pub phase: u32,
    pub p2_control: u32,
}

pub(crate) const SFIII3NR1: RomAddresses = RomAddresses {
    p1_health: 0x068D08,
    p2_health: 0x0691A0,
    rounds: 0x010D28,
    phase: 0x0154A4,
    p2_control: 0x069104,
};

pub(crate) fn addresses_for(rom: &str) -> Option<RomAddresses> {
    match rom {
        "sfiii3nr1" => Some(SFIII3NR1),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Winner {
    P1,
    P2,
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RoundOutcome {
    pub winner: Winner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HealthSnapshot {
    pub p1_health: u8,
    pub p2_health: u8,
    pub rounds: u8,
    pub phase: u8,
    pub p2_control: u8,
}

impl HealthSnapshot {
    pub(crate) fn p1_ko(&self) -> bool {
        self.p1_health == KO_HEALTH
    }

    pub(crate) fn p2_ko(&self) -> bool {
        self.p2_health == KO_HEALTH
    }
}

pub(crate) fn read_snapshot(
    port: u16,
    addresses: RomAddresses,
) -> crate::error::Result<HealthSnapshot> {
    Ok(HealthSnapshot {
        p1_health: read_byte(port, addresses.p1_health)?,
        p2_health: read_byte(port, addresses.p2_health)?,
        rounds: read_byte(port, addresses.rounds)?,
        phase: read_byte(port, addresses.phase)?,
        p2_control: read_byte(port, addresses.p2_control)?,
    })
}

fn read_byte(port: u16, address: u32) -> crate::error::Result<u8> {
    let read = command::read_core_ram_bytes(port, address, 1)?;
    read.bytes
        .first()
        .copied()
        .ok_or_else(|| format!("READ_CORE_RAM {address:#x} returned no bytes").into())
}

#[derive(Debug, Default)]
pub(crate) struct RoundWatcher {
    last: Option<HealthSnapshot>,
    /// Set once the game reports a live round (`phase == 0x02`) **and** P2 is human-controlled.
    /// Every KO edge we sampled landed after the game had already moved into its round-end sequence
    /// (`0x06`), so the latch must survive `0x06`–`0x09` and is only cleared by the next
    /// select/intro — an edge that was never live is not a match result, and neither is a round the
    /// game hands to the CPU (arcade drift, attract demo). See docs/10-lobby-spike.md §6 L12/L15.
    armed: bool,
    /// Set once the round has been decided, cleared when a fresh (healthy) live round starts.
    /// Guards against deciding the same round twice: a KO byte that only shows up a poll after the
    /// round-end transition, or a KO seen while the phase was still live followed by the
    /// transition sample.
    decided: bool,
}

impl RoundWatcher {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn observe(&mut self, snapshot: HealthSnapshot) -> Option<RoundOutcome> {
        if snapshot.phase == PHASE_LIVE {
            self.armed = snapshot.p2_control == CONTROL_HUMAN;
            if !snapshot.p1_ko() && !snapshot.p2_ko() {
                self.decided = false;
            }
        } else if !phase_is_round_end(snapshot.phase) {
            self.armed = false;
            self.decided = false;
        }

        let previous = self.last.replace(snapshot)?;
        if !self.armed || self.decided {
            return None;
        }

        // A KO decides the round the moment health saturates. Emit on the non-KO -> KO edge:
        // waiting for the next round would stall behind the loser's coin-in, and the round counter
        // cannot be the trigger (it does not move during versus matches; see
        // docs/10-lobby-spike.md §6 L18).
        let p1_ko = snapshot.p1_ko() && !previous.p1_ko();
        let p2_ko = snapshot.p2_ko() && !previous.p2_ko();
        if p1_ko || p2_ko {
            self.decided = true;
            let winner = match (p1_ko, p2_ko) {
                (true, true) => Winner::Draw,
                (true, false) => Winner::P2,
                (false, true) => Winner::P1,
                (false, false) => unreachable!("a KO edge was just checked"),
            };
            return Some(RoundOutcome { winner });
        }

        // A timeout leaves both fighters alive: a live round entering the round-end sequence
        // without a KO has run out of time, and the health at the transition decides it.
        if previous.phase == PHASE_LIVE && phase_is_round_end(snapshot.phase) {
            self.decided = true;
            return Some(RoundOutcome {
                winner: timeout_winner(&snapshot),
            });
        }

        None
    }
}

fn timeout_winner(snapshot: &HealthSnapshot) -> Winner {
    use std::cmp::Ordering;
    match snapshot.p1_health.cmp(&snapshot.p2_health) {
        Ordering::Greater => Winner::P1,
        Ordering::Less => Winner::P2,
        Ordering::Equal => Winner::Draw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, UdpSocket};

    fn snap(p1: u8, p2: u8, rounds: u8) -> HealthSnapshot {
        snap_at(PHASE_LIVE, p1, p2, rounds)
    }

    fn snap_at(phase: u8, p1: u8, p2: u8, rounds: u8) -> HealthSnapshot {
        HealthSnapshot {
            p1_health: p1,
            p2_health: p2,
            rounds,
            phase,
            p2_control: CONTROL_HUMAN,
        }
    }

    /// The same snapshot with P2 driven by the CPU (arcade drift, attract demo).
    fn snap_cpu(phase: u8, p1: u8, p2: u8, rounds: u8) -> HealthSnapshot {
        HealthSnapshot {
            p2_control: 0x00,
            ..snap_at(phase, p1, p2, rounds)
        }
    }

    #[test]
    fn knows_the_sfiii3nr1_addresses_and_nothing_else() {
        assert_eq!(addresses_for("sfiii3nr1"), Some(SFIII3NR1));
        assert_eq!(addresses_for("sf2ce"), None);
    }

    #[test]
    fn does_not_emit_on_the_first_observation() {
        let mut watcher = RoundWatcher::new();
        assert_eq!(watcher.observe(snap(0xA0, 0xA0, 0)), None);
    }

    #[test]
    fn p1_ko_hands_the_round_to_p2() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        let outcome = watcher.observe(snap(0xFF, 0x40, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P2);
        // The next round starting, with the counter catching up, must not double-count.
        assert_eq!(watcher.observe(snap(0xA0, 0xA0, 1)), None);
    }

    #[test]
    fn p2_ko_hands_the_round_to_p1() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        let outcome = watcher.observe(snap(0x30, 0xFF, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
        assert_eq!(watcher.observe(snap(0xA0, 0xA0, 1)), None);
    }

    #[test]
    fn a_double_ko_is_a_draw() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        let outcome = watcher.observe(snap(0xFF, 0xFF, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::Draw);
        assert_eq!(watcher.observe(snap(0xA0, 0xA0, 1)), None);
    }

    #[test]
    fn a_ko_counts_even_when_the_round_counter_stays_at_zero() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        let first = watcher.observe(snap(0xA0, 0xFF, 0)).unwrap();
        assert_eq!(first.winner, Winner::P1);
        // The continue screen holds the KO; it must not be counted again.
        assert_eq!(watcher.observe(snap(0xA0, 0xFF, 0)), None);
        // The next round starts with the counter still at 0 — no re-emit, but the watcher re-arms.
        assert_eq!(watcher.observe(snap(0xA0, 0xA0, 0)), None);
        let second = watcher.observe(snap(0xFF, 0xA0, 0)).unwrap();
        assert_eq!(second.winner, Winner::P2);
    }

    #[test]
    fn a_timeout_goes_to_the_higher_health() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_LIVE, 0x50, 0x20, 0));
        let outcome = watcher.observe(snap_at(0x06, 0x50, 0x20, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
    }

    #[test]
    fn an_equal_timeout_is_a_draw() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_LIVE, 0x30, 0x30, 0));
        let outcome = watcher.observe(snap_at(0x06, 0x30, 0x30, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::Draw);
    }

    #[test]
    fn a_new_match_resets_without_emitting() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 2));
        assert_eq!(watcher.observe(snap(0xA0, 0xA0, 0)), None);
        let outcome = watcher.observe(snap(0xFF, 0x20, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P2);
    }

    #[test]
    fn a_ko_seen_only_after_the_reset_still_counts() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        let outcome = watcher.observe(snap(0x20, 0xFF, 1)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
    }

    #[test]
    fn read_snapshot_collects_the_health_rounds_phase_and_control_bytes() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        let responder = std::thread::spawn(move || {
            let mut buffer = [0u8; 128];
            for _ in 0..5 {
                let (len, from) = server.recv_from(&mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..len]).into_owned();
                let address = request
                    .split_whitespace()
                    .nth(1)
                    .expect("address token")
                    .to_string();
                let value = match u32::from_str_radix(&address, 16).unwrap() {
                    0x068D08 => 0xA0u8,
                    0x0691A0 => 0xFF,
                    0x010D28 => 0x02,
                    0x0154A4 => 0x02,
                    0x069104 => 0x01,
                    other => panic!("unexpected address {other:#x}"),
                };
                server
                    .send_to(
                        format!("READ_CORE_RAM {address} {value:02X}\n").as_bytes(),
                        from,
                    )
                    .unwrap();
            }
        });

        let snapshot = read_snapshot(port, SFIII3NR1).unwrap();
        assert_eq!(snapshot, snap(0xA0, 0xFF, 0x02));
        responder.join().unwrap();
    }

    #[test]
    fn a_ko_that_was_never_live_is_not_a_result() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_INTRO, 0xA0, 0xA0, 0));
        assert_eq!(watcher.observe(snap_at(PHASE_INTRO, 0xFF, 0xA0, 0)), None);
    }

    #[test]
    fn a_ko_first_seen_in_the_round_end_sequence_still_counts() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_LIVE, 0xA0, 0xA0, 0));
        // The poll lands after the game already moved into its round-end phase (every live KO edge
        // we sampled looked like this).
        let outcome = watcher.observe(snap_at(0x06, 0xA0, 0xFF, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
    }

    #[test]
    fn the_next_select_clears_the_latch() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_LIVE, 0xA0, 0xA0, 0));
        watcher.observe(snap_at(0x06, 0xFF, 0xA0, 0));
        watcher.observe(snap_at(PHASE_INTRO, 0xA0, 0xA0, 0));
        assert_eq!(watcher.observe(snap_at(PHASE_INTRO, 0xA0, 0xFF, 0)), None);
    }

    #[test]
    fn a_new_match_rearms_once_it_goes_live() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_INTRO, 0xA0, 0xA0, 0));
        watcher.observe(snap_at(PHASE_LIVE, 0xA0, 0xA0, 0));
        let outcome = watcher.observe(snap_at(0x06, 0xFF, 0x30, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P2);
    }

    #[test]
    fn a_round_end_without_a_live_round_is_not_a_result() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_INTRO, 0xA0, 0x60, 0));
        assert_eq!(watcher.observe(snap_at(0x06, 0xA0, 0x60, 0)), None);
    }

    #[test]
    fn the_recorded_versus_timeout_counts() {
        // Replays the exact samples of the 2026-09-22 validation run (18:52:20-18:52:39): a versus
        // round that ran out of time with the round counter still at 0 and P2 still human.
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_LIVE, 0xA0, 0x77, 0));
        let outcome = watcher.observe(snap_at(0x06, 0xA0, 0x77, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
        // The round-end sequence holds the same health; it must not be counted again.
        assert_eq!(watcher.observe(snap_at(0x07, 0xA0, 0x77, 0)), None);
        assert_eq!(watcher.observe(snap_at(0x09, 0xA0, 0x77, 0)), None);
    }

    #[test]
    fn a_lagging_ko_byte_after_a_timeout_decision_is_not_counted_twice() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_LIVE, 0xA0, 0x77, 0));
        let outcome = watcher.observe(snap_at(0x06, 0xA0, 0x77, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
        // A KO byte that only shows up a poll later must not decide the round again.
        assert_eq!(watcher.observe(snap_at(0x06, 0xA0, 0xFF, 0)), None);
    }

    #[test]
    fn a_ko_seen_while_still_live_does_not_double_count_at_the_transition() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_at(PHASE_LIVE, 0xA0, 0xA0, 0));
        let outcome = watcher.observe(snap_at(PHASE_LIVE, 0xA0, 0xFF, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
        assert_eq!(watcher.observe(snap_at(0x06, 0xA0, 0xFF, 0)), None);
    }

    #[test]
    fn a_cpu_driven_p2_is_not_a_result() {
        // Covers both the arcade drift after a 2P match and the attract demo (both fighters CPU).
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_cpu(PHASE_LIVE, 0xA0, 0xA0, 0));
        assert_eq!(watcher.observe(snap_cpu(0x06, 0xA0, 0xFF, 0)), None);
    }

    #[test]
    fn a_cpu_timeout_is_not_a_result() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap_cpu(PHASE_LIVE, 0x50, 0x20, 0));
        assert_eq!(watcher.observe(snap_cpu(0x06, 0x50, 0x20, 0)), None);
    }

    #[test]
    fn the_arcade_round_after_a_2p_match_is_not_a_result() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        let outcome = watcher.observe(snap_at(0x06, 0xA0, 0xFF, 0)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
        // The match ends, the game hands P2 to the CPU, and the next arcade round must not count.
        watcher.observe(snap_cpu(PHASE_INTRO, 0xA0, 0xA0, 0));
        watcher.observe(snap_cpu(PHASE_LIVE, 0xA0, 0xA0, 0));
        assert_eq!(watcher.observe(snap_cpu(0x06, 0xA0, 0xFF, 0)), None);
    }
}
