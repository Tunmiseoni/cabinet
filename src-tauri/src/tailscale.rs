use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub hostname: String,
    pub dns_name: String,
    pub os: String,
    pub ip: String,
    pub ips: Vec<String>,
    pub online: bool,
    pub active: bool,
    pub cur_addr: String,
    pub relay: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub last_seen: String,
    pub is_self: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tailnet {
    pub backend_state: String,
    pub self_peer: Option<Peer>,
    pub peers: Vec<Peer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PathKind {
    Direct,
    Relay,
    Local,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerHealth {
    pub ip: String,
    pub ok: bool,
    pub path: PathKind,
    pub rtt_ms: Option<f64>,
    pub relay_code: Option<String>,
    pub raw: String,
}

#[derive(Deserialize)]
struct RawStatus {
    #[serde(rename = "BackendState", default)]
    backend_state: String,
    #[serde(rename = "Self", default)]
    self_node: Option<RawPeer>,
    #[serde(rename = "Peer", default)]
    peers: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize)]
struct RawPeer {
    #[serde(rename = "HostName", default)]
    hostname: String,
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(rename = "OS", default)]
    os: String,
    #[serde(rename = "TailscaleIPs", default)]
    ips: Vec<String>,
    #[serde(rename = "Online", default)]
    online: bool,
    #[serde(rename = "Active", default)]
    active: bool,
    #[serde(rename = "CurAddr", default)]
    cur_addr: String,
    #[serde(rename = "Relay", default)]
    relay: String,
    #[serde(rename = "RxBytes", default)]
    rx_bytes: u64,
    #[serde(rename = "TxBytes", default)]
    tx_bytes: u64,
    #[serde(rename = "LastSeen", default)]
    last_seen: String,
}

impl RawPeer {
    fn into_peer(self, is_self: bool) -> Peer {
        let ip = self
            .ips
            .iter()
            .find(|ip| !ip.contains(':'))
            .cloned()
            .or_else(|| self.ips.first().cloned())
            .unwrap_or_default();
        Peer {
            hostname: self.hostname,
            dns_name: self.dns_name,
            os: self.os,
            ip,
            ips: self.ips,
            online: self.online,
            active: self.active,
            cur_addr: self.cur_addr,
            relay: self.relay,
            rx_bytes: self.rx_bytes,
            tx_bytes: self.tx_bytes,
            last_seen: self.last_seen,
            is_self,
        }
    }
}

pub fn resolve_binary(cfg: &Config) -> Result<PathBuf, String> {
    if let Some(custom) = cfg.tailscale_path.as_deref() {
        let path = PathBuf::from(custom);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!("configured tailscale path not found: {custom}"));
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from(
            "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
        ));
    }
    if cfg!(target_os = "windows") {
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            candidates.push(PathBuf::from(program_files).join("Tailscale").join("tailscale.exe"));
        }
        if let Ok(program_files) = std::env::var("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(program_files).join("Tailscale").join("tailscale.exe"));
        }
    }

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    if let Some(found) = find_on_path() {
        return Ok(found);
    }

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("tailscale binary not found (set an override in settings)".into())
}

fn find_on_path() -> Option<PathBuf> {
    let binary = if cfg!(target_os = "windows") {
        "tailscale.exe"
    } else {
        "tailscale"
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

fn run(binary: &PathBuf, args: &[&str]) -> Result<String, String> {
    let output = Command::new(binary)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run tailscale: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if stdout.trim().is_empty() && !output.status.success() {
        return Err(if stderr.trim().is_empty() {
            "tailscale command failed".to_string()
        } else {
            stderr.trim().to_string()
        });
    }
    Ok(stdout)
}

pub fn status(binary: &PathBuf) -> Result<Tailnet, String> {
    let raw = run(binary, &["status", "--json"])?;
    parse_status(&raw)
}

pub fn parse_status(raw: &str) -> Result<Tailnet, String> {
    let parsed: RawStatus =
        serde_json::from_str(raw).map_err(|err| format!("invalid status JSON: {err}"))?;

    let self_peer = parsed.self_node.map(|node| node.into_peer(true));

    let mut peers: Vec<Peer> = parsed
        .peers
        .map(|map| {
            map.into_values()
                .filter_map(|value| serde_json::from_value::<RawPeer>(value).ok())
                .map(|node| node.into_peer(false))
                .collect()
        })
        .unwrap_or_default();

    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()))
    });

    Ok(Tailnet {
        backend_state: parsed.backend_state,
        self_peer,
        peers,
    })
}

