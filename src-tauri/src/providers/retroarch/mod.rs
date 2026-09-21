#[allow(dead_code)]
pub(crate) mod command;
mod core;
mod hotkeys;
mod parity;
mod spec;
#[cfg(test)]
mod test_support;

pub(crate) use core::{download_managed_core, frozen_core_sha256};
pub use hotkeys::Binding;
pub use parity::ParityStatus;

use super::{Capabilities, MatchRequest, Provider, ProviderKind, Role};
use crate::config::{Config, RetroArchInput};
use crate::constants;
use crate::contracts::{InstallInfo, LaunchSpec, PeerOverride};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputMap {
    pub bindings: Vec<Binding>,
    pub defaults: RetroArchInput,
}

pub struct RetroArchProvider {
    program: PathBuf,
    core: PathBuf,
    port: u16,
    command_port: u16,
    nickname: Option<String>,
    handle: Option<String>,
    tailscale_binary: Option<PathBuf>,
    overrides_dir: PathBuf,
    peer_override: PeerOverride,
    verbose: bool,
    mute_spectators: bool,
    max_ping_ms: u32,
    isolated_config: bool,
    autoconfig_dir: Option<PathBuf>,
    host_config: Option<PathBuf>,
    input: RetroArchInput,
    input_enabled: bool,
}

impl RetroArchProvider {
    pub fn new(cfg: &Config, app_config_dir: &Path, app_data_dir: &Path, dev: bool) -> Self {
        let program = cfg
            .retroarch_path
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(core::DEFAULT_PROGRAM));
        let managed_core = core::managed_core_path(app_data_dir);
        let home = crate::env::home_dir();
        let candidates = core::core_candidates(&program, home.as_deref());
        let core = core::resolve_core(cfg.retroarch_core.as_deref(), &candidates, managed_core);
        let nickname = cfg
            .retroarch_nickname
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let handle = cfg
            .handle
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let tailscale_binary = crate::tailscale::resolve_binary(cfg).ok();
        let autoconfig_dir = core::resolve_autoconfig_dir(&program, home.as_deref());
        let host_config = core::resolve_host_config(&program, home.as_deref());
        Self {
            program,
            core,
            port: if cfg.retroarch_port == 0 {
                constants::RETROARCH_DEFAULT_PORT
            } else {
                cfg.retroarch_port
            },
            command_port: if cfg.retroarch_command_port == 0 {
                constants::RETROARCH_DEFAULT_COMMAND_PORT
            } else {
                cfg.retroarch_command_port
            },
            nickname,
            handle,
            tailscale_binary,
            overrides_dir: app_config_dir.join("retroarch"),
            peer_override: if dev {
                PeerOverride::loopback()
            } else {
                PeerOverride::default()
            },
            verbose: cfg.verbose_logging,
            mute_spectators: cfg.retroarch_mute_spectators,
            max_ping_ms: cfg.retroarch_max_ping_ms,
            isolated_config: cfg.retroarch_isolated_config,
            autoconfig_dir,
            host_config,
            input: cfg.retroarch_input.clone(),
            input_enabled: cfg.retroarch_input_enabled,
        }
    }

    fn peer(&self, fallback: &str) -> String {
        self.peer_override.resolve_or(fallback)
    }

    pub(crate) fn command_port_for(&self, role: Role) -> u16 {
        let offset = match role {
            Role::P1 => 0,
            Role::P2 => 1,
            Role::Spectator => 2,
        };
        self.command_port.saturating_add(offset)
    }

    fn tailnet_hostname(&self) -> Option<String> {
        let binary = self.tailscale_binary.as_ref()?;
        let tailnet = crate::tailscale::status(binary).ok()?;
        let hostname = tailnet.self_peer?.hostname;
        let hostname = hostname.trim();
        (!hostname.is_empty()).then(|| hostname.to_string())
    }

    fn resolve_nickname(&self) -> String {
        let explicit = self
            .nickname
            .as_deref()
            .or(self.handle.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let nickname = explicit
            .or_else(|| self.tailnet_hostname())
            .or_else(crate::env::os_hostname)
            .unwrap_or_else(|| "player".to_string());
        spec::sanitize_value(&nickname)
    }

    fn write_overrides(&self, role: Role, nickname: &str) -> crate::error::Result<PathBuf> {
        spec::write_overrides(self, role, nickname)
    }

    pub(crate) fn hotkey_map(&self) -> InputMap {
        let host_cfg = if self.isolated_config {
            None
        } else {
            self.host_config.as_deref()
        };
        let bindings = if self.input_enabled {
            hotkeys::effective_bindings_with_collisions(host_cfg, &self.input)
        } else {
            Vec::new()
        };
        InputMap {
            bindings,
            defaults: RetroArchInput::default(),
        }
    }
}

