use crate::config::Config;
use crate::launcher::{InstallInfo, LaunchSpec};
use crate::provider::{Capabilities, MatchRequest, Provider, ProviderKind, Role};
use serde::Serialize;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const DEFAULT_PORT: u16 = 55435;
pub const FROZEN_CORE_GIT: &str = "GIT6bb3167";
pub const FROZEN_ROM_SHA256: &str =
    "4ed142c90fc1a4632d20600d2f5b46caac813422caab62cce3fa9f85ee5cc4dc";

const FROZEN_CORE_SHA256_MACOS: &str =
    "674e76fbe4980b716214e7bb2b5a6e06b9489cc08e7472bcd676b5f1dfcb8488";
#[allow(dead_code)]
const FROZEN_CORE_SHA256_LINUX: &str =
    "a154a08d0f97ff1c66e6ec22a5c209854f31eb2ed7d831b2cb4c0101d48a2448";
#[allow(dead_code)]
const FROZEN_CORE_SHA256_WINDOWS: &str =
    "0a92f3b61dba68b34df0a93afe1b24b98debb3c25dfe63bf21c02034fdfa1179";

#[cfg(target_os = "macos")]
pub const DEFAULT_PROGRAM: &str = "/Applications/RetroArch.app/Contents/MacOS/RetroArch";
#[cfg(target_os = "linux")]
pub const DEFAULT_PROGRAM: &str = "retroarch";
#[cfg(target_os = "windows")]
pub const DEFAULT_PROGRAM: &str = "retroarch.exe";

pub const fn core_file_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "fbneo_libretro.dylib"
    }
    #[cfg(target_os = "linux")]
    {
        "fbneo_libretro.so"
    }
    #[cfg(target_os = "windows")]
    {
        "fbneo_libretro.dll"
    }
}

pub fn platform_tag() -> &'static str {
    #[cfg(target_os = "macos")]
    return if cfg!(target_arch = "aarch64") {
        "macos-arm64"
    } else {
        "macos-x86_64"
    };
    #[cfg(target_os = "linux")]
    return if cfg!(target_arch = "aarch64") {
        "linux-aarch64"
    } else {
        "linux-x86_64"
    };
    #[cfg(target_os = "windows")]
    return "windows-x86_64";
    #[allow(unreachable_code)]
    "unsupported"
}

pub fn frozen_core_sha256() -> &'static str {
    #[cfg(target_os = "macos")]
    return FROZEN_CORE_SHA256_MACOS;
    #[cfg(target_os = "linux")]
    return FROZEN_CORE_SHA256_LINUX;
    #[cfg(target_os = "windows")]
    return FROZEN_CORE_SHA256_WINDOWS;
    #[allow(unreachable_code)]
    ""
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParityStatus {
    pub applicable: bool,
    pub ok: bool,
    pub core_path: String,
    pub core_git: Option<String>,
    pub core_sha256: Option<String>,
    pub expected_git: String,
    pub expected_sha256: String,
    pub rom_path: String,
    pub rom_sha256: Option<String>,
    pub expected_rom_sha256: String,
    pub detail: String,
}

pub struct RetroArchProvider {
    program: PathBuf,
    core: PathBuf,
    port: u16,
    nickname: String,
    overrides_dir: PathBuf,
    peer_override: Option<String>,
}

