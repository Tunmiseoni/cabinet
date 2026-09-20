use super::{InstallInfo, LaunchSpec, Launcher, MatchConfig};
use crate::process;
use std::env;
use std::path::{Path, PathBuf};

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
    peer_override: Option<String>,
}

impl LinuxLauncher {
    pub fn flatpak(data_dir: PathBuf) -> Self {
        Self {
            layout: Layout::Flatpak { data_dir },
            peer_override: None,
        }
    }

    pub fn native(fb_dir: PathBuf, runner: Runner) -> Self {
        Self {
            layout: Layout::Native { fb_dir, runner },
            peer_override: None,
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
            peer_override: None,
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
        self.peer_override = Some("127.0.0.1".to_string());
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
            let wine = find_on_path("wine", "wine64")?;
            return Some(Self::native(fb_dir, Runner::Wine(wine)));
        }
        None
    }

    fn quark(&self, config: &MatchConfig) -> String {
        match &self.peer_override {
            Some(peer) => {
                let mut overridden = config.clone();
                overridden.peer_ip = peer.clone();
                overridden.quark_arg()
            }
            None => config.quark_arg(),
        }
    }
}

impl Launcher for LinuxLauncher {
    fn id(&self) -> &'static str {
        "linux-fightcade"
    }

    fn label(&self) -> &'static str {
        "Linux (FightCade)"
    }

    fn detect(&self) -> Result<InstallInfo, String> {
        let installed = !matches!(self.layout, Layout::Unavailable { .. });
        let detail = match &self.layout {
            Layout::Flatpak { data_dir } => format!(
                "Flatpak {FLATPAK_APP} · {}",
                data_dir.to_string_lossy()
            ),
            Layout::Native { fb_dir, runner } => match runner {
                Runner::Direct => format!("native {}", fb_dir.display()),
                Runner::Wine(wine) => format!("{wine} · {}", fb_dir.display()),
            },
            Layout::Unavailable { detail } => detail.clone(),
        };
        Ok(InstallInfo {
            id: self.id().to_string(),
            label: self.label().to_string(),
            installed,
            detail,
        })
    }

    fn emulator_dir(&self) -> PathBuf {
        match &self.layout {
            Layout::Flatpak { data_dir } => data_dir.join("ROMs/fbneo"),
            Layout::Native { fb_dir, .. } => fb_dir.clone(),
            Layout::Unavailable { .. } => PathBuf::new(),
        }
    }

    fn spec(&self, config: &MatchConfig) -> Result<LaunchSpec, String> {
        let quark = self.quark(config);
        match &self.layout {
            Layout::Flatpak { data_dir } => {
                let inner = format!(
                    ". /app/bin/get-wine-prefix; cd {FLATPAK_EMULATOR_DIR}; \
                     export WINEDEBUG=-all; \
                     exec {FLATPAK_WINE_ENTRY} fcadefbneo.exe '{}' -w",
                    shell_single_quote(&quark)
                );
                Ok(LaunchSpec {
                    program: PathBuf::from("flatpak"),
                    args: vec![
                        "run".to_string(),
                        "--command=/bin/sh".to_string(),
                        FLATPAK_APP.to_string(),
                        "-c".to_string(),
                        inner,
                    ],
                    cwd: data_dir.clone(),
                    envs: Vec::new(),
                })
            }
            Layout::Native { fb_dir, runner } => match runner {
                Runner::Direct => Ok(LaunchSpec {
                    program: fb_dir.join("fcadefbneo"),
                    args: vec![quark, "-w".to_string()],
                    cwd: fb_dir.clone(),
                    envs: vec![("WINEDEBUG".to_string(), "-all".to_string())],
                }),
                Runner::Wine(wine) => Ok(LaunchSpec {
                    program: PathBuf::from(wine),
                    args: vec!["fcadefbneo.exe".to_string(), quark, "-w".to_string()],
                    cwd: fb_dir.clone(),
                    envs: vec![("WINEDEBUG".to_string(), "-all".to_string())],
                }),
            },
            Layout::Unavailable { detail } => Err(format!("FightCade is not available — {detail}")),
        }
    }
}

fn shell_single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

fn flatpak_data_dir() -> Option<PathBuf> {
    if let Some(data_dir) = flatpak_data_dir_via_cli() {
        return Some(data_dir);
    }
    let home = env::var_os("HOME")?;
    let data_dir = PathBuf::from(home)
        .join(".var/app")
        .join(FLATPAK_APP)
        .join("data");
    data_dir.is_dir().then_some(data_dir)
}

fn flatpak_data_dir_via_cli() -> Option<PathBuf> {
    let flatpak = find_on_path("flatpak", "flatpak")?;
    let ok = process::command(flatpak)
        .args(["info", FLATPAK_APP])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !ok {
        return None;
    }
    let home = env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".var/app")
            .join(FLATPAK_APP)
            .join("data"),
    )
}

