use crate::config::Config;
use crate::logging;
use crate::providers;
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
    let _ = writeln!(
        out,
        "NOTE: the raw app-log tail at the end is not redacted; review this bundle before sharing it publicly."
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
    match providers::resolve_provider(app, &cfg, false) {
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
                for dir in dirs
                    .iter()
                    .rev()
                    .take(crate::constants::DIAGNOSTICS_SESSION_LIST_LIMIT)
                {
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

    if let Ok(sessions) = logging::sessions_dir(app) {
        if let Some(latest) = latest_session_dir(&sessions) {
            let logs = emulator_logs(&latest);
            if !logs.is_empty() {
                let _ = writeln!(
                    out,
                    "\n--- latest session emulator logs ({}) ---",
                    latest.display()
                );
                for log in logs {
                    let name = log
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("emulator");
                    let _ = writeln!(
                        out,
                        "\n### {name} (last {} lines)",
                        crate::constants::DIAGNOSTICS_LOG_TAIL_LINES
                    );
                    let _ = writeln!(
                        out,
                        "{}",
                        tail_text(&log, crate::constants::DIAGNOSTICS_LOG_TAIL_LINES)
                    );
                }
            }
        }
    }

    let tail = match logging::app_log_path(app) {
        Ok(path) => tail_text(&path, crate::constants::DIAGNOSTICS_LOG_TAIL_LINES),
        Err(err) => err.to_string(),
    };
    format!(
        "{}\n--- recent app log (last {} lines) ---\n{tail}",
        redact(&out, &cfg),
        crate::constants::DIAGNOSTICS_LOG_TAIL_LINES
    )
}

fn latest_session_dir(sessions: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(sessions)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.pop()
}

fn emulator_logs(dir: &Path) -> Vec<PathBuf> {
    let mut logs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("emulator-"))
                .unwrap_or(false)
        })
        .collect();
    logs.sort();
    logs
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn redact(text: &str, cfg: &Config) -> String {
    let text = redact_home(text);
    let text = redact_configured(&text, cfg);
    let text = redact_ipv4(&text);
    let text = redact_ipv6(&text);
    redact_magicdns(&text)
}

fn redact_configured(text: &str, cfg: &Config) -> String {
    let mut out = text.to_string();
    for (value, placeholder) in [
        (cfg.handle.as_deref(), "<handle>"),
        (cfg.default_peer_ip.as_deref(), "<peer-ip>"),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            out = out.replace(value, placeholder);
        }
    }
    out
}

fn redact_home(text: &str) -> String {
    let Some(home) = home_dir() else {
        return text.to_string();
    };
    let home = home.to_string_lossy();
    if home.is_empty() {
        return text.to_string();
    }
    text.replace(home.as_ref(), "~")
}

fn redact_ipv4(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            token.push(ch);
        } else {
            push_redacted_token(&mut token, &mut out);
            out.push(ch);
        }
    }
    push_redacted_token(&mut token, &mut out);
    out
}

fn push_redacted_token(token: &mut String, out: &mut String) {
    if token.is_empty() {
        return;
    }
    if token.parse::<std::net::Ipv4Addr>().is_ok() {
        out.push_str("100.x.x.x");
    } else {
        out.push_str(token);
    }
    token.clear();
}

fn redact_ipv6(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_ascii_hexdigit() || ch == ':' {
            token.push(ch);
        } else {
            push_redacted_ipv6_token(&mut token, &mut out);
            out.push(ch);
        }
    }
    push_redacted_ipv6_token(&mut token, &mut out);
    out
}

fn push_redacted_ipv6_token(token: &mut String, out: &mut String) {
    if token.is_empty() {
        return;
    }
    if token.contains(':') && token.parse::<std::net::Ipv6Addr>().is_ok() {
        out.push_str("fdxx::x");
    } else {
        out.push_str(token);
    }
    token.clear();
}

fn redact_magicdns(text: &str) -> String {
    const SUFFIX: &str = ".ts.net";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find(SUFFIX) {
        let bytes = rest.as_bytes();
        let mut start = index;
        while start > 0 {
            let prev = bytes[start - 1];
            if prev.is_ascii_alphanumeric() || prev == b'-' || prev == b'.' {
                start -= 1;
            } else {
                break;
            }
        }
        let mut end = index + SUFFIX.len();
        while end < bytes.len() {
            let next = bytes[end];
            if next.is_ascii_alphanumeric() || next == b'-' || next == b'.' {
                end += 1;
            } else {
                break;
            }
        }
        out.push_str(&rest[..start]);
        out.push_str("example-tailnet.ts.net");
        rest = &rest[end..];
    }
    out.push_str(rest);
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

    #[test]
    fn redacts_ipv4_addresses() {
        assert_eq!(
            redact_ipv4("connected to 100.64.0.2:41641 via 192.0.2.101"),
            "connected to 100.x.x.x:41641 via 100.x.x.x"
        );
    }

    #[test]
    fn redacts_ipv6_addresses() {
        assert_eq!(
            redact_ipv6("via [fd7a:115c:a1e0::f801:3fab]:41641 at 12:34:56"),
            "via [fdxx::x]:41641 at 12:34:56"
        );
    }

    #[test]
    fn redacts_magicdns_names() {
        assert_eq!(
            redact_magicdns(
                "self mac-host.example-tailnet.ts.net. peer windows-host.example-tailnet.ts.net."
            ),
            "self example-tailnet.ts.net peer example-tailnet.ts.net"
        );
    }

    #[test]
    fn redacts_configured_handle_and_peer() {
        let cfg = Config {
            handle: Some("player-one".into()),
            default_peer_ip: Some("100.64.0.2".into()),
            ..Config::default()
        };
        assert_eq!(
            redact("hi player-one at 100.64.0.2", &cfg),
            "hi <handle> at <peer-ip>"
        );
    }

    #[test]
    fn leaves_version_like_tokens_alone() {
        assert_eq!(redact_ipv4("v1.0.0.03 GIT6bb3167"), "v1.0.0.03 GIT6bb3167");
        assert_eq!(redact_ipv4("RetroArch 1.22.2"), "RetroArch 1.22.2");
    }

    #[test]
    fn latest_session_dir_picks_the_newest() {
        let root = std::env::temp_dir().join(format!("cabinet-latest-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        for name in ["20260101-000000", "20260103-000000", "20260102-000000"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        std::fs::write(root.join("stray.txt"), b"x").unwrap();

        assert_eq!(
            latest_session_dir(&root),
            Some(root.join("20260103-000000"))
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn emulator_logs_filters_and_sorts() {
        let root = std::env::temp_dir().join(format!("cabinet-logs-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        for name in ["emulator-p2.log", "emulator-p1.log", "notes.txt"] {
            std::fs::write(root.join(name), b"x").unwrap();
        }

        let logs = emulator_logs(&root);
        assert_eq!(logs.len(), 2);
        assert!(logs[0].ends_with("emulator-p1.log"));
        assert!(logs[1].ends_with("emulator-p2.log"));

        std::fs::remove_dir_all(&root).ok();
    }
}
