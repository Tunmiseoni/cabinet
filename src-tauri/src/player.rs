use crate::config::Config;
use crate::tailscale::{Peer, Tailnet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub node_id: String,
    pub handle: String,
    pub ip: String,
}

impl Player {
    pub fn new(
        node_id: impl Into<String>,
        handle: impl Into<String>,
        ip: impl Into<String>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            handle: handle.into(),
            ip: ip.into(),
        }
    }
}

impl From<&Peer> for Player {
    fn from(peer: &Peer) -> Self {
        Self::new(peer.node_id.clone(), peer.hostname.clone(), peer.ip.clone())
    }
}

pub fn handle_for(cfg: &Config, peer: &Peer) -> String {
    cfg.handle
        .as_deref()
        .map(str::trim)
        .filter(|handle| !handle.is_empty())
        .unwrap_or(&peer.hostname)
        .to_string()
}

pub fn self_player(cfg: &Config, tailnet: &Tailnet) -> Option<Player> {
    let peer = tailnet.self_peer.as_ref()?;
    Some(Player::new(
        peer.node_id.clone(),
        handle_for(cfg, peer),
        peer.ip.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer() -> Peer {
        Peer {
            node_id: "n123".to_string(),
            hostname: "mac-host".to_string(),
            dns_name: "mac-host.example.ts.net.".to_string(),
            os: "macOS".to_string(),
            ip: "100.0.0.1".to_string(),
            ips: vec!["100.0.0.1".to_string()],
            online: true,
            active: false,
            cur_addr: String::new(),
            relay: String::new(),
            rx_bytes: 0,
            tx_bytes: 0,
            last_seen: String::new(),
            is_self: true,
        }
    }

    fn tailnet() -> Tailnet {
        Tailnet {
            backend_state: "Running".to_string(),
            self_peer: Some(peer()),
            peers: Vec::new(),
        }
    }

    #[test]
    fn serializes_camel_case() {
        let raw = serde_json::to_string(&Player::new("n123", "Tunmise", "100.x.x.x")).unwrap();
        assert!(raw.contains("\"nodeId\""));
        assert!(raw.contains("\"handle\""));
    }

    #[test]
    fn handle_prefers_config_and_falls_back_to_hostname() {
        let peer = peer();

        let cfg = Config {
            handle: Some("  Tunmise  ".to_string()),
            ..Config::default()
        };
        assert_eq!(handle_for(&cfg, &peer), "Tunmise");

        let cfg = Config {
            handle: Some("   ".to_string()),
            ..Config::default()
        };
        assert_eq!(handle_for(&cfg, &peer), "mac-host");
    }

    #[test]
    fn self_player_uses_handle_and_node_id() {
        let cfg = Config {
            handle: Some("Tunmise".to_string()),
            ..Config::default()
        };
        let me = self_player(&cfg, &tailnet()).expect("self present");
        assert_eq!(me.node_id, "n123");
        assert_eq!(me.handle, "Tunmise");
        assert_eq!(me.ip, "100.0.0.1");
    }

    #[test]
    fn peer_converts_to_player() {
        let player = Player::from(&peer());
        assert_eq!(player.node_id, "n123");
        assert_eq!(player.handle, "mac-host");
    }
}
