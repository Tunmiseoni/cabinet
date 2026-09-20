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
    let dir = sessions_dir(app)?.join(utc_stamp());
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    Ok(dir)
}

pub fn emulator_log_path(session_dir: &std::path::Path, role: &str) -> PathBuf {
    session_dir.join(format!("emulator-{role}.log"))
}

pub fn open_emulator_log(session_dir: &std::path::Path, role: &str) -> Result<Arc<Mutex<File>>, String> {
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

pub fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_089), (2025, 1, 1));
    }

    #[test]
    fn utc_stamp_has_the_expected_shape() {
        let stamp = utc_stamp();
        assert_eq!(stamp.len(), 15, "got {stamp}");
        assert_eq!(&stamp[8..9], "-");
    }

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
