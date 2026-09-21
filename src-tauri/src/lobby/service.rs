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

impl Lobby {
    pub(crate) fn start(&self, room: Room, port: u16) -> crate::error::Result<Room> {
        let mut inner = self.inner.lock_or_recover();
        if inner.is_some() {
            return Err("a room is already hosting".into());
        }
        let shared = Arc::new(Mutex::new(room.clone()));
        let beacon = Beacon::start(port, Arc::clone(&shared))?;
        if let Some(bound) = beacon.local_port() {
            log::info!("room {} beacon listening on {bound}", room.room_id);
        }
        *inner = Some(Live {
            room: shared,
            beacon,
        });
        Ok(room)
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
        room.set_occupancy(players, spectators);
        Some(room.clone())
    }
}
