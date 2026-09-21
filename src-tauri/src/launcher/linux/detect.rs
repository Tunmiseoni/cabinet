use super::FLATPAK_APP;
use crate::process;
use std::env;
use std::path::{Path, PathBuf};

pub(super) fn flatpak_data_dir() -> Option<PathBuf> {
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

pub(super) fn looks_like_flatpak_data_dir(dir: &Path) -> bool {
    dir.file_name().and_then(|name| name.to_str()) == Some("data")
        && dir
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            == Some(FLATPAK_APP)
}

pub(super) fn as_flatpak_data_dir(dir: &Path) -> Option<PathBuf> {
    if looks_like_flatpak_data_dir(dir) || dir.join("ROMs").is_dir() {
        return Some(dir.to_path_buf());
    }
    if dir.file_name().and_then(|name| name.to_str()) == Some(FLATPAK_APP) {
        return Some(dir.join("data"));
    }
    None
}

pub(super) fn native_candidates() -> Vec<PathBuf> {
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

pub(super) fn game_subdirs(games: &Path) -> Vec<PathBuf> {
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

pub(super) fn find_on_path(primary: &str, fallback: &str) -> Option<String> {
    [primary, fallback]
        .into_iter()
        .find(|name| crate::env::on_path(name).is_some())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::super::LinuxLauncher;
    use super::*;
    use crate::launcher::{Launcher, MatchConfig};
    use std::path::PathBuf;

    fn config() -> MatchConfig {
        MatchConfig::new("sfiii3nr1".into(), "100.64.0.2".into(), 0).unwrap()
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
        assert_eq!(
            launcher.spec(&config()).unwrap().program,
            PathBuf::from("flatpak")
        );
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
        assert_eq!(
            launcher.spec(&config()).unwrap().program,
            PathBuf::from("flatpak")
        );
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
