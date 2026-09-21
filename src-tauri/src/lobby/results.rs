use crate::providers::command;

const KO_HEALTH: u8 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RomAddresses {
    pub p1_health: u32,
    pub p2_health: u32,
    pub rounds: u32,
}

pub(crate) const SFIII3NR1: RomAddresses = RomAddresses {
    p1_health: 0x068D08,
    p2_health: 0x0691A0,
    rounds: 0x010D28,
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
    ko: Option<Winner>,
}

impl RoundWatcher {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn observe(&mut self, snapshot: HealthSnapshot) -> Option<RoundOutcome> {
        if let Some(last) = self.last {
            if snapshot.rounds < last.rounds {
                self.ko = None;
                self.last = Some(snapshot);
                return None;
            }
        }

        match (snapshot.p1_ko(), snapshot.p2_ko()) {
            (true, true) => self.ko = Some(Winner::Draw),
            (true, false) => self.ko = Some(Winner::P2),
            (false, true) => self.ko = Some(Winner::P1),
            (false, false) => {}
        }

        let outcome = match self.last {
            Some(last) if snapshot.rounds > last.rounds => {
                let winner = self.ko.take().unwrap_or_else(|| timeout_winner(&last));
                Some(RoundOutcome { winner })
            }
            _ => None,
        };
        self.last = Some(snapshot);
        outcome
    }
}

fn timeout_winner(last: &HealthSnapshot) -> Winner {
    use std::cmp::Ordering;
    match last.p1_health.cmp(&last.p2_health) {
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
        HealthSnapshot {
            p1_health: p1,
            p2_health: p2,
            rounds,
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
        watcher.observe(snap(0xFF, 0x40, 0));
        let outcome = watcher.observe(snap(0xA0, 0xA0, 1)).unwrap();
        assert_eq!(outcome.winner, Winner::P2);
    }

    #[test]
    fn p2_ko_hands_the_round_to_p1() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        watcher.observe(snap(0x30, 0xFF, 0));
        let outcome = watcher.observe(snap(0xA0, 0xA0, 1)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
    }

    #[test]
    fn a_double_ko_is_a_draw() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        watcher.observe(snap(0xFF, 0xFF, 0));
        let outcome = watcher.observe(snap(0xA0, 0xA0, 1)).unwrap();
        assert_eq!(outcome.winner, Winner::Draw);
    }

    #[test]
    fn a_timeout_goes_to_the_higher_health() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        watcher.observe(snap(0x50, 0x20, 0));
        let outcome = watcher.observe(snap(0xA0, 0xA0, 1)).unwrap();
        assert_eq!(outcome.winner, Winner::P1);
    }

    #[test]
    fn an_equal_timeout_is_a_draw() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 0));
        watcher.observe(snap(0x30, 0x30, 0));
        let outcome = watcher.observe(snap(0xA0, 0xA0, 1)).unwrap();
        assert_eq!(outcome.winner, Winner::Draw);
    }

    #[test]
    fn a_new_match_resets_without_emitting() {
        let mut watcher = RoundWatcher::new();
        watcher.observe(snap(0xA0, 0xA0, 2));
        assert_eq!(watcher.observe(snap(0xA0, 0xA0, 0)), None);
        watcher.observe(snap(0xFF, 0x20, 0));
        let outcome = watcher.observe(snap(0xA0, 0xA0, 1)).unwrap();
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
    fn read_snapshot_collects_the_three_addresses() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        let responder = std::thread::spawn(move || {
            let mut buffer = [0u8; 128];
            for _ in 0..3 {
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
}
