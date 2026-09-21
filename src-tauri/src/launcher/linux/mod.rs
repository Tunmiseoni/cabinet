use super::{Launcher, MatchConfig};
use crate::contracts::{InstallInfo, LaunchSpec, PeerOverride};
use std::path::{Path, PathBuf};

mod detect;
mod layouts;

use detect::{
    as_flatpak_data_dir, flatpak_data_dir, looks_like_flatpak_data_dir, native_candidates,
};
use layouts::{flatpak_spec, native_spec};

pub const FLATPAK_APP: &str = "com.fightcade.Fightcade";
const FLATPAK_EMULATOR_DIR: &str = "/app/fightcade/Fightcade/emulator/fbneo";
const FLATPAK_WINE_ENTRY: &str = "/app/fightcade/Resources/wine.sh";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runner {
    Direct,
    Wine(String),
}

#[derive(Debug, Clone)]
enum Layout {
    Flatpak { data_dir: PathBuf },
    Native { fb_dir: PathBuf, runner: Runner },
    Unavailable { detail: String },
}

pub struct LinuxLauncher {
    layout: Layout,
    peer_override: PeerOverride,
}

impl LinuxLauncher {
    pub fn flatpak(data_dir: PathBuf) -> Self {
        Self {
            layout: Layout::Flatpak { data_dir },
            peer_override: PeerOverride::default(),
        }
    }

    pub fn native(fb_dir: PathBuf, runner: Runner) -> Self {
        Self {
            layout: Layout::Native { fb_dir, runner },
            peer_override: PeerOverride::default(),
        }
    }

    pub fn detect(override_dir: Option<PathBuf>, rom_dir: Option<PathBuf>) -> Self {
        if let Some(dir) = override_dir {
            if let Some(launcher) = Self::from_any_dir(&dir) {
                return launcher;
            }
        }

        if let Some(data_dir) = flatpak_data_dir() {
            return Self::flatpak(data_dir);
        }

        for candidate in native_candidates() {
            if let Some(launcher) = Self::from_native_dir(&candidate) {
                return launcher;
            }
        }

        if let Some(launcher) = rom_dir.as_deref().and_then(Self::from_rom_dir) {
            return launcher;
        }

        Self {
            layout: Layout::Unavailable {
                detail: "no FightCade Flatpak or native install found (checked Flatpak, native \
                         candidates, and your ROM directory) — set one in settings"
                    .to_string(),
            },
            peer_override: PeerOverride::default(),
        }
    }

    fn from_any_dir(dir: &Path) -> Option<Self> {
        if let Some(launcher) = Self::from_native_dir(dir) {
            return Some(launcher);
        }
        as_flatpak_data_dir(dir).map(Self::flatpak)
    }

    fn from_rom_dir(rom_dir: &Path) -> Option<Self> {
        if rom_dir.ends_with("ROMs") {
            if let Some(fb_dir) = rom_dir.parent().filter(|dir| dir.ends_with("fbneo")) {
                if let Some(launcher) = Self::from_native_dir(fb_dir) {
                    return Some(launcher);
                }
            }
        }

        let mut current = Some(rom_dir);
        while let Some(path) = current {
            if looks_like_flatpak_data_dir(path) {
                return Some(Self::flatpak(path.to_path_buf()));
            }
            current = path.parent();
        }
        None
    }

    pub fn loopback(mut self) -> Self {
        self.peer_override = PeerOverride::loopback();
        self
    }

    fn from_native_dir(dir: &Path) -> Option<Self> {
        let fb_dir = if dir.ends_with("fbneo") {
            dir.to_path_buf()
        } else {
            dir.join("emulator/fbneo")
        };
        if fb_dir.join("fcadefbneo").is_file() {
            return Some(Self::native(fb_dir, Runner::Direct));
        }
        if fb_dir.join("fcadefbneo.exe").is_file() {
            let wine = detect::find_on_path("wine", "wine64")?;
            return Some(Self::native(fb_dir, Runner::Wine(wine)));
        }
        None
    }
}

