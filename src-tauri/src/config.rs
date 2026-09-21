use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::providers::ProviderKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub handle: Option<String>,
    pub fightcade_dir: Option<String>,
    pub rom_dir: Option<String>,
    pub tailscale_path: Option<String>,
    pub default_peer_ip: Option<String>,
    pub rtt_warn_ms: u32,
    pub poll_interval_secs: u32,
    pub cabinet_mode: bool,
    pub provider: ProviderKind,
    pub retroarch_path: Option<String>,
    pub retroarch_core: Option<String>,
    pub retroarch_port: u16,
    pub retroarch_nickname: Option<String>,
    pub verbose_logging: bool,
    pub developer_mode: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            handle: None,
            fightcade_dir: None,
            rom_dir: None,
            tailscale_path: None,
            default_peer_ip: None,
            rtt_warn_ms: 150,
            poll_interval_secs: 10,
            cabinet_mode: false,
            provider: ProviderKind::default(),
            retroarch_path: None,
            retroarch_core: None,
            retroarch_port: crate::constants::RETROARCH_DEFAULT_PORT,
            retroarch_nickname: None,
            verbose_logging: false,
            developer_mode: false,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_the_fightcade_provider() {
        let config = Config::default();
        assert_eq!(config.provider, ProviderKind::Fightcade);
        assert_eq!(
            config.retroarch_port,
            crate::constants::RETROARCH_DEFAULT_PORT
        );
        assert!(config.retroarch_core.is_none());
    }

    #[test]
    fn loads_a_config_without_provider_fields() {
        let config: Config = serde_json::from_str(
            r#"{"handle":"Tunmise","cabinetMode":true,"rttWarnMs":150,"pollIntervalSecs":10}"#,
        )
        .unwrap();
        assert_eq!(config.provider, ProviderKind::Fightcade);
        assert!(config.cabinet_mode);
        assert!(!config.verbose_logging);
        assert!(!config.developer_mode);
    }

    #[test]
    fn round_trips_the_provider_fields() {
        let config = Config {
            provider: ProviderKind::Retroarch,
            retroarch_path: Some("/opt/RetroArch".into()),
            retroarch_core: Some("/tmp/fbneo.so".into()),
            retroarch_port: 60000,
            retroarch_nickname: Some("Tunmise".into()),
            ..Config::default()
        };
        let raw = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&raw).unwrap();
        assert_eq!(loaded.provider, ProviderKind::Retroarch);
        assert_eq!(loaded.retroarch_path.as_deref(), Some("/opt/RetroArch"));
        assert_eq!(loaded.retroarch_core.as_deref(), Some("/tmp/fbneo.so"));
        assert_eq!(loaded.retroarch_port, 60000);
        assert_eq!(loaded.retroarch_nickname.as_deref(), Some("Tunmise"));
    }
}
