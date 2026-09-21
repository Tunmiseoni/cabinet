mod core;
mod parity;
mod spec;
#[cfg(test)]
mod test_support;

pub use parity::ParityStatus;

use super::{Capabilities, MatchRequest, Provider, ProviderKind, Role};
use crate::config::Config;
use crate::constants;
use crate::contracts::{InstallInfo, LaunchSpec};
use std::path::{Path, PathBuf};

pub struct RetroArchProvider {
    program: PathBuf,
    core: PathBuf,
    port: u16,
    nickname: String,
    overrides_dir: PathBuf,
    peer_override: Option<String>,
    verbose: bool,
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
        let home = core::home_dir();
        let candidates = core::core_candidates(&program, home.as_deref());
        let core = core::resolve_core(cfg.retroarch_core.as_deref(), &candidates, managed_core);
        let nickname = cfg
            .retroarch_nickname
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                cfg.handle
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or("player")
            .to_string();
        Self {
            program,
            core,
            port: if cfg.retroarch_port == 0 {
                constants::RETROARCH_DEFAULT_PORT
            } else {
                cfg.retroarch_port
            },
            nickname: spec::sanitize_value(&nickname),
            overrides_dir: app_config_dir.join("retroarch"),
            peer_override: dev.then(|| "127.0.0.1".to_string()),
            verbose: cfg.verbose_logging,
        }
    }

    fn peer(&self, fallback: &str) -> String {
        self.peer_override
            .clone()
            .unwrap_or_else(|| fallback.to_string())
    }

    fn write_overrides(&self, role: Role) -> Result<PathBuf, String> {
        spec::write_overrides(self, role)
    }
}

impl Provider for RetroArchProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Retroarch
    }

    fn detect(&self) -> Result<InstallInfo, String> {
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

    fn requires_rom_file(&self) -> bool {
        true
    }

    fn spec(&self, request: &MatchRequest) -> Result<LaunchSpec, String> {
        if !request.rom_path.is_file() {
            return Err(format!("ROM not found: {}", request.rom_path.display()));
        }
        let overrides = self.write_overrides(request.role)?;
        let peer = self.peer(request.peer_ip);
        let args = spec::launch_args(&spec::Args {
            core: &self.core,
            rom_path: request.rom_path,
            overrides: &overrides,
            verbose: self.verbose,
            role: request.role,
            peer: &peer,
            port: self.port,
            nickname: &self.nickname,
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

    fn parity(&self, rom_path: &Path) -> Result<Option<ParityStatus>, String> {
        parity::status(self, rom_path)
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
}
