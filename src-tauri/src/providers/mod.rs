mod fightcade;
mod retroarch;

pub(crate) use retroarch::command;
pub(crate) use retroarch::InputMap;
pub use retroarch::ParityStatus;
pub(crate) use retroarch::RetroArchProvider;
pub(crate) use retroarch::{download_managed_core, frozen_core_sha256};

use crate::config::Config;
use crate::contracts::{InstallInfo, LaunchSpec};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    #[default]
    Fightcade,
    Retroarch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    P1,
    P2,
    Spectator,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::P1 => "P1",
            Role::P2 => "P2",
            Role::Spectator => "Spectator",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Role::P1 => "p1",
            Role::P2 => "p2",
            Role::Spectator => "spectator",
        }
    }

    pub fn side(self) -> Option<u8> {
        self.default_player_slot().map(|slot| slot - 1)
    }

    /// The seat this role takes by default: a player slot (`1`/`2`), or `None` when the
    /// instance sends no input. The lobby can override it with an explicit seat for the host's
    /// "play as" choice and for room bookkeeping.
    pub fn default_player_slot(self) -> Option<u8> {
        match self {
            Role::P1 => Some(1),
            Role::P2 => Some(2),
            Role::Spectator => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub spectate: bool,
    pub dev_pair: bool,
}

#[derive(Debug, Clone)]
pub struct MatchRequest<'a> {
    pub role: Role,
    /// The player seat this instance intends to occupy: `Some(1)`/`Some(2)`, or `None` for the
    /// role's default (`P1` -> 1, `P2` -> 2, `Spectator` -> none). The seat does **not** choose
    /// the input bind prefix — RetroArch netplay reads the local keyboard from the first local
    /// device, i.e. `input_player1_*` — it selects which device the host requests and is recorded
    /// for the room. Separates *which player you control* from *how you connect*.
    pub player_slot: Option<u8>,
    pub rom: &'a str,
    pub rom_path: &'a Path,
    pub peer_ip: &'a str,
    pub start_as_spectator: bool,
}

impl MatchRequest<'_> {
    /// The resolved seat: the explicit `player_slot` if valid, else the role's default.
    pub fn seat(&self) -> crate::error::Result<Option<u8>> {
        match self.player_slot {
            Some(slot @ (1 | 2)) => Ok(Some(slot)),
            Some(other) => Err(format!("invalid player slot {other}: expected 1 or 2").into()),
            None => Ok(self.role.default_player_slot()),
        }
    }
}

pub trait Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;
    fn detect(&self) -> crate::error::Result<InstallInfo>;
    fn capabilities(&self) -> Capabilities;
    fn port(&self, role: Role) -> Option<u16>;
    fn spec(&self, request: &MatchRequest) -> crate::error::Result<LaunchSpec>;

    fn command_port(&self, _role: Role) -> Option<u16> {
        None
    }

    fn requires_rom_file(&self) -> bool {
        false
    }

    fn parity(&self, _rom_path: &Path) -> crate::error::Result<Option<ParityStatus>> {
        Ok(None)
    }

    fn input_map(&self) -> Option<InputMap> {
        None
    }
}

pub(crate) fn resolve_provider(
    app: &AppHandle,
    cfg: &Config,
    dev: bool,
) -> crate::error::Result<Box<dyn Provider>> {
    match cfg.provider {
        ProviderKind::Fightcade => Ok(Box::new(fightcade::FightCadeProvider::new(
            fightcade::resolve_launcher(cfg, dev)?,
        ))),
        ProviderKind::Retroarch => {
            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|err| format!("cannot resolve config dir: {err}"))?;
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|err| format!("cannot resolve data dir: {err}"))?;
            Ok(Box::new(RetroArchProvider::new(
                cfg,
                &config_dir,
                &data_dir,
                dev,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_side_mapping() {
        assert_eq!(Role::P1.side(), Some(0));
        assert_eq!(Role::P2.side(), Some(1));
        assert_eq!(Role::Spectator.side(), None);
        assert_eq!(Role::Spectator.label(), "Spectator");
    }

    #[test]
    fn default_player_slot_matches_the_role() {
        assert_eq!(Role::P1.default_player_slot(), Some(1));
        assert_eq!(Role::P2.default_player_slot(), Some(2));
        assert_eq!(Role::Spectator.default_player_slot(), None);
    }

    #[test]
    fn an_explicit_seat_overrides_the_role_default() {
        let rom = std::path::Path::new("/tmp/sfiii3nr1.zip");
        let mut request = MatchRequest {
            role: Role::P2,
            player_slot: Some(1),
            rom: "sfiii3nr1",
            rom_path: rom,
            peer_ip: "100.64.0.2",
            start_as_spectator: false,
        };
        assert_eq!(request.seat().unwrap(), Some(1));

        request.player_slot = None;
        assert_eq!(request.seat().unwrap(), Some(2));

        request.player_slot = Some(0);
        assert!(request.seat().is_err());
        request.player_slot = Some(3);
        assert!(request.seat().is_err());
    }
}