impl Launcher for LinuxLauncher {
    fn id(&self) -> &'static str {
        "linux-fightcade"
    }

    fn label(&self) -> &'static str {
        "Linux (FightCade)"
    }

    fn detect(&self) -> crate::error::Result<InstallInfo> {
        let installed = !matches!(self.layout, Layout::Unavailable { .. });
        let detail = match &self.layout {
            Layout::Flatpak { data_dir } => {
                format!("Flatpak {FLATPAK_APP} · {}", data_dir.to_string_lossy())
            }
            Layout::Native { fb_dir, runner } => match runner {
                Runner::Direct => format!("native {}", fb_dir.display()),
                Runner::Wine(wine) => format!("{wine} · {}", fb_dir.display()),
            },
            Layout::Unavailable { detail } => detail.clone(),
        };
        Ok(self.info(installed, detail))
    }

    fn emulator_dir(&self) -> PathBuf {
        match &self.layout {
            Layout::Flatpak { data_dir } => data_dir.join("ROMs/fbneo"),
            Layout::Native { fb_dir, .. } => fb_dir.clone(),
            Layout::Unavailable { .. } => PathBuf::new(),
        }
    }

    fn spec(&self, config: &MatchConfig) -> crate::error::Result<LaunchSpec> {
        let quark = config.quark_arg_overriding(self.peer_override.as_deref());
        match &self.layout {
            Layout::Flatpak { data_dir } => Ok(flatpak_spec(data_dir, &quark)),
            Layout::Native { fb_dir, runner } => Ok(native_spec(fb_dir, runner, &quark)),
            Layout::Unavailable { detail } => {
                Err(format!("FightCade is not available — {detail}").into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MatchConfig {
        MatchConfig::new("sfiii3nr1".into(), "100.64.0.2".into(), 0).unwrap()
    }

    #[test]
    fn flatpak_spec_wraps_the_emulator_in_the_sandbox() {
        let launcher = LinuxLauncher::flatpak(PathBuf::from("/home/u/.var/app/data"));
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(spec.program, PathBuf::from("flatpak"));
        assert_eq!(spec.args[0], "run");
        assert!(spec.args.contains(&FLATPAK_APP.to_string()));
        let inner = spec.args.last().unwrap();
        assert!(inner.contains(FLATPAK_EMULATOR_DIR));
        assert!(inner.contains(FLATPAK_WINE_ENTRY));
        assert!(inner.contains("quark:direct,sfiii3nr1,7001,100.64.0.2,7000,0,0"));
        assert_eq!(spec.cwd, PathBuf::from("/home/u/.var/app/data"));
        assert_eq!(
            launcher.emulator_dir(),
            PathBuf::from("/home/u/.var/app/data/ROMs/fbneo")
        );
    }

    #[test]
    fn native_direct_spec_runs_the_binary() {
        let launcher =
            LinuxLauncher::native(PathBuf::from("/opt/fc/emulator/fbneo"), Runner::Direct);
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(
            spec.program,
            PathBuf::from("/opt/fc/emulator/fbneo/fcadefbneo")
        );
        assert_eq!(
            spec.args,
            vec!["quark:direct,sfiii3nr1,7001,100.64.0.2,7000,0,0", "-w"]
        );
        assert_eq!(spec.cwd, PathBuf::from("/opt/fc/emulator/fbneo"));
    }

    #[test]
    fn native_wine_spec_wraps_the_exe() {
        let launcher = LinuxLauncher::native(
            PathBuf::from("/opt/fc/emulator/fbneo"),
            Runner::Wine("wine".to_string()),
        );
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(spec.program, PathBuf::from("wine"));
        assert_eq!(spec.args[0], "fcadefbneo.exe");
        assert_eq!(
            spec.args[1],
            "quark:direct,sfiii3nr1,7001,100.64.0.2,7000,0,0"
        );
        assert!(spec
            .envs
            .contains(&("WINEDEBUG".to_string(), "-all".to_string())));
    }

    #[test]
    fn loopback_rewrites_the_peer_ip() {
        let launcher =
            LinuxLauncher::native(PathBuf::from("/opt/fc/emulator/fbneo"), Runner::Direct)
                .loopback();
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(
            spec.args[0],
            "quark:direct,sfiii3nr1,7001,127.0.0.1,7000,0,0"
        );
    }

    #[test]
    fn unavailable_reports_not_installed_and_errors_on_spec() {
        let launcher = LinuxLauncher {
            layout: Layout::Unavailable {
                detail: "nope".to_string(),
            },
            peer_override: PeerOverride::default(),
        };
        assert!(!launcher.detect().unwrap().installed);
        assert!(launcher.spec(&config()).is_err());
    }

    #[test]
    fn detects_native_layout_from_a_directory() {
        let dir = std::env::temp_dir().join(format!("cabinet-linux-{}", std::process::id()));
        let fb = dir.join("emulator/fbneo");
        std::fs::create_dir_all(&fb).expect("create fbneo dir");
        std::fs::write(fb.join("fcadefbneo"), b"#!/bin/sh\n").expect("write binary");

        let launcher = LinuxLauncher::from_native_dir(&dir).expect("native layout");
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(spec.cwd, fb);

        std::fs::remove_dir_all(&dir).ok();
    }
}
