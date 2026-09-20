use crate::logging;
use crate::provider;
use crate::session;
use crate::tailscale;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsResult {
    pub path: String,
    pub log_dir: String,
}

#[tauri::command]
pub fn log_dir(app: AppHandle) -> Result<String, String> {
    Ok(logging::app_log_dir(&app)?.to_string_lossy().to_string())
}

#[tauri::command]
pub fn open_logs_dir(app: AppHandle) -> Result<String, String> {
    let dir = logging::app_log_dir(&app)?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|err| format!("cannot open {}: {err}", dir.display()))?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command(async)]
pub fn collect_diagnostics(app: AppHandle) -> Result<DiagnosticsResult, String> {
    let dir = logging::app_log_dir(&app)?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let path = dir.join(format!("diagnostics-{}.txt", crate::time::utc_stamp()));
    let text = diagnostics_text(&app);
    std::fs::write(&path, text).map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    log::info!("diagnostics written to {}", path.display());
    let _ = app
        .opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>);
    Ok(DiagnosticsResult {
        path: path.to_string_lossy().to_string(),
        log_dir: dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn log_frontend(level: String, message: String, context: Option<String>) {
    use std::str::FromStr;
    let level = log::Level::from_str(&level).unwrap_or(log::Level::Info);
    let suffix = context
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" ({value})"))
        .unwrap_or_default();
    log::log!(target: "webview", level, "{message}{suffix}");
}

fn diagnostics_text(app: &AppHandle) -> String {
    use std::fmt::Write as _;

    let cfg = crate::commands::load_config(app).unwrap_or_default();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "The Cabinet diagnostics — {}",
        crate::time::utc_stamp()
    );
    let _ = writeln!(out, "version: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(
        out,
        "platform: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    match logging::app_log_dir(app) {
        Ok(dir) => {
            let _ = writeln!(out, "log dir: {}", dir.display());
        }
        Err(err) => {
            let _ = writeln!(out, "log dir error: {err}");
        }
    }

    let _ = writeln!(out, "\n--- config ---");
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&cfg).unwrap_or_else(|_| "<unavailable>".to_string())
    );

    let _ = writeln!(out, "\n--- provider ---");
    match provider::resolve_provider(app, &cfg, false) {
        Ok(provider) => {
            let _ = writeln!(out, "kind: {:?}", provider.kind());
            match provider.detect() {
                Ok(install) => {
                    let _ = writeln!(
                        out,
                        "install: {} installed={} detail={}",
                        install.label, install.installed, install.detail
                    );
                }
                Err(err) => {
                    let _ = writeln!(out, "install error: {err}");
                }
            }
            let caps = provider.capabilities();
            let _ = writeln!(
                out,
                "capabilities: spectate={} devPair={}",
                caps.spectate, caps.dev_pair
            );
        }
        Err(err) => {
            let _ = writeln!(out, "provider error: {err}");
        }
    }

    let _ = writeln!(out, "\n--- tailnet ---");
    match tailscale::resolve_binary(&cfg) {
        Ok(binary) => match tailscale::status(&binary) {
            Ok(tailnet) => {
                let online = tailnet.peers.iter().filter(|peer| peer.online).count();
                let _ = writeln!(out, "backendState: {}", tailnet.backend_state);
                let _ = writeln!(out, "peers: {} ({online} online)", tailnet.peers.len());
            }
            Err(err) => {
                let _ = writeln!(out, "status error: {err}");
            }
        },
        Err(err) => {
            let _ = writeln!(out, "tailscale binary error: {err}");
        }
    }

    let _ = writeln!(out, "\n--- last match ---");
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&session::status(app))
            .unwrap_or_else(|_| "<unavailable>".to_string())
    );

    let _ = writeln!(out, "\n--- sessions ---");
    match logging::sessions_dir(app) {
        Ok(sessions) => match std::fs::read_dir(&sessions) {
            Ok(entries) => {
                let mut dirs: Vec<PathBuf> = entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())
                    .collect();
                dirs.sort();
                for dir in dirs.iter().rev().take(10) {
                    let _ = writeln!(out, "{}", dir.display());
                    if let Ok(files) = std::fs::read_dir(dir) {
                        for file in files.flatten() {
                            let size = file.metadata().map(|meta| meta.len()).unwrap_or(0);
                            let _ = writeln!(out, "  {} ({size} bytes)", file.path().display());
                        }
                    }
                }
            }
            Err(err) => {
                let _ = writeln!(out, "cannot list sessions: {err}");
            }
        },
        Err(err) => {
            let _ = writeln!(out, "{err}");
        }
    }

    let _ = writeln!(out, "\n--- recent app log (last 200 lines) ---");
    match logging::app_log_path(app) {
        Ok(path) => {
            let _ = writeln!(out, "{}", tail_text(&path, 200));
        }
        Err(err) => {
            let _ = writeln!(out, "{err}");
        }
    }

    out
}

fn tail_text(path: &Path, max_lines: usize) -> String {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return format!("(no log at {})", path.display());
    };
    let lines: Vec<&str> = raw.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_text_returns_the_last_lines() {
        let path = std::env::temp_dir().join(format!("cabinet-tail-{}.txt", std::process::id()));
        std::fs::write(&path, "a\nb\nc\nd\n").unwrap();
        assert_eq!(tail_text(&path, 2), "c\nd");
        assert_eq!(tail_text(&path, 10), "a\nb\nc\nd");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn tail_text_reports_a_missing_file() {
        let path = std::env::temp_dir().join("cabinet-definitely-missing.txt");
        assert!(tail_text(&path, 5).starts_with("(no log at"));
    }
}
