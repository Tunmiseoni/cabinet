use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::sync::MutexExt;
use tauri::{AppHandle, Manager};

pub const APP_LOG_FILE: &str = "the-cabinet";
pub const MAX_LOG_BYTES: u128 = 5_000_000;
pub const KEEP_LOG_FILES: usize = 3;
pub const SESSIONS_DIR: &str = "sessions";
pub const EMULATOR_TARGET: &str = "emulator";

pub fn app_log_dir(app: &AppHandle) -> crate::error::Result<PathBuf> {
    app.path()
        .app_log_dir()
        .map_err(|err| format!("cannot resolve log dir: {err}"))
        .map_err(Into::into)
}

pub fn app_log_path(app: &AppHandle) -> crate::error::Result<PathBuf> {
    Ok(app_log_dir(app)?.join(format!("{APP_LOG_FILE}.log")))
}

pub fn sessions_dir(app: &AppHandle) -> crate::error::Result<PathBuf> {
    Ok(app_log_dir(app)?.join(SESSIONS_DIR))
}

pub fn create_session_dir(app: &AppHandle) -> crate::error::Result<PathBuf> {
    let dir = sessions_dir(app)?.join(crate::time::utc_stamp());
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    Ok(dir)
}

pub fn prune_sessions(app: &AppHandle) -> crate::error::Result<usize> {
    prune_session_dirs(&sessions_dir(app)?, crate::constants::SESSION_KEEP)
}

pub fn prune_session_dirs(dir: &Path, keep: usize) -> crate::error::Result<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(0),
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    if dirs.len() <= keep {
        return Ok(0);
    }
    let remove = dirs.len() - keep;
    let mut removed = 0;
    for path in dirs.into_iter().take(remove) {
        if std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

pub fn emulator_log_path(session_dir: &std::path::Path, role: &str) -> PathBuf {
    session_dir.join(format!("emulator-{role}.log"))
}

pub fn stamp_line(line: &str) -> String {
    format!("[{}] {line}", crate::time::utc_stamp())
}

pub fn open_emulator_log(
    session_dir: &std::path::Path,
    role: &str,
) -> crate::error::Result<Arc<Mutex<File>>> {
    let path = emulator_log_path(session_dir, role);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| format!("cannot open {}: {err}", path.display()))?;
    Ok(Arc::new(Mutex::new(file)))
}

pub fn capture_stream<R>(stream: R, label: String, sink: Arc<Mutex<File>>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            let mut file = sink.lock_or_recover();
            let _ = writeln!(file, "{}", stamp_line(&line));
            let _ = file.flush();
            log::debug!(target: EMULATOR_TARGET, "{label} {line}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulator_log_path_is_per_role() {
        let dir = std::path::Path::new("/tmp/session");
        assert_eq!(
            emulator_log_path(dir, "p1"),
            std::path::PathBuf::from("/tmp/session/emulator-p1.log")
        );
        assert_eq!(
            emulator_log_path(dir, "spectator"),
            std::path::PathBuf::from("/tmp/session/emulator-spectator.log")
        );
    }

    #[test]
    fn stamp_line_prefixes_a_utc_stamp() {
        let line = stamp_line("hello");
        assert!(line.starts_with('['), "got {line}");
        assert!(line.ends_with("] hello"), "got {line}");
    }

    #[test]
    fn prunes_oldest_session_dirs() {
        let root = std::env::temp_dir().join(format!("cabinet-sessions-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        for name in ["20260101-000000", "20260102-000000", "20260103-000000"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }

        assert_eq!(prune_session_dirs(&root, 2).unwrap(), 1);
        assert!(!root.join("20260101-000000").exists());
        assert!(root.join("20260103-000000").exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn prune_keeps_everything_under_the_cap() {
        let root =
            std::env::temp_dir().join(format!("cabinet-sessions-keep-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("20260101-000000")).unwrap();

        assert_eq!(prune_session_dirs(&root, 5).unwrap(), 0);
        assert!(root.join("20260101-000000").exists());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn prune_tolerates_a_missing_dir() {
        assert_eq!(
            prune_session_dirs(Path::new("/nonexistent/cabinet/sessions"), 3).unwrap(),
            0
        );
    }
}