impl Provider for RetroArchProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Retroarch
    }

    fn detect(&self) -> crate::error::Result<InstallInfo> {
        let program_ok = self.program.is_file() || core::is_on_path(&self.program);
        let core_ok = self.core.is_file();
        let installed = program_ok && core_ok;
        let detail = if installed {
            format!("{} · {}", self.program.display(), self.core.display())
        } else {
            format!(
                "missing {} or {}",
                self.program.display(),
                self.core.display()
            )
        };
        Ok(InstallInfo {
            id: "retroarch".to_string(),
            label: "RetroArch (netplay)".to_string(),
            installed,
            detail,
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            spectate: true,
            dev_pair: true,
        }
    }

    fn port(&self, _role: Role) -> Option<u16> {
        Some(self.port)
    }

    fn command_port(&self, role: Role) -> Option<u16> {
        Some(self.command_port_for(role))
    }

    fn requires_rom_file(&self) -> bool {
        true
    }

    fn spec(&self, request: &MatchRequest) -> crate::error::Result<LaunchSpec> {
        if !request.rom_path.is_file() {
            return Err(format!("ROM not found: {}", request.rom_path.display()).into());
        }
        let nickname = self.resolve_nickname();
        let overrides = self.write_overrides(request.role, &nickname)?;
        let base_config = spec::write_base_config(self)?;
        let peer = self.peer(request.peer_ip);
        let args = spec::launch_args(&spec::Args {
            core: &self.core,
            rom_path: request.rom_path,
            overrides: &overrides,
            base_config: base_config.as_deref(),
            verbose: self.verbose,
            role: request.role,
            peer: &peer,
            port: self.port,
            nickname: &nickname,
        });

        Ok(LaunchSpec {
            program: self.program.clone(),
            args,
            cwd: request
                .rom_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
            envs: Vec::new(),
        })
    }

    fn parity(&self, rom_path: &Path) -> crate::error::Result<Option<ParityStatus>> {
        parity::status(self, rom_path)
    }

    fn input_map(&self) -> Option<InputMap> {
        Some(self.hotkey_map())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::retroarch::test_support::{provider, request, Scratch};

    #[test]
    fn capabilities_match_the_degrade_paths() {
        let scratch = Scratch::new("caps");
        let caps = provider(&scratch).capabilities();
        assert!(caps.spectate);
        assert!(caps.dev_pair);
    }

    #[test]
    fn nickname_prefers_the_explicit_value_and_sanitizes_it() {
        let scratch = Scratch::new("nick-explicit");
        let mut provider = provider(&scratch);
        provider.nickname = Some("ex\"plicit\n".to_string());
        provider.handle = Some("handle".to_string());
        assert_eq!(provider.resolve_nickname(), "explicit");
    }

    #[test]
    fn nickname_falls_back_to_the_handle() {
        let scratch = Scratch::new("nick-handle");
        let mut provider = provider(&scratch);
        provider.nickname = None;
        provider.handle = Some("handle".to_string());
        assert_eq!(provider.resolve_nickname(), "handle");
    }

    #[test]
    fn nickname_is_never_empty() {
        let scratch = Scratch::new("nick-empty");
        let mut provider = provider(&scratch);
        provider.nickname = Some("   ".to_string());
        provider.handle = Some(String::new());
        assert!(!provider.resolve_nickname().is_empty());
    }

    #[test]
    #[ignore = "launches three real RetroArch netplay instances (host/client/spectator) over loopback"]
    fn live_loopback_roles_smoke() {
        use std::time::Duration;

        let core = PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join("Library/Application Support/RetroArch/cores/fbneo_libretro.dylib");
        let rom = PathBuf::from(
            "/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo/ROMs/sfiii3nr1.zip",
        );
        if !core.is_file() || !rom.is_file() {
            eprintln!("skipping: RetroArch core or ROM not present");
            return;
        }

        let scratch = Scratch::new("live");
        let cfg = Config {
            retroarch_core: Some(core.to_string_lossy().to_string()),
            ..Config::default()
        };
        let provider = RetroArchProvider::new(&cfg, &scratch.dir, &scratch.dir, true);

        let mut children = Vec::new();
        for role in [Role::P1, Role::P2, Role::Spectator] {
            let spec = provider.spec(&request(role, &rom, "127.0.0.1")).unwrap();
            let child = crate::process::command(&spec.program)
                .args(&spec.args)
                .current_dir(&spec.cwd)
                .envs(spec.envs.iter().cloned())
                .spawn()
                .expect("spawn retroarch");
            children.push((role, child));
            std::thread::sleep(Duration::from_secs(2));
        }
        std::thread::sleep(Duration::from_secs(8));

        let mut all_alive = true;
        for (role, child) in children.iter_mut() {
            let alive = child.try_wait().unwrap().is_none();
            eprintln!("retroarch {role:?} alive={alive}");
            all_alive &= alive;
        }
        for (_, child) in children.iter_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(all_alive, "all three roles should stay running");
    }

    #[test]
    #[ignore = "launches a real RetroArch host and drives its loopback command socket"]
    fn live_command_socket_smoke() {
        use std::time::Duration;

        let core = PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join("Library/Application Support/RetroArch/cores/fbneo_libretro.dylib");
        let rom = PathBuf::from(
            "/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo/ROMs/sfiii3nr1.zip",
        );
        if !core.is_file() || !rom.is_file() {
            eprintln!("skipping: RetroArch core or ROM not present");
            return;
        }

        let scratch = Scratch::new("live-command");
        let cfg = Config {
            retroarch_core: Some(core.to_string_lossy().to_string()),
            ..Config::default()
        };
        let provider = RetroArchProvider::new(&cfg, &scratch.dir, &scratch.dir, false);
        let port = provider.command_port_for(Role::P1);
        let spec = provider
            .spec(&request(Role::P1, &rom, "127.0.0.1"))
            .unwrap();
        let mut child = crate::process::command(&spec.program)
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(spec.envs.iter().cloned())
            .spawn()
            .expect("spawn retroarch");

        let mut status = None;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(500));
            if let Ok(answer) = command::instance_status(port) {
                status = Some(answer);
                break;
            }
        }

        let read = command::read_core_ram_bytes(port, 0x010D28, 1);
        let snapshot = crate::lobby::results::read_snapshot(port, crate::lobby::results::SFIII3NR1);

        let _ = child.kill();
        let _ = child.wait();

        assert!(
            status.is_some(),
            "command socket on {port} never answered GET_STATUS"
        );
        let read = read.expect("read the round counter over READ_CORE_RAM");
        assert_eq!(read.address, 0x010D28);
        assert_eq!(read.bytes.len(), 1);
        assert!(snapshot.is_ok(), "read a health snapshot: {snapshot:?}");
    }
}
