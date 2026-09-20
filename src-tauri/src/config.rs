use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub handle: Option<String>,
    pub fightcade_dir: Option<String>,
    pub rom_dir: Option<String>,
    pub tailscale_path: Option<String>,
    pub default_peer_ip: Option<String>,
    pub discovery_port: u16,
    pub control_port: u16,
    pub rtt_warn_ms: u32,
    pub poll_interval_secs: u32,
    pub cabinet_mode: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            handle: None,
            fightcade_dir: None,
            rom_dir: None,
            tailscale_path: None,
            default_peer_ip: None,
            discovery_port: crate::discovery::DEFAULT_PORT,
            control_port: crate::control::DEFAULT_PORT,
            rtt_warn_ms: 150,
            poll_interval_secs: 10,
            cabinet_mode: false,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, raw)
    }
}

pub fn config_path(app_config_dir: PathBuf) -> PathBuf {
    app_config_dir.join("config.json")
}
