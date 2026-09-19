use super::{InstallInfo, LaunchSpec, Launcher, MatchConfig};
use std::path::PathBuf;

pub const DEFAULT_APP_DIR: &str = "/Applications/FightCade2.app";

pub struct MacosLauncher {
    app_dir: PathBuf,
    peer_override: Option<String>,
}

impl MacosLauncher {
    pub fn new(app_dir: PathBuf) -> Self {
        Self {
            app_dir,
            peer_override: None,
        }
    }

    pub fn loopback_with(app_dir: PathBuf) -> Self {
        Self {
            app_dir,
            peer_override: Some("127.0.0.1".to_string()),
        }
    }

    fn wine(&self) -> PathBuf {
        self.app_dir
            .join("Contents/Resources/wine/bin/wine32on64")
    }

    fn prefix(&self) -> PathBuf {
        self.app_dir.join("Contents/Resources/.wine32")
    }
}

impl Launcher for MacosLauncher {
    fn id(&self) -> &'static str {
        "macos-wine"
    }

    fn label(&self) -> &'static str {
        "macOS (FightCade Wine)"
    }

    fn detect(&self) -> Result<InstallInfo, String> {
        let wine = self.wine();
        let emulator_dir = self.emulator_dir();
        let exe = emulator_dir.join("fcadefbneo.exe");
        let installed = wine.is_file() && exe.is_file();
        let detail = if installed {
            format!("{} · {}", wine.display(), emulator_dir.display())
        } else {
            format!(
                "missing {} or {}",
                wine.display(),
                exe.display()
            )
        };
        Ok(InstallInfo {
            id: self.id().to_string(),
            label: self.label().to_string(),
            installed,
            detail,
        })
    }

    fn emulator_dir(&self) -> PathBuf {
        self.app_dir.join("Contents/MacOS/emulator/fbneo")
    }

    fn spec(&self, config: &MatchConfig) -> Result<LaunchSpec, String> {
        let emulator_dir = self.emulator_dir();
        let quark = match &self.peer_override {
            Some(peer) => {
                let mut overridden = config.clone();
                overridden.peer_ip = peer.clone();
                overridden.quark_arg()
            }
            None => config.quark_arg(),
        };

        Ok(LaunchSpec {
            program: self.wine(),
            args: vec!["fcadefbneo.exe".to_string(), quark, "-w".to_string()],
            cwd: emulator_dir,
            envs: vec![
                (
                    "WINEPREFIX".to_string(),
                    self.prefix().to_string_lossy().to_string(),
                ),
                ("WINEDEBUG".to_string(), "-all".to_string()),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MatchConfig {
        MatchConfig::new("sfiii3nr1".into(), "100.64.0.2".into(), 0).unwrap()
    }

    #[test]
    fn spec_matches_launcher_script() {
        let launcher = MacosLauncher::new(PathBuf::from("/Applications/FightCade2.app"));
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(
            spec.program,
            PathBuf::from("/Applications/FightCade2.app/Contents/Resources/wine/bin/wine32on64")
        );
        assert_eq!(
            spec.cwd,
            PathBuf::from("/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo")
        );
        assert_eq!(
            spec.args,
            vec![
                "fcadefbneo.exe",
                "quark:direct,sfiii3nr1,7001,100.64.0.2,7000,0,0",
                "-w"
            ]
        );
        let expected_prefix = PathBuf::from("/Applications/FightCade2.app")
            .join("Contents/Resources/.wine32")
            .to_string_lossy()
            .to_string();
        assert!(spec
            .envs
            .contains(&("WINEPREFIX".to_string(), expected_prefix)));
    }

    #[test]
    fn loopback_rewrites_peer_ip() {
        let launcher = MacosLauncher::loopback_with(PathBuf::from(DEFAULT_APP_DIR));
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(
            spec.args[1],
            "quark:direct,sfiii3nr1,7001,127.0.0.1,7000,0,0"
        );
    }

    #[test]
    #[ignore = "requires FightCade; opens two Wine emulator windows"]
    fn live_dev_pair_smoke() {
        use std::process::Command;
        use std::time::Duration;

        let launcher = MacosLauncher::loopback_with(PathBuf::from(DEFAULT_APP_DIR));
        let install = launcher.detect().unwrap();
        if !install.installed {
            eprintln!("skipping: {}", install.detail);
            return;
        }

        let spawn = |side: u8| {
            let peer = MatchConfig::new("sfiii3nr1".into(), "127.0.0.1".into(), side).unwrap();
            let spec = launcher.spec(&peer).unwrap();
            Command::new(&spec.program)
                .args(&spec.args)
                .current_dir(&spec.cwd)
                .envs(spec.envs.iter().cloned())
                .spawn()
                .expect("spawn emulator")
        };

        let mut p1 = spawn(0);
        let mut p2 = spawn(1);
        std::thread::sleep(Duration::from_secs(6));

        let alive = |child: &mut std::process::Child| child.try_wait().unwrap().is_none();
        let both_alive = alive(&mut p1) && alive(&mut p2);

        let _ = p1.kill();
        let _ = p2.kill();
        let _ = p1.wait();
        let _ = p2.wait();

        assert!(both_alive, "both loopback instances should stay running");
    }
}
