use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    pub id: String,
    pub label: String,
    pub installed: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub envs: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct PeerOverride(Option<String>);

impl PeerOverride {
    pub fn loopback() -> Self {
        Self(Some("127.0.0.1".to_string()))
    }

    pub fn resolve_or(&self, fallback: &str) -> String {
        self.0.clone().unwrap_or_else(|| fallback.to_string())
    }
}
