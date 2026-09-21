use super::{Capabilities, MatchRequest, Provider, ProviderKind, Role};
use crate::config::Config;
use crate::contracts::{InstallInfo, LaunchSpec};
use crate::launcher::{Launcher, MatchConfig};
use std::path::PathBuf;

#[cfg(target_os = "linux")]
use crate::launcher::linux;
#[cfg(target_os = "macos")]
use crate::launcher::macos;
#[cfg(target_os = "windows")]
use crate::launcher::windows;

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

    fn detect(&self) -> crate::error::Result<InstallInfo> {
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

    fn spec(&self, request: &MatchRequest) -> crate::error::Result<LaunchSpec> {
        let side = request.role.side().ok_or_else(|| {
            "FightCade has no spectator role — pick a spectator-capable provider".to_string()
        })?;
        let config = MatchConfig::new(request.rom.to_string(), request.peer_ip.to_string(), side)?;
        self.inner.spec(&config)
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> crate::error::Result<Box<dyn Launcher>> {
    let app_dir = cfg
        .fightcade_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(macos::DEFAULT_APP_DIR));
    let launcher = if dev {
        macos::MacosLauncher::new(app_dir).loopback()
    } else {
        macos::MacosLauncher::new(app_dir)
    };
    Ok(Box::new(launcher))
}

#[cfg(target_os = "linux")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> crate::error::Result<Box<dyn Launcher>> {
    let override_dir = cfg.fightcade_dir.clone().map(PathBuf::from);
    let rom_dir = crate::roms::resolve_rom_dir(cfg);
    let launcher = linux::LinuxLauncher::detect(override_dir, rom_dir);
    let launcher = if dev { launcher.loopback() } else { launcher };
    Ok(Box::new(launcher))
}

#[cfg(target_os = "windows")]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> crate::error::Result<Box<dyn Launcher>> {
    let override_dir = cfg.fightcade_dir.clone().map(PathBuf::from);
    let launcher = windows::WindowsLauncher::detect(override_dir);
    let launcher = if dev { launcher.loopback() } else { launcher };
    Ok(Box::new(launcher))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub(crate) fn resolve_launcher(cfg: &Config, dev: bool) -> crate::error::Result<Box<dyn Launcher>> {
    let _ = (cfg, dev);
    Err("this build only ships the macOS, Linux, and Windows launchers".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::macos::MacosLauncher;
    use std::path::Path;

    fn request<'a>(role: Role, rom_path: &'a Path, peer: &'a str) -> MatchRequest<'a> {
        MatchRequest {
            role,
            player_slot: None,
            rom: "sfiii3nr1",
            rom_path,
            peer_ip: peer,
            start_as_spectator: false,
        }
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