impl RetroArchProvider {
    pub fn new(cfg: &Config, app_config_dir: &Path, app_data_dir: &Path, dev: bool) -> Self {
        let program = cfg
            .retroarch_path
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_PROGRAM));
        let core = cfg
            .retroarch_core
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                app_data_dir
                    .join("cores")
                    .join(platform_tag())
                    .join(core_file_name())
            });
        let nickname = cfg
            .retroarch_nickname
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| cfg.handle.as_deref().filter(|value| !value.trim().is_empty()))
            .unwrap_or("player")
            .to_string();
        Self {
            program,
            core,
            port: if cfg.retroarch_port == 0 {
                DEFAULT_PORT
            } else {
                cfg.retroarch_port
            },
            nickname: sanitize_value(&nickname),
            overrides_dir: app_config_dir.join("retroarch"),
            peer_override: dev.then(|| "127.0.0.1".to_string()),
        }
    }

    fn peer(&self, fallback: &str) -> String {
        self.peer_override
            .clone()
            .unwrap_or_else(|| fallback.to_string())
    }

    fn overrides_path(&self, role: Role) -> PathBuf {
        self.overrides_dir.join(format!("netplay-{}.cfg", role.key()))
    }

    fn write_overrides(&self, role: Role) -> Result<PathBuf, String> {
        let dir = self.overrides_dir.join(role.key());
        let saves = dir.join("saves");
        let states = dir.join("states");
        std::fs::create_dir_all(&saves)
            .map_err(|err| format!("cannot create {}: {err}", saves.display()))?;
        std::fs::create_dir_all(&states)
            .map_err(|err| format!("cannot create {}: {err}", states.display()))?;

        let mut content = String::new();
        content.push_str("config_save_on_exit = \"false\"\n");
        content.push_str("video_fullscreen = \"false\"\n");
        content.push_str("pause_nonactive = \"false\"\n");
        content.push_str("netplay_nat_traversal = \"false\"\n");
        content.push_str("netplay_public_announce = \"false\"\n");
        content.push_str("netplay_check_frames = \"600\"\n");
        content.push_str("netplay_ping_show = \"true\"\n");
        content.push_str("netplay_allow_slaves = \"true\"\n");
        content.push_str("netplay_require_slaves = \"false\"\n");
        content.push_str("netplay_max_connections = \"8\"\n");
        content.push_str(&format!("netplay_ip_port = \"{}\"\n", self.port));
        content.push_str(&format!("netplay_nickname = \"{}\"\n", self.nickname));
        content.push_str(&format!(
            "savefile_directory = \"{}\"\n",
            saves.display()
        ));
        content.push_str(&format!(
            "savestate_directory = \"{}\"\n",
            states.display()
        ));
        if role == Role::Spectator {
            content.push_str("netplay_start_as_spectator = \"true\"\n");
        }

        let path = self.overrides_path(role);
        std::fs::write(&path, content)
            .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
        Ok(path)
    }
}

impl Provider for RetroArchProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Retroarch
    }

    fn detect(&self) -> Result<InstallInfo, String> {
        let program_ok = self.program.is_file() || is_on_path(&self.program);
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
            overlay_results: false,
            dev_pair: true,
        }
    }

    fn emulator_dir(&self) -> PathBuf {
        self.core
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn port(&self, _role: Role) -> Option<u16> {
        Some(self.port)
    }

    fn spec(&self, request: &MatchRequest) -> Result<LaunchSpec, String> {
        if !request.rom_path.is_file() {
            return Err(format!("ROM not found: {}", request.rom_path.display()));
        }
        let overrides = self.write_overrides(request.role)?;
        let mut args = vec![
            "-L".to_string(),
            self.core.to_string_lossy().to_string(),
            request.rom_path.to_string_lossy().to_string(),
            "--appendconfig".to_string(),
            overrides.to_string_lossy().to_string(),
        ];
        match request.role {
            Role::P1 => args.push("-H".to_string()),
            Role::P2 | Role::Spectator => {
                args.push("-C".to_string());
                args.push(self.peer(request.peer_ip));
            }
        }
        args.push("--port".to_string());
        args.push(self.port.to_string());
        args.push("--nick".to_string());
        args.push(self.nickname.clone());

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
        let expected_git = FROZEN_CORE_GIT.to_string();
        let expected_sha = frozen_core_sha256().to_string();
        let expected_rom = FROZEN_ROM_SHA256.to_string();

        let core_sha = sha256_file(&self.core).ok();
        let core_git = core_git(&self.core).ok().flatten();
        let rom_sha = sha256_file(rom_path).ok();

        let core_ok = core_sha.as_deref() == Some(expected_sha.as_str())
            && core_git.as_deref() == Some(expected_git.as_str());
        let rom_ok = rom_sha.as_deref() == Some(expected_rom.as_str());
        let ok = core_ok && rom_ok;

        let detail = if ok {
            "PARITY OK (core revision + sha256, ROM sha256)".to_string()
        } else {
            let mut parts = Vec::new();
            if core_sha.is_none() {
                parts.push(format!("core not readable: {}", self.core.display()));
            } else if !core_ok {
                parts.push("core revision or sha256 differs from the frozen set".to_string());
            }
            if rom_sha.is_none() {
                parts.push(format!("ROM not readable: {}", rom_path.display()));
            } else if !rom_ok {
                parts.push("ROM sha256 differs from the frozen reference".to_string());
            }
            format!("PARITY MISMATCH — {}", parts.join("; "))
        };

        Ok(Some(ParityStatus {
            applicable: true,
            ok,
            core_path: self.core.to_string_lossy().to_string(),
            core_git,
            core_sha256: core_sha,
            expected_git,
            expected_sha256: expected_sha,
            rom_path: rom_path.to_string_lossy().to_string(),
            rom_sha256: rom_sha,
            expected_rom_sha256: expected_rom,
            detail,
        }))
    }
}

