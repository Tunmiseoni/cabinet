mod fightcade;
mod retroarch;

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
        match self {
            Role::P1 => Some(0),
            Role::P2 => Some(1),
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
    pub rom: &'a str,
    pub rom_path: &'a Path,
    pub peer_ip: &'a str,
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
}
