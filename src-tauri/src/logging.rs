use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use tauri::{AppHandle, Manager};

pub const APP_LOG_FILE: &str = "the-cabinet";
pub const MAX_LOG_BYTES: u128 = 5_000_000;
pub const KEEP_LOG_FILES: usize = 3;
pub const SESSIONS_DIR: &str = "sessions";
pub const EMULATOR_TARGET: &str = "emulator";

pub fn app_log_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_log_dir()
        .map_err(|err| format!("cannot resolve log dir: {err}"))
}

pub fn app_log_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_log_dir(app)?.join(format!("{APP_LOG_FILE}.log")))
}

pub fn sessions_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_log_dir(app)?.join(SESSIONS_DIR))
}

pub fn create_session_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = sessions_dir(app)?.join(crate::time::utc_stamp());
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    Ok(dir)
}

pub fn emulator_log_path(session_dir: &std::path::Path, role: &str) -> PathBuf {
    session_dir.join(format!("emulator-{role}.log"))
}

pub fn open_emulator_log(
    session_dir: &std::path::Path,
    role: &str,
) -> Result<Arc<Mutex<File>>, String> {
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
            if let Ok(mut file) = sink.lock() {
                let _ = writeln!(file, "{line}");
                let _ = file.flush();
            }
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
}