pub fn ping(binary: &PathBuf, ip: &str) -> PeerHealth {
    let output = Command::new(binary)
        .args(["ping", "--c", "1", "--timeout", "2s", ip])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let combined = format!("{stdout}{stderr}");
            parse_ping(ip, &combined)
        }
        Err(err) => PeerHealth {
            ip: ip.to_string(),
            ok: false,
            path: PathKind::Unknown,
            rtt_ms: None,
            relay_code: None,
            raw: format!("failed to run tailscale ping: {err}"),
        },
    }
}

pub fn ping_many(binary: &PathBuf, ips: &[String]) -> Vec<PeerHealth> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = ips
            .iter()
            .map(|ip| scope.spawn(|| ping(binary, ip)))
            .collect();
        handles
            .into_iter()
            .zip(ips)
            .map(|(handle, ip)| {
                handle.join().unwrap_or_else(|_| PeerHealth {
                    ip: ip.clone(),
                    ok: false,
                    path: PathKind::Unknown,
                    rtt_ms: None,
                    relay_code: None,
                    raw: "ping worker panicked".to_string(),
                })
            })
            .collect()
    })
}

pub fn parse_ping(ip: &str, raw: &str) -> PeerHealth {
    let trimmed = raw.trim().to_string();

    if trimmed.contains("is local Tailscale IP") {
        return PeerHealth {
            ip: ip.to_string(),
            ok: true,
            path: PathKind::Local,
            rtt_ms: Some(0.0),
            relay_code: None,
            raw: trimmed,
        };
    }

    let lower = trimmed.to_lowercase();
    let timed_out = lower.contains("timed out") || lower.contains("no reply");
    if timed_out {
        return PeerHealth {
            ip: ip.to_string(),
            ok: false,
            path: PathKind::Unknown,
            rtt_ms: None,
            relay_code: None,
            raw: trimmed,
        };
    }

    let rtt_ms = extract_rtt_ms(&trimmed);
    let relay_code = extract_relay_code(&trimmed);

    let path = if relay_code.is_some() {
        PathKind::Relay
    } else if trimmed.contains(" via ") {
        PathKind::Direct
    } else {
        PathKind::Unknown
    };

    let ok = rtt_ms.is_some() && path != PathKind::Unknown;

    PeerHealth {
        ip: ip.to_string(),
        ok,
        path,
        rtt_ms,
        relay_code,
        raw: trimmed,
    }
}

fn extract_rtt_ms(raw: &str) -> Option<f64> {
    let marker = " in ";
    let tail = raw.get(raw.rfind(marker)? + marker.len()..)?;
    let number: String = tail
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    number.parse::<f64>().ok()
}

