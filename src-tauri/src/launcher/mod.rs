use serde::Serialize;
use std::path::PathBuf;

#[allow(dead_code)]
pub mod linux;
pub mod macos;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct MatchConfig {
    pub rom: String,
    pub peer_ip: String,
    pub side: u8,
}

impl MatchConfig {
    pub fn new(rom: String, peer_ip: String, side: u8) -> Result<Self, String> {
        let rom = rom.trim().to_string();
        let peer_ip = peer_ip.trim().to_string();
        if rom.is_empty() {
            return Err("a ROM short name is required".into());
        }
        if peer_ip.is_empty() {
            return Err("a peer IP is required".into());
        }
        if side > 1 {
            return Err(format!("side must be 0 (P1) or 1 (P2), got {side}"));
        }
        Ok(Self { rom, peer_ip, side })
    }

    pub fn local_port(&self) -> u16 {
        if self.side == 0 {
            7001
        } else {
            7000
        }
    }

    pub fn peer_port(&self) -> u16 {
        if self.side == 0 {
            7000
        } else {
            7001
        }
    }

    pub fn side_label(&self) -> &'static str {
        if self.side == 0 {
            "P1"
        } else {
            "P2"
        }
    }

    pub fn quark_arg(&self) -> String {
        format!(
            "quark:direct,{},{},{},{},{},0",
            self.rom,
            self.local_port(),
            self.peer_ip,
            self.peer_port(),
            self.side
        )
    }
}

#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub envs: Vec<(String, String)>,
}

pub trait Launcher: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn detect(&self) -> Result<InstallInfo, String>;
    fn emulator_dir(&self) -> PathBuf;
    fn spec(&self, config: &MatchConfig) -> Result<LaunchSpec, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_zero_maps_to_p1_ports() {
        let config = MatchConfig::new("sfiii3nr1".into(), "100.64.0.2".into(), 0).unwrap();
        assert_eq!(config.local_port(), 7001);
        assert_eq!(config.peer_port(), 7000);
        assert_eq!(config.side_label(), "P1");
        assert_eq!(
            config.quark_arg(),
            "quark:direct,sfiii3nr1,7001,100.64.0.2,7000,0,0"
        );
    }

    #[test]
    fn side_one_maps_to_p2_ports() {
        let config = MatchConfig::new("sfiii3nr1".into(), "100.64.0.2".into(), 1).unwrap();
        assert_eq!(config.local_port(), 7000);
        assert_eq!(config.peer_port(), 7001);
        assert_eq!(config.side_label(), "P2");
        assert_eq!(
            config.quark_arg(),
            "quark:direct,sfiii3nr1,7000,100.64.0.2,7001,1,0"
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(MatchConfig::new("".into(), "100.64.0.2".into(), 0).is_err());
        assert!(MatchConfig::new("rom".into(), "".into(), 0).is_err());
        assert!(MatchConfig::new("rom".into(), "100.64.0.2".into(), 2).is_err());
    }
}
