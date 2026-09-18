use crate::config::Config;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rom {
    pub short_name: String,
    pub file_name: String,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomIndex {
    pub dir: Option<String>,
    pub roms: Vec<Rom>,
}

pub fn default_rom_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from(
            "/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo/ROMs",
        ));
    }

    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(
                home.join(".var/app/com.fightcade.Fightcade/data/ROMs/fbneo"),
            );
            dirs.push(home.join("fightcade/emulator/fbneo/ROMs"));
            dirs.push(home.join("Fightcade/emulator/fbneo/ROMs"));
        }
    }

    if cfg!(target_os = "windows") {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("Fightcade/emulator/fbneo/ROMs"));
        }
    }

    dirs
}

pub fn resolve_rom_dir(cfg: &Config) -> Option<PathBuf> {
    if let Some(custom) = cfg.rom_dir.as_deref() {
        return Some(PathBuf::from(custom));
    }
    default_rom_dirs().into_iter().find(|dir| dir.is_dir())
}

pub fn scan(dir: &Path) -> std::io::Result<Vec<Rom>> {
    let mut roms = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_zip = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("zip"))
            .unwrap_or(false);
        if !is_zip {
            continue;
        }
        let file_name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };
        let short_name = file_name
            .strip_suffix(".zip")
            .or_else(|| file_name.strip_suffix(".ZIP"))
            .unwrap_or(&file_name)
            .to_string();
        let size_bytes = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        roms.push(Rom {
            short_name,
            file_name,
            path: path.to_string_lossy().to_string(),
            size_bytes,
        });
    }
    roms.sort_by_key(|rom| rom.short_name.to_lowercase());
    Ok(roms)
}

pub fn index(cfg: &Config) -> RomIndex {
    let dir = resolve_rom_dir(cfg);
    let roms = dir
        .as_deref()
        .and_then(|dir| scan(dir).ok())
        .unwrap_or_default();
    RomIndex {
        dir: dir.map(|dir| dir.to_string_lossy().to_string()),
        roms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_zip_files_into_short_names() {
        let dir = std::env::temp_dir().join(format!("cabinet-roms-{}", std::process::id()));
        let nested = dir.join("nested");
        fs::create_dir_all(&nested).expect("create temp rom dir");
        fs::write(dir.join("sfiii3nr1.zip"), b"a").expect("write zip");
        fs::write(dir.join("UPPER.ZIP"), b"bb").expect("write upper zip");
        fs::write(dir.join("notes.txt"), b"ignore").expect("write txt");
        fs::write(nested.join("nested.zip"), b"ignore").expect("write nested zip");

        let roms = scan(&dir).expect("scan");
        let names: Vec<&str> = roms.iter().map(|r| r.short_name.as_str()).collect();
        assert_eq!(names, vec!["sfiii3nr1", "UPPER"]);
        let upper = roms.iter().find(|r| r.short_name == "UPPER").expect("upper");
        assert_eq!(upper.size_bytes, 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_dir_indexes_empty() {
        let cfg = Config {
            rom_dir: Some("/definitely/not/a/real/path".into()),
            ..Config::default()
        };
        let result = index(&cfg);
        assert_eq!(result.roms.len(), 0);
        assert_eq!(result.dir.as_deref(), Some("/definitely/not/a/real/path"));
    }

    #[test]
    #[ignore = "requires a local FightCade ROM directory"]
    fn live_index_smoke() {
        let result = index(&Config::default());
        println!("dir={:?} roms={}", result.dir, result.roms.len());
        assert!(result.dir.is_some(), "a ROM directory should be detected");
    }
}
