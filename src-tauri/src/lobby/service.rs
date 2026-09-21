use super::beacon::Beacon;
use super::room::Room;
use crate::sync::MutexExt;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct Lobby {
    inner: Mutex<Option<Live>>,
}

struct Live {
    room: Arc<Mutex<Room>>,
    beacon: Beacon,
}

/// Seats advertised to joiners: the host's own held seat plus the seat(s) its netplay observer
/// has seen fill, capped at the two player slots. Without the host's seat the beacon would
/// advertise a room with a playing host as empty, and the first joiner would collide on seat 1.
pub(crate) fn advertised_players(held: Option<u8>, observed: u8) -> u8 {
    observed.saturating_add(held.is_some() as u8).min(2)
}

impl Lobby {
    pub(crate) fn start(&self, room: Room, port: u16) -> crate::error::Result<Room> {
        let mut inner = self.inner.lock_or_recover();
        if inner.is_some() {
            return Err("a room is already hosting".into());
        }
        let mut room = room;
        if room.host_seat.is_some() {
            let held = room.host_seat;
            room.set_occupancy(advertised_players(held, 0), 0);
        }
        let starting = room.clone();
        let shared = Arc::new(Mutex::new(room));
        let beacon = Beacon::start(port, Arc::clone(&shared))?;
        if let Some(bound) = beacon.local_port() {
            log::info!("room {} beacon listening on {bound}", starting.room_id);
        }
        *inner = Some(Live {
            room: shared,
            beacon,
        });
        Ok(starting)
    }

    pub(crate) fn stop(&self) {
        if let Some(live) = self.inner.lock_or_recover().take() {
            log::info!(
                "room {} beacon stopped",
                live.room.lock_or_recover().room_id
            );
            live.beacon.stop();
        }
    }

    pub(crate) fn room(&self) -> Option<Room> {
        let inner = self.inner.lock_or_recover();
        inner
            .as_ref()
            .map(|live| live.room.lock_or_recover().clone())
    }

    /// Update the room's live occupancy. The host's netplay observer calls this whenever the
    /// peer list changes so the beacon tells joiners which seats are free.
    pub(crate) fn set_occupancy(&self, players: u8, spectators: u8) -> Option<Room> {
        let inner = self.inner.lock_or_recover();
        let live = inner.as_ref()?;
        let mut room = live.room.lock_or_recover();
        let held = room.host_seat;
        room.set_occupancy(advertised_players(held, players), spectators);
        Some(room.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spectating_host_leaves_both_seats_for_joiners() {
        assert_eq!(advertised_players(None, 0), 0);
        assert_eq!(advertised_players(None, 1), 1);
        assert_eq!(advertised_players(None, 2), 2);
    }

    #[test]
    fn a_playing_host_holds_one_seat_from_the_start() {
        assert_eq!(advertised_players(Some(1), 0), 1);
        assert_eq!(advertised_players(Some(1), 1), 2);
    }

    #[test]
    fn a_playing_host_never_advertises_more_than_two_seats() {
        assert_eq!(advertised_players(Some(1), 2), 2);
        assert_eq!(advertised_players(Some(1), 5), 2);
    }
}
