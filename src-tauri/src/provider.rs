use crate::config::Config;
use crate::launcher::{InstallInfo, LaunchSpec, Launcher, MatchConfig};
use crate::retroarch::{ParityStatus, RetroArchProvider};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "linux")]
use crate::launcher::linux;
#[cfg(target_os = "macos")]
use crate::launcher::macos;
#[cfg(target_os = "windows")]
use crate::launcher::windows;

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
    fn detect(&self) -> Result<InstallInfo, String>;
    fn capabilities(&self) -> Capabilities;
    fn port(&self, role: Role) -> Option<u16>;
    fn spec(&self, request: &MatchRequest) -> Result<LaunchSpec, String>;

    fn parity(&self, _rom_path: &Path) -> Result<Option<ParityStatus>, String> {
        Ok(None)
    }
}

pub struct FightCadeProvider {
    inner: Box<dyn Launcher>,
}

impl FightCadeProvider {
    pub fn new(inner: Box<dyn Launcher>) -> Self {
        Self { inner }
    }
}

impl Provider for FightCadeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Fightcade
    }

    fn detect(&self) -> Result<InstallInfo, String> {
        self.inner.detect()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            spectate: false,
            dev_pair: true,
        }
    }

    fn port(&self, role: Role) -> Option<u16> {
        role.side().map(MatchConfig::local_port_for_side)
    }

    fn spec(&self, request: &MatchRequest) -> Result<LaunchSpec, String> {
        let side = request.role.side().ok_or_else(|| {
            "FightCade has no spectator role — pick a spectator-capable provider".to_string()
        })?;
        let config = MatchConfig::new(request.rom.to_string(), request.peer_ip.to_string(), side)?;
        self.inner.spec(&config)
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    let app_dir = cfg
        .fightcade_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(macos::DEFAULT_APP_DIR));
    let launcher = if dev {
        macos::MacosLauncher::loopback_with(app_dir)
    } else {
        macos::MacosLauncher::new(app_dir)
    };
    Ok(Box::new(launcher))
}

#[cfg(target_os = "linux")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    let override_dir = cfg.fightcade_dir.clone().map(PathBuf::from);
    let rom_dir = crate::roms::resolve_rom_dir(cfg);
    let launcher = linux::LinuxLauncher::detect(override_dir, rom_dir);
    let launcher = if dev { launcher.loopback() } else { launcher };
    Ok(Box::new(launcher))
}

#[cfg(target_os = "windows")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    let override_dir = cfg.fightcade_dir.clone().map(PathBuf::from);
    let launcher = windows::WindowsLauncher::detect(override_dir);
    let launcher = if dev { launcher.loopback() } else { launcher };
    Ok(Box::new(launcher))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> Result<Box<dyn Launcher>, String> {
    let _ = (cfg, dev);
    Err("this build only ships the macOS, Linux, and Windows launchers".into())
}

pub(crate) fn resolve_provider(
    app: &AppHandle,
    cfg: &Config,
    dev: bool,
) -> Result<Box<dyn Provider>, String> {
    match cfg.provider {
        ProviderKind::Fightcade => Ok(Box::new(FightCadeProvider::new(resolve_launcher(
            cfg, dev,
        )?))),
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
    use crate::launcher::macos::MacosLauncher;

    fn request<'a>(role: Role, rom_path: &'a Path, peer: &'a str) -> MatchRequest<'a> {
        MatchRequest {
            role,
            rom: "sfiii3nr1",
            rom_path,
            peer_ip: peer,
        }
    }

    #[test]
    fn role_side_mapping() {
        assert_eq!(Role::P1.side(), Some(0));
        assert_eq!(Role::P2.side(), Some(1));
        assert_eq!(Role::Spectator.side(), None);
        assert_eq!(Role::Spectator.label(), "Spectator");
    }

    #[test]
    fn fightcade_rejects_spectators_but_maps_players() {
        let provider = FightCadeProvider::new(Box::new(MacosLauncher::new(PathBuf::from(
            "/Applications/FightCade2.app",
        ))));
        let path = PathBuf::from("/tmp/sfiii3nr1.zip");

        let p1 = provider
            .spec(&request(Role::P1, &path, "100.64.0.2"))
            .unwrap();
        assert_eq!(
            p1.args[1],
            "quark:direct,sfiii3nr1,7001,100.64.0.2,7000,0,0"
        );
        assert!(provider
            .spec(&request(Role::Spectator, &path, "100.64.0.2"))
            .is_err());
    }

    #[test]
    fn fightcade_capabilities_and_ports() {
        let provider = FightCadeProvider::new(Box::new(MacosLauncher::new(PathBuf::from(
            "/Applications/FightCade2.app",
        ))));
        let caps = provider.capabilities();
        assert!(!caps.spectate);
        assert!(caps.dev_pair);
        assert_eq!(provider.port(Role::P1), Some(7001));
        assert_eq!(provider.port(Role::P2), Some(7000));
        assert_eq!(provider.port(Role::Spectator), None);
    }

    #[test]
    fn fightcade_has_no_parity_gate() {
        let provider = FightCadeProvider::new(Box::new(MacosLauncher::new(PathBuf::from(
            "/Applications/FightCade2.app",
        ))));
        assert!(provider
            .parity(Path::new("/tmp/rom.zip"))
            .unwrap()
            .is_none());
    }
}
