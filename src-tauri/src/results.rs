use serde::Serialize;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

pub const OVERLAY_DIR_NAME: &str = "fightcade";

const OVERLAY_FILES: &[&str] = &[
    "game.txt",
    "gametype.txt",
    "vs.txt",
    "started.inf",
    "p1name.txt",
    "p2name.txt",
    "p1rank.txt",
    "p2rank.txt",
    "p1character.txt",
    "p2character.txt",
    "p1score.txt",
    "p2score.txt",
    "winner.txt",
];

pub fn overlay_dir(emulator_dir: &Path) -> PathBuf {
    emulator_dir.join(OVERLAY_DIR_NAME)
}

pub fn reset(dir: &Path) {
    for name in OVERLAY_FILES {
        let _ = fs::remove_file(dir.join(name));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayStatus {
    pub enabled: bool,
    pub ini_path: String,
}

pub fn overlay_status(emulator_dir: &Path) -> OverlayStatus {
    let ini = emulator_dir.join("config").join("fcadefbneo.ini");
    let enabled = fs::read_to_string(&ini)
        .map(|raw| ini_overlay_enabled(&raw))
        .unwrap_or(false);
    OverlayStatus {
        enabled,
        ini_path: ini.to_string_lossy().to_string(),
    }
}

fn ini_overlay_enabled(raw: &str) -> bool {
    raw.lines().any(|line| {
        let line = line.trim();
        if line.starts_with("//") {
            return false;
        }
        let mut parts = line.split_whitespace();
        parts.next() == Some("bVidSaveOverlayFiles")
            && parts.next().map(|value| value != "0").unwrap_or(false)
    })
}

pub fn enable_overlay(emulator_dir: &Path) -> io::Result<OverlayStatus> {
    let ini = emulator_dir.join("config").join("fcadefbneo.ini");
    let raw = match fs::read(&ini) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| {
            io::Error::new(
                ErrorKind::InvalidData,
                "config file is not valid UTF-8; edit it manually",
            )
        })?,
        Err(err) if err.kind() == ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };

    let newline = if raw.contains("\r\n") || cfg!(target_os = "windows") {
        "\r\n"
    } else {
        "\n"
    };

    let mut found = false;
    let mut out = String::with_capacity(raw.len() + 32);
    for line in raw.lines() {
        let indent_len = line.len() - line.trim_start().len();
        let trimmed = &line[indent_len..];
        let uncommented = trimmed
            .strip_prefix("//")
            .map(str::trim_start)
            .unwrap_or(trimmed);
        if uncommented.split_whitespace().next() == Some("bVidSaveOverlayFiles") {
            out.push_str(&line[..indent_len]);
            out.push_str("bVidSaveOverlayFiles 1");
            found = true;
        } else {
            out.push_str(line);
        }
        out.push_str(newline);
    }
    if !found {
        out.push_str("bVidSaveOverlayFiles 1");
        out.push_str(newline);
    }

    if let Some(parent) = ini.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&ini, out)?;
    Ok(overlay_status(emulator_dir))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResult {
    pub rom: Option<String>,
    pub started: bool,
    pub winner: Option<String>,
    pub winner_side: Option<u8>,
    pub p1_name: Option<String>,
    pub p2_name: Option<String>,
    pub p1_score: Option<i64>,
    pub p2_score: Option<i64>,
    pub p1_character: Option<String>,
    pub p2_character: Option<String>,
}

impl MatchResult {
    pub fn is_empty(&self) -> bool {
        self == &MatchResult::default()
    }
}

pub fn read(dir: &Path) -> MatchResult {
    let p1_name = read_text(dir, "p1name.txt");
    let p2_name = read_text(dir, "p2name.txt");
    let p1_score = read_score(dir, "p1score.txt");
    let p2_score = read_score(dir, "p2score.txt");
    let winner = read_text(dir, "winner.txt");
    let winner_side = resolve_winner_side(
        winner.as_deref(),
        p1_name.as_deref(),
        p2_name.as_deref(),
        p1_score,
        p2_score,
    );

    MatchResult {
        rom: read_text(dir, "game.txt"),
        started: read_text(dir, "started.inf").as_deref() == Some("1"),
        winner,
        winner_side,
        p1_name,
        p2_name,
        p1_score,
        p2_score,
        p1_character: read_text(dir, "p1character.txt"),
        p2_character: read_text(dir, "p2character.txt"),
    }
}

