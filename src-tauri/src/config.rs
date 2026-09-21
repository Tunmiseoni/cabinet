use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::providers::ProviderKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RetroArchInput {
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
    pub light_punch: String,
    pub medium_punch: String,
    pub heavy_punch: String,
    pub light_kick: String,
    pub medium_kick: String,
    pub heavy_kick: String,
    pub start: String,
    pub coin: String,
}

impl Default for RetroArchInput {
    fn default() -> Self {
        Self {
            up: "space".into(),
            down: "s".into(),
            left: "a".into(),
            right: "d".into(),
            light_punch: "u".into(),
            medium_punch: "i".into(),
            heavy_punch: "o".into(),
            light_kick: "j".into(),
            medium_kick: "k".into(),
            heavy_kick: "l".into(),
            start: "num1".into(),
            coin: "num5".into(),
        }
    }
}

impl RetroArchInput {
    pub fn key_values(&self) -> [&str; 12] {
        [
            &self.up,
            &self.down,
            &self.left,
            &self.right,
            &self.light_punch,
            &self.medium_punch,
            &self.heavy_punch,
            &self.light_kick,
            &self.medium_kick,
            &self.heavy_kick,
            &self.start,
            &self.coin,
        ]
    }
}

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
    pub retroarch_command_port: u16,
    pub retroarch_nickname: Option<String>,
    pub retroarch_mute_spectators: bool,
    pub retroarch_max_ping_ms: u32,
    pub retroarch_isolated_config: bool,
    pub retroarch_input: RetroArchInput,
    pub retroarch_input_enabled: bool,
    pub lobby_beacon_port: u16,
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
            retroarch_command_port: crate::constants::RETROARCH_DEFAULT_COMMAND_PORT,
            retroarch_nickname: None,
            retroarch_mute_spectators: true,
            retroarch_max_ping_ms: 0,
            retroarch_isolated_config: false,
            retroarch_input: RetroArchInput::default(),
            retroarch_input_enabled: true,
            lobby_beacon_port: crate::constants::LOBBY_BEACON_PORT,
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

    /// Developer mode is session-scoped: it must not carry across launches, even when a
    /// previous session saved it on. Returns whether it had been left on.
    pub fn reset_developer_mode(&mut self) -> bool {
        let was_on = self.developer_mode;
        self.developer_mode = false;
        was_on
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

/// The netplay nickname a provider will use, and the identity the lobby keys seats by: the explicit
/// `retroarchNickname`, else the handle, else the tailnet hostname, else the OS hostname, else
/// `"player"`. Must stay in step with how the RetroArch provider resolves `--nick`.
pub(crate) fn netplay_nickname(cfg: &Config, tailnet_hostname: Option<&str>) -> String {
    let explicit = cfg
        .retroarch_nickname
        .as_deref()
        .or(cfg.handle.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let tailnet = tailnet_hostname
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let nickname = explicit
        .map(str::to_string)
        .or_else(|| tailnet.map(str::to_string))
        .or_else(crate::env::os_hostname)
        .unwrap_or_else(|| "player".to_string());
    nickname.replace(['"', '\n', '\r'], "")
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
        assert_eq!(
            config.retroarch_command_port,
            crate::constants::RETROARCH_DEFAULT_COMMAND_PORT
        );
        assert!(config.retroarch_core.is_none());
    }

    #[test]
    fn reset_developer_mode_turns_it_off_and_reports_the_prior_state() {
        let mut config = Config {
            developer_mode: true,
            ..Config::default()
        };
        assert!(config.reset_developer_mode());
        assert!(!config.developer_mode);
        assert!(!config.reset_developer_mode());
    }

    #[test]
    fn loads_a_config_without_provider_fields() {
        let config: Config = serde_json::from_str(
            r#"{"handle":"player-one","cabinetMode":true,"rttWarnMs":150,"pollIntervalSecs":10}"#,
        )
        .unwrap();
        assert_eq!(config.provider, ProviderKind::Fightcade);
        assert!(config.cabinet_mode);
        assert!(!config.verbose_logging);
        assert!(!config.developer_mode);
        assert!(config.retroarch_mute_spectators);
        assert_eq!(config.retroarch_max_ping_ms, 0);
        assert!(!config.retroarch_isolated_config);
        assert!(config.retroarch_input_enabled);
        assert_eq!(config.retroarch_input, RetroArchInput::default());
        assert_eq!(
            config.retroarch_command_port,
            crate::constants::RETROARCH_DEFAULT_COMMAND_PORT
        );
    }

    #[test]
    fn round_trips_the_provider_fields() {
        let config = Config {
            provider: ProviderKind::Retroarch,
            retroarch_path: Some("/opt/RetroArch".into()),
            retroarch_core: Some("/tmp/fbneo.so".into()),
            retroarch_port: 60000,
            retroarch_nickname: Some("player-one".into()),
            ..Config::default()
        };
        let raw = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&raw).unwrap();
        assert_eq!(loaded.provider, ProviderKind::Retroarch);
        assert_eq!(loaded.retroarch_path.as_deref(), Some("/opt/RetroArch"));
        assert_eq!(loaded.retroarch_core.as_deref(), Some("/tmp/fbneo.so"));
        assert_eq!(loaded.retroarch_port, 60000);
        assert_eq!(loaded.retroarch_nickname.as_deref(), Some("player-one"));
    }

    #[test]
    fn round_trips_the_retroarch_netplay_fields() {
        let config = Config {
            retroarch_mute_spectators: false,
            retroarch_max_ping_ms: 150,
            retroarch_isolated_config: true,
            retroarch_command_port: 60010,
            ..Config::default()
        };
        let raw = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&raw).unwrap();
        assert!(!loaded.retroarch_mute_spectators);
        assert_eq!(loaded.retroarch_max_ping_ms, 150);
        assert!(loaded.retroarch_isolated_config);
        assert_eq!(loaded.retroarch_command_port, 60010);
    }

    #[test]
    fn retroarch_input_defaults_to_the_sf3_preset() {
        let input = RetroArchInput::default();
        assert_eq!(input.up, "space");
        assert_eq!(input.down, "s");
        assert_eq!(input.left, "a");
        assert_eq!(input.right, "d");
        assert_eq!(input.light_punch, "u");
        assert_eq!(input.medium_punch, "i");
        assert_eq!(input.heavy_punch, "o");
        assert_eq!(input.light_kick, "j");
        assert_eq!(input.medium_kick, "k");
        assert_eq!(input.heavy_kick, "l");
        assert_eq!(input.start, "num1");
        assert_eq!(input.coin, "num5");
        assert_eq!(input.key_values().len(), 12);
    }

    #[test]
    fn round_trips_the_retroarch_input_fields() {
        let input = RetroArchInput {
            up: "w".into(),
            heavy_kick: "nul".into(),
            ..RetroArchInput::default()
        };
        let config = Config {
            retroarch_input: input.clone(),
            retroarch_input_enabled: false,
            ..Config::default()
        };
        let raw = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&raw).unwrap();
        assert_eq!(loaded.retroarch_input, input);
        assert!(!loaded.retroarch_input_enabled);
    }

    #[test]
    fn partial_retroarch_input_fills_missing_fields_with_the_preset() {
        let loaded: Config = serde_json::from_str(r#"{"retroarchInput":{"up":"w"}}"#).unwrap();
        assert_eq!(loaded.retroarch_input.up, "w");
        assert_eq!(loaded.retroarch_input.coin, "num5");
        assert!(loaded.retroarch_input_enabled);
    }
}