fn looks_like_flatpak_data_dir(dir: &Path) -> bool {
    dir.file_name().and_then(|name| name.to_str()) == Some("data")
        && dir
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some(FLATPAK_APP)
}

fn as_flatpak_data_dir(dir: &Path) -> Option<PathBuf> {
    if looks_like_flatpak_data_dir(dir) || dir.join("ROMs").is_dir() {
        return Some(dir.to_path_buf());
    }
    if dir.file_name().and_then(|name| name.to_str()) == Some(FLATPAK_APP) {
        return Some(dir.join("data"));
    }
    None
}

fn native_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(dir) = env::var("FC_DIR") {
        if !dir.is_empty() {
            candidates.push(PathBuf::from(dir));
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        for name in [".fightcade", "fightcade", "Fightcade", "fightcade2"] {
            candidates.push(home.join(name));
        }
        candidates.push(home.join(".local/share/fightcade"));
        candidates.extend(game_subdirs(&home.join("Games")));
    }
    candidates.push(PathBuf::from("/opt/fightcade"));
    candidates.push(PathBuf::from("/opt/FightCade2"));
    candidates
}

fn game_subdirs(games: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(games)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn find_on_path(primary: &str, fallback: &str) -> Option<String> {
    let path = env::var_os("PATH")?;
    for name in [primary, fallback] {
        if env::split_paths(&path).any(|dir| dir.join(name).is_file()) {
            return Some(name.to_string());
        }
    }
    None
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
        let launcher = LinuxLauncher::native(PathBuf::from("/opt/fc/emulator/fbneo"), Runner::Direct);
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
        assert!(spec.envs.contains(&("WINEDEBUG".to_string(), "-all".to_string())));
    }

    #[test]
    fn loopback_rewrites_the_peer_ip() {
        let launcher =
            LinuxLauncher::native(PathBuf::from("/opt/fc/emulator/fbneo"), Runner::Direct)
                .loopback();
        let spec = launcher.spec(&config()).unwrap();
        assert_eq!(spec.args[0], "quark:direct,sfiii3nr1,7001,127.0.0.1,7000,0,0");
    }

    #[test]
    fn unavailable_reports_not_installed_and_errors_on_spec() {
        let launcher = LinuxLauncher {
            layout: Layout::Unavailable {
                detail: "nope".to_string(),
            },
            peer_override: None,
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

    #[test]
    fn recognizes_a_flatpak_data_dir_override() {
        let data = PathBuf::from("/home/u/.var/app/com.fightcade.Fightcade/data");
        assert_eq!(as_flatpak_data_dir(&data), Some(data.clone()));
        assert_eq!(
            as_flatpak_data_dir(&PathBuf::from("/home/u/.var/app/com.fightcade.Fightcade")),
            Some(data.clone())
        );
        assert_eq!(as_flatpak_data_dir(&PathBuf::from("/home/u/roms")), None);

        let launcher = LinuxLauncher::detect(Some(data), None);
        assert_eq!(launcher.spec(&config()).unwrap().program, PathBuf::from("flatpak"));
    }

    #[test]
    fn derives_a_native_install_from_the_rom_dir() {
        let dir = std::env::temp_dir().join(format!("cabinet-linux-roms-{}", std::process::id()));
        let fb = dir.join("emulator/fbneo");
        std::fs::create_dir_all(fb.join("ROMs")).expect("create rom dir");
        std::fs::write(fb.join("fcadefbneo"), b"#!/bin/sh\n").expect("write binary");

        let launcher = LinuxLauncher::from_rom_dir(&fb.join("ROMs")).expect("native layout");
        assert_eq!(launcher.spec(&config()).unwrap().cwd, fb);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn derives_a_flatpak_install_from_the_rom_dir() {
        let roms = PathBuf::from("/home/u/.var/app/com.fightcade.Fightcade/data/ROMs/fbneo");
        let launcher = LinuxLauncher::from_rom_dir(&roms).expect("flatpak layout");
        assert_eq!(launcher.spec(&config()).unwrap().program, PathBuf::from("flatpak"));
    }

    #[test]
    fn games_subdirs_are_sorted_and_directory_only() {
        let dir = std::env::temp_dir().join(format!("cabinet-linux-games-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("zeta")).expect("create zeta");
        std::fs::create_dir_all(dir.join("alpha")).expect("create alpha");
        std::fs::write(dir.join("file.txt"), b"ignore").expect("write file");

        let found = game_subdirs(&dir);
        assert_eq!(found, vec![dir.join("alpha"), dir.join("zeta")]);

        std::fs::remove_dir_all(&dir).ok();
    }
}
