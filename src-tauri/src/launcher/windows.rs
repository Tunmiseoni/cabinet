use super::{Launcher, MatchConfig};
use crate::contracts::{InstallInfo, LaunchSpec};
use std::env;
use std::path::{Path, PathBuf};

pub const EMULATOR_EXE: &str = "fcadefbneo.exe";

pub struct WindowsLauncher {
    install_dir: PathBuf,
    peer_override: Option<String>,
}

impl WindowsLauncher {
    pub fn new(install_dir: PathBuf) -> Self {
        Self {
            install_dir,
            peer_override: None,
        }
    }

    pub fn loopback(self) -> Self {
        Self {
            peer_override: Some("127.0.0.1".to_string()),
            ..self
        }
    }

    pub fn detect(override_dir: Option<PathBuf>) -> Self {
        if let Some(dir) = override_dir {
            if has_emulator(&dir) {
                return Self::new(dir);
            }
        }
        for candidate in candidate_dirs() {
            if has_emulator(&candidate) {
                return Self::new(candidate);
            }
        }
        Self::new(default_install_dir())
    }
}

impl Launcher for WindowsLauncher {
    fn id(&self) -> &'static str {
        "windows-native"
    }

    fn label(&self) -> &'static str {
        "Windows (FightCade)"
    }

    fn detect(&self) -> Result<InstallInfo, String> {
        let emulator_dir = self.emulator_dir();
        let exe = emulator_dir.join(EMULATOR_EXE);
        let installed = exe.is_file();
        let detail = if installed {
            emulator_dir.to_string_lossy().to_string()
        } else {
            format!("missing {}", exe.display())
        };
        Ok(self.info(installed, detail))
    }

    fn emulator_dir(&self) -> PathBuf {
        emulator_dir_for(&self.install_dir)
    }

    fn spec(&self, config: &MatchConfig) -> Result<LaunchSpec, String> {
        Ok(LaunchSpec {
            program: self.emulator_dir().join(EMULATOR_EXE),
            args: vec![
                config.quark_arg_overriding(self.peer_override.as_deref()),
                "-w".to_string(),
            ],
            cwd: self.emulator_dir(),
            envs: Vec::new(),
        })
    }
}

fn emulator_dir_for(install_dir: &Path) -> PathBuf {
    if install_dir.ends_with("fbneo") {
        install_dir.to_path_buf()
    } else {
        install_dir.join("emulator").join("fbneo")
    }
}

fn has_emulator(install_dir: &Path) -> bool {
    emulator_dir_for(install_dir).join(EMULATOR_EXE).is_file()
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(appdata) = env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("Fightcade"));
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        let profile = PathBuf::from(profile);
        dirs.push(profile.join("Fightcade"));
        dirs.push(profile.join("Documents").join("GGs").join("Fightcade"));
        dirs.push(profile.join("Fightcade2"));
    }
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Fightcade"));
    }
    dirs.push(PathBuf::from("C:\\Fightcade"));
    dirs
}

fn default_install_dir() -> PathBuf {
    if let Some(appdata) = env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("Fightcade");
    }
    PathBuf::from("C:\\Fightcade")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MatchConfig {
        MatchConfig::new("sfiii3nr1".into(), "100.64.0.2".into(), 0).unwrap()
    }

    fn launcher_at(dir: &str) -> WindowsLauncher {
        WindowsLauncher::new(PathBuf::from(dir))
    }

    #[test]
    fn spec_runs_the_native_exe() {
        let launcher = launcher_at("/fc");
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(
            spec.program,
            PathBuf::from("/fc/emulator/fbneo/fcadefbneo.exe")
        );
        assert_eq!(
            spec.args,
            vec!["quark:direct,sfiii3nr1,7001,100.64.0.2,7000,0,0", "-w"]
        );
        assert_eq!(spec.cwd, PathBuf::from("/fc/emulator/fbneo"));
        assert!(spec.envs.is_empty());
    }

    #[test]
    fn emulator_dir_accepts_a_direct_fbneo_path() {
        let launcher = launcher_at("/fc/emulator/fbneo");
        assert_eq!(launcher.emulator_dir(), PathBuf::from("/fc/emulator/fbneo"));
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(spec.cwd, PathBuf::from("/fc/emulator/fbneo"));
    }

    #[test]
    fn loopback_rewrites_the_peer_ip() {
        let launcher = launcher_at("/fc").loopback();
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(
            spec.args[0],
            "quark:direct,sfiii3nr1,7001,127.0.0.1,7000,0,0"
        );
    }

    #[test]
    fn detects_an_install_from_a_directory() {
        let dir = std::env::temp_dir().join(format!("cabinet-windows-{}", std::process::id()));
        let fb = dir.join("emulator/fbneo");
        std::fs::create_dir_all(&fb).expect("create fbneo dir");
        std::fs::write(fb.join(EMULATOR_EXE), b"MZ").expect("write exe");

        let launcher = WindowsLauncher::new(dir.clone());
        let info = launcher.detect().unwrap();
        assert!(info.installed);
        assert_eq!(info.id, "windows-native");
        assert_eq!(launcher.emulator_dir(), fb);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_install_reports_not_installed() {
        let launcher = launcher_at("/nope");
        assert!(!launcher.detect().unwrap().installed);
    }

    #[test]
    fn detect_honors_an_override_directory() {
        let dir = std::env::temp_dir().join(format!("cabinet-win-ovr-{}", std::process::id()));
        let fb = dir.join("emulator/fbneo");
        std::fs::create_dir_all(&fb).expect("create fbneo dir");
        std::fs::write(fb.join(EMULATOR_EXE), b"MZ").expect("write exe");

        let launcher = WindowsLauncher::detect(Some(dir.clone()));
        assert!(launcher.detect().unwrap().installed);

        std::fs::remove_dir_all(&dir).ok();
    }
}