fn read_text(dir: &Path, name: &str) -> Option<String> {
    let raw = fs::read_to_string(dir.join(name)).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn read_score(dir: &Path, name: &str) -> Option<i64> {
    read_text(dir, name)?.parse::<i64>().ok()
}

fn resolve_winner_side(
    winner: Option<&str>,
    p1_name: Option<&str>,
    p2_name: Option<&str>,
    p1_score: Option<i64>,
    p2_score: Option<i64>,
) -> Option<u8> {
    if let Some(winner) = winner {
        let winner = winner.trim().to_lowercase();
        let matches_p1 = p1_name
            .map(|name| name.trim().to_lowercase() == winner)
            .unwrap_or(false);
        let matches_p2 = p2_name
            .map(|name| name.trim().to_lowercase() == winner)
            .unwrap_or(false);
        match (matches_p1, matches_p2) {
            (true, false) => return Some(0),
            (false, true) => return Some(1),
            _ => {}
        }
    }

    match (p1_score, p2_score) {
        (Some(a), Some(b)) if a > b => Some(0),
        (Some(a), Some(b)) if b > a => Some(1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempOverlay {
        dir: PathBuf,
    }

    impl TempOverlay {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cabinet-results-{}-{}-{tag}",
                std::process::id(),
                unique()
            ));
            fs::create_dir_all(&dir).expect("create temp overlay dir");
            Self { dir }
        }

        fn write(&self, name: &str, contents: &str) {
            fs::write(self.dir.join(name), contents).expect("write overlay file");
        }
    }

    impl Drop for TempOverlay {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.dir).ok();
        }
    }

    fn unique() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    }

    #[test]
    fn reads_full_overlay_and_resolves_winner_by_name() {
        let overlay = TempOverlay::new("full");
        overlay.write("game.txt", "sfiii3nr1");
        overlay.write("started.inf", "1");
        overlay.write("p1name.txt", "Player1");
        overlay.write("p2name.txt", "Player2");
        overlay.write("p1score.txt", "0");
        overlay.write("p2score.txt", "1");
        overlay.write("p1character.txt", "Alex");
        overlay.write("p2character.txt", "Ryu");
        overlay.write("winner.txt", "Player2");

        let result = read(&overlay.dir);
        assert_eq!(result.rom.as_deref(), Some("sfiii3nr1"));
        assert!(result.started);
        assert_eq!(result.winner.as_deref(), Some("Player2"));
        assert_eq!(result.winner_side, Some(1));
        assert_eq!(result.p1_score, Some(0));
        assert_eq!(result.p2_score, Some(1));
        assert_eq!(result.p1_character.as_deref(), Some("Alex"));
        assert_eq!(result.p2_character.as_deref(), Some("Ryu"));
    }

    #[test]
    fn missing_dir_is_empty() {
        let dir = std::env::temp_dir().join("cabinet-results-does-not-exist");
        assert!(read(&dir).is_empty());
    }

    #[test]
    fn reset_clears_only_known_overlay_files() {
        let overlay = TempOverlay::new("reset");
        overlay.write("winner.txt", "Player2");
        overlay.write("p1score.txt", "1");
        overlay.write("chat_history.txt", "keep me");

        reset(&overlay.dir);

        assert!(!overlay.dir.join("winner.txt").exists());
        assert!(!overlay.dir.join("p1score.txt").exists());
        assert!(overlay.dir.join("chat_history.txt").exists());
        assert!(read(&overlay.dir).is_empty());
    }

    #[test]
    fn tolerates_whitespace_and_invalid_values() {
        let overlay = TempOverlay::new("tolerant");
        overlay.write("p1score.txt", " 3\n");
        overlay.write("p2score.txt", "not-a-number");
        overlay.write("winner.txt", "\n");
        overlay.write("started.inf", "0");

        let result = read(&overlay.dir);
        assert_eq!(result.p1_score, Some(3));
        assert_eq!(result.p2_score, None);
        assert_eq!(result.winner, None);
        assert!(!result.started);
    }

    #[test]
    fn resolves_winner_side_from_custom_names_case_insensitively() {
        let overlay = TempOverlay::new("names");
        overlay.write("p1name.txt", "Tunmise");
        overlay.write("p2name.txt", "Friend");
        overlay.write("winner.txt", "friend");

        assert_eq!(read(&overlay.dir).winner_side, Some(1));
    }

    #[test]
    fn falls_back_to_scores_when_winner_name_is_not_a_player() {
        let overlay = TempOverlay::new("scores");
        overlay.write("p1name.txt", "Player1");
        overlay.write("p2name.txt", "Player2");
        overlay.write("p1score.txt", "2");
        overlay.write("p2score.txt", "0");
        overlay.write("winner.txt", "unknown");

        assert_eq!(read(&overlay.dir).winner_side, Some(0));
    }

    #[test]
    fn draw_has_no_winner_side() {
        let overlay = TempOverlay::new("draw");
        overlay.write("p1name.txt", "Player1");
        overlay.write("p2name.txt", "Player2");
        overlay.write("p1score.txt", "1");
        overlay.write("p2score.txt", "1");

        let result = read(&overlay.dir);
        assert_eq!(result.winner, None);
        assert_eq!(result.winner_side, None);
        assert!(!result.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires a local FightCade install with bVidSaveOverlayFiles 1"]
    fn live_overlay_smoke() {
        let dir = overlay_dir(Path::new(
            "/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo",
        ));
        let result = read(&dir);
        println!("overlay {} -> {result:?}", dir.display());
        assert!(
            !result.is_empty(),
            "expected overlay files in {}",
            dir.display()
        );
    }

    #[test]
    fn detects_overlay_setting_in_ini() {
        assert!(ini_overlay_enabled("bVidSaveOverlayFiles 1\n"));
        assert!(!ini_overlay_enabled("bVidSaveOverlayFiles 0\n"));
        assert!(!ini_overlay_enabled("// bVidSaveOverlayFiles 1\n"));
        assert!(ini_overlay_enabled(
            "bVidOverlay 1\nbVidSaveOverlayFiles 1\nbVidBigOverlay 0\n"
        ));
        assert!(!ini_overlay_enabled("bVidSaveOverlayFiles\n"));
        assert!(!ini_overlay_enabled(""));
    }

    struct TempEmulator {
        dir: PathBuf,
    }

    impl TempEmulator {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cabinet-emulator-{}-{}-{tag}",
                std::process::id(),
                unique()
            ));
            fs::create_dir_all(dir.join("config")).expect("create temp emulator dir");
            Self { dir }
        }

        fn ini(&self) -> PathBuf {
            self.dir.join("config").join("fcadefbneo.ini")
        }

        fn write_ini(&self, contents: &str) {
            fs::write(self.ini(), contents).expect("write ini");
        }
    }

    impl Drop for TempEmulator {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.dir).ok();
        }
    }

    #[test]
    fn enable_overlay_appends_when_absent() {
        let emulator = TempEmulator::new("append");
        emulator.write_ini("bVidOverlay 1\n");

        let status = enable_overlay(&emulator.dir).expect("enable");

        assert!(status.enabled);
        assert!(ini_overlay_enabled(
            &fs::read_to_string(emulator.ini()).unwrap()
        ));
        assert!(fs::read_to_string(emulator.ini())
            .unwrap()
            .contains("bVidOverlay 1"));
    }

    #[test]
    fn enable_overlay_rewrites_a_disabled_value() {
        let emulator = TempEmulator::new("zero");
        emulator.write_ini("bVidSaveOverlayFiles 0\n");

        assert!(enable_overlay(&emulator.dir).unwrap().enabled);
        let newline = if cfg!(target_os = "windows") {
            "\r\n"
        } else {
            "\n"
        };
        assert_eq!(
            fs::read_to_string(emulator.ini()).unwrap(),
            format!("bVidSaveOverlayFiles 1{newline}")
        );
    }

    #[test]
    fn enable_overlay_uncomments_the_setting() {
        let emulator = TempEmulator::new("comment");
        emulator.write_ini("// bVidSaveOverlayFiles 1\n");

        assert!(enable_overlay(&emulator.dir).unwrap().enabled);
        let newline = if cfg!(target_os = "windows") {
            "\r\n"
        } else {
            "\n"
        };
        assert_eq!(
            fs::read_to_string(emulator.ini()).unwrap(),
            format!("bVidSaveOverlayFiles 1{newline}")
        );
    }

    #[test]
    fn enable_overlay_creates_the_file_and_dirs() {
        let dir = std::env::temp_dir().join(format!(
            "cabinet-emulator-{}-{}-create",
            std::process::id(),
            unique()
        ));

        let status = enable_overlay(&dir).expect("enable");

        assert!(status.enabled);
        assert!(dir.join("config").join("fcadefbneo.ini").is_file());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn enable_overlay_preserves_crlf_and_indentation() {
        let emulator = TempEmulator::new("crlf");
        emulator.write_ini("bVidOverlay 1\r\n  bVidSaveOverlayFiles 0\r\n");

        enable_overlay(&emulator.dir).unwrap();

        assert_eq!(
            fs::read_to_string(emulator.ini()).unwrap(),
            "bVidOverlay 1\r\n  bVidSaveOverlayFiles 1\r\n"
        );
    }

    #[test]
    fn enable_overlay_refuses_invalid_utf8() {
        let emulator = TempEmulator::new("binary");
        fs::write(emulator.ini(), [0xff, 0xfe, 0x00]).unwrap();

        let err = enable_overlay(&emulator.dir).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert_eq!(fs::read(emulator.ini()).unwrap(), vec![0xff, 0xfe, 0x00]);
    }
}