fn is_on_path(program: &Path) -> bool {
    if program.components().count() > 1 {
        return program.is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
}

fn sanitize_value(value: &str) -> String {
    value.replace(['"', '\n', '\r'], "")
}

fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let mut file = std::fs::File::open(path).map_err(|err| err.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn core_git(path: &Path) -> Result<Option<String>, String> {
    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        if &bytes[index..index + 3] == b"GIT" {
            let mut end = index + 3;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            if end > index + 3 && end - index <= 40 {
                found.insert(String::from_utf8_lossy(&bytes[index..end]).to_string());
            }
            index = end;
        } else {
            index += 1;
        }
    }
    Ok(found.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch {
        dir: PathBuf,
    }

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cabinet-retroarch-{}-{}-{tag}",
                std::process::id(),
                unique()
            ));
            std::fs::create_dir_all(&dir).expect("create scratch dir");
            Self { dir }
        }

        fn rom(&self) -> PathBuf {
            let path = self.dir.join("sfiii3nr1.zip");
            std::fs::write(&path, b"rom-bytes").expect("write rom");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    fn unique() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    }

    fn provider(scratch: &Scratch) -> RetroArchProvider {
        RetroArchProvider {
            program: PathBuf::from(DEFAULT_PROGRAM),
            core: PathBuf::from("/app/cores/fbneo_libretro.dylib"),
            port: 55435,
            nickname: "spike".to_string(),
            overrides_dir: scratch.dir.join("cfg"),
            peer_override: None,
        }
    }

    fn request<'a>(role: Role, rom: &'a Path, peer: &'a str) -> MatchRequest<'a> {
        MatchRequest {
            role,
            rom: "sfiii3nr1",
            rom_path: rom,
            peer_ip: peer,
        }
    }

    #[test]
    fn p1_spec_hosts_on_loopback_port() {
        let scratch = Scratch::new("p1");
        let rom = scratch.rom();
        let spec = provider(&scratch)
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        assert_eq!(spec.program, PathBuf::from(DEFAULT_PROGRAM));
        assert_eq!(
            spec.args,
            vec![
                "-L",
                "/app/cores/fbneo_libretro.dylib",
                rom.to_str().unwrap(),
                "--appendconfig",
                scratch
                    .dir
                    .join("cfg/netplay-p1.cfg")
                    .to_str()
                    .unwrap(),
                "-H",
                "--port",
                "55435",
                "--nick",
                "spike",
            ]
        );
    }

    #[test]
    fn p2_spec_connects_to_the_peer() {
        let scratch = Scratch::new("p2");
        let rom = scratch.rom();
        let spec = provider(&scratch)
            .spec(&request(Role::P2, &rom, "100.64.0.2"))
            .unwrap();
        assert_eq!(
            spec.args,
            vec![
                "-L",
                "/app/cores/fbneo_libretro.dylib",
                rom.to_str().unwrap(),
                "--appendconfig",
                scratch
                    .dir
                    .join("cfg/netplay-p2.cfg")
                    .to_str()
                    .unwrap(),
                "-C",
                "100.64.0.2",
                "--port",
                "55435",
                "--nick",
                "spike",
            ]
        );
    }

    #[test]
    fn spectator_spec_connects_and_sets_the_spectator_flag() {
        let scratch = Scratch::new("spec");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        let spec = provider
            .spec(&request(Role::Spectator, &rom, "100.64.0.2"))
            .unwrap();
        assert_eq!(spec.args[5], "-C");
        assert_eq!(spec.args[6], "100.64.0.2");
        let overrides = std::fs::read_to_string(provider.overrides_path(Role::Spectator)).unwrap();
        assert!(overrides.contains("netplay_start_as_spectator = \"true\""));
        provider
            .spec(&request(Role::P2, &rom, "100.64.0.2"))
            .unwrap();
        let player = std::fs::read_to_string(provider.overrides_path(Role::P2)).unwrap();
        assert!(!player.contains("netplay_start_as_spectator"));
    }

    #[test]
    fn overrides_disable_pause_and_never_touch_the_user_config() {
        let scratch = Scratch::new("overrides");
        let rom = scratch.rom();
        provider(&scratch)
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let overrides = std::fs::read_to_string(scratch.dir.join("cfg/netplay-p1.cfg")).unwrap();
        assert!(overrides.contains("pause_nonactive = \"false\""));
        assert!(overrides.contains("config_save_on_exit = \"false\""));
        assert!(overrides.contains("netplay_nat_traversal = \"false\""));
    }

    #[test]
    fn dev_launch_rewrites_the_peer_to_loopback() {
        let scratch = Scratch::new("dev");
        let rom = scratch.rom();
        let mut provider = provider(&scratch);
        provider.peer_override = Some("127.0.0.1".to_string());
        let spec = provider
            .spec(&request(Role::P2, &rom, "100.64.0.2"))
            .unwrap();
        assert_eq!(spec.args[6], "127.0.0.1");
    }

    #[test]
    fn missing_rom_fails_before_launching() {
        let scratch = Scratch::new("missing");
        let rom = scratch.dir.join("nope.zip");
        assert!(provider(&scratch)
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .is_err());
    }

    #[test]
    fn capabilities_match_the_degrade_paths() {
        let scratch = Scratch::new("caps");
        let caps = provider(&scratch).capabilities();
        assert!(caps.spectate);
        assert!(!caps.overlay_results);
        assert!(caps.dev_pair);
    }

    #[test]
    fn parity_detects_mismatch_against_the_frozen_set() {
        let scratch = Scratch::new("parity");
        let rom = scratch.rom();
        let status = provider(&scratch)
            .parity(&rom)
            .unwrap()
            .expect("retroarch has a parity gate");
        assert!(!status.ok);
        assert!(status.detail.starts_with("PARITY MISMATCH"));
        assert_eq!(status.expected_git, FROZEN_CORE_GIT);
        assert_eq!(status.expected_rom_sha256, FROZEN_ROM_SHA256);
    }

    #[test]
    fn reads_the_core_git_revision() {
        let scratch = Scratch::new("git");
        let core = scratch.dir.join("core.bin");
        std::fs::write(&core, b"....GIT6bb3167....extra GIT deadbeef").unwrap();
        assert_eq!(core_git(&core).unwrap().as_deref(), Some("GIT6bb3167"));
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