fn extract_relay_code(raw: &str) -> Option<String> {
    let start = raw.find("DERP(")? + "DERP(".len();
    let rest = raw.get(start..)?;
    let end = rest.find(')')?;
    Some(rest.get(..end)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_direct_ping() {
        let health = parse_ping(
            "100.64.0.11",
            "pong from cachyos-host (100.64.0.11) via 192.168.1.101:41641 in 12ms",
        );
        assert!(health.ok);
        assert_eq!(health.path, PathKind::Direct);
        assert_eq!(health.rtt_ms, Some(12.0));
        assert_eq!(health.relay_code, None);
    }

    #[test]
    fn parses_derp_ping() {
        let health = parse_ping(
            "100.64.0.11",
            "pong from cachyos-host (100.64.0.11) via DERP(lhr) in 329ms",
        );
        assert!(health.ok);
        assert_eq!(health.path, PathKind::Relay);
        assert_eq!(health.rtt_ms, Some(329.0));
        assert_eq!(health.relay_code.as_deref(), Some("lhr"));
    }

    #[test]
    fn parses_local_ping() {
        let health = parse_ping("100.64.0.10", "100.64.0.10 is local Tailscale IP");
        assert!(health.ok);
        assert_eq!(health.path, PathKind::Local);
    }

    #[test]
    fn parses_timeout() {
        let health = parse_ping(
            "100.64.0.11",
            "ping \"100.64.0.11\" timed out\n2026/09/18 23:48:45 no reply",
        );
        assert!(!health.ok);
        assert_eq!(health.path, PathKind::Unknown);
        assert_eq!(health.rtt_ms, None);
    }

    #[test]
    fn parses_fractional_rtt() {
        let health = parse_ping(
            "100.64.0.11",
            "pong from cachyos-host (100.64.0.11) via [fd7a::1]:41641 in 4.5ms",
        );
        assert_eq!(health.rtt_ms, Some(4.5));
        assert_eq!(health.path, PathKind::Direct);
    }

    const STATUS_FIXTURE: &str = r#"{
      "Version": "1.102.4",
      "BackendState": "Running",
      "Self": {
        "HostName": "Onis-MacBook-Pro",
        "DNSName": "mac-host.example-tailnet.ts.net.",
        "OS": "macOS",
        "TailscaleIPs": ["100.64.0.10", "fd7a:115c:a1e0::f801:3fab"],
        "Online": true,
        "Active": false,
        "CurAddr": "",
        "Relay": "",
        "RxBytes": 100,
        "TxBytes": 200,
        "LastSeen": "0001-01-01T00:00:00Z"
      },
      "Peer": {
        "nodeA": {
          "HostName": "DESKTOP-8NLUFK6",
          "DNSName": "windows-host.example-tailnet.ts.net.",
          "OS": "windows",
          "TailscaleIPs": ["100.64.0.12", "fd7a:115c:a1e0::9401:b84c"],
          "Online": false,
          "Active": false,
          "CurAddr": "",
          "Relay": "lhr",
          "RxBytes": 0,
          "TxBytes": 0,
          "LastSeen": "2026-09-18T21:51:35.1Z"
        },
        "nodeB": {
          "HostName": "cachyos-host",
          "DNSName": "cachyos-host.example-tailnet.ts.net.",
          "OS": "linux",
          "TailscaleIPs": ["100.64.0.11", "fd7a:115c:a1e0::2"],
          "Online": true,
          "Active": true,
          "CurAddr": "192.168.1.101:41641",
          "Relay": "lhr",
          "RxBytes": 5000,
          "TxBytes": 6000,
          "LastSeen": "2026-09-18T22:00:00Z"
        }
      }
    }"#;

    #[test]
    fn parses_status_and_orders_online_first() {
        let tailnet = parse_status(STATUS_FIXTURE).expect("should parse");
        assert_eq!(tailnet.backend_state, "Running");
        let self_peer = tailnet.self_peer.expect("self present");
        assert!(self_peer.is_self);
        assert_eq!(self_peer.ip, "100.64.0.10");
        assert_eq!(tailnet.peers.len(), 2);
        assert_eq!(tailnet.peers[0].hostname, "cachyos-host");
        assert!(tailnet.peers[0].online);
        assert_eq!(tailnet.peers[1].hostname, "DESKTOP-8NLUFK6");
        assert_eq!(tailnet.peers[0].ip, "100.64.0.11");
    }

    #[test]
    fn tolerates_stopped_backend() {
        let tailnet = parse_status(r#"{"BackendState":"Stopped"}"#).expect("should parse");
        assert_eq!(tailnet.backend_state, "Stopped");
        assert!(tailnet.self_peer.is_none());
        assert!(tailnet.peers.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn ping_many_preserves_order() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("cabinet-ping-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let script = dir.join("faketailscale");
        std::fs::write(
            &script,
            "#!/bin/sh\necho \"pong from fake ($6) via 1.2.3.4:41641 in 7ms\"\n",
        )
        .expect("write fake binary");
        let mut perms = std::fs::metadata(&script).expect("stat").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");

        let ips = vec!["100.0.0.1".to_string(), "100.0.0.2".to_string()];
        let results = ping_many(&script, &ips);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].ip, "100.0.0.1");
        assert_eq!(results[1].ip, "100.0.0.2");
        assert!(results.iter().all(|h| h.ok && h.path == PathKind::Direct));
        assert_eq!(results[0].rtt_ms, Some(7.0));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    #[ignore = "requires a running local tailscale"]
    fn live_status_smoke() {
        let cfg = Config::default();
        let binary = resolve_binary(&cfg).expect("tailscale binary should resolve");
        let tailnet = status(&binary).expect("status should run");
        assert!(tailnet.self_peer.is_some(), "self node should be present");
        println!(
            "backend={} self={:?} peers={}",
            tailnet.backend_state,
            tailnet.self_peer.as_ref().map(|p| &p.ip),
            tailnet.peers.len()
        );
    }

    #[test]
    #[ignore = "requires a running local tailscale"]
    fn live_ping_smoke() {
        let cfg = Config::default();
        let binary = resolve_binary(&cfg).expect("tailscale binary should resolve");
        let tailnet = status(&binary).expect("status should run");
        let self_ip = tailnet.self_peer.expect("self node").ip;
        let results = ping_many(&binary, std::slice::from_ref(&self_ip));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, PathKind::Local);
        println!("ping self -> {:?}", results[0]);
    }
}
