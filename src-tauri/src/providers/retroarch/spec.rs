use super::RetroArchProvider;
use crate::providers::Role;
use std::path::{Path, PathBuf};

pub(super) struct Args<'a> {
    pub core: &'a Path,
    pub rom_path: &'a Path,
    pub overrides: &'a Path,
    pub verbose: bool,
    pub role: Role,
    pub peer: &'a str,
    pub port: u16,
    pub nickname: &'a str,
}

pub(super) fn launch_args(args: &Args) -> Vec<String> {
    let mut argv = vec![
        "-L".to_string(),
        args.core.to_string_lossy().to_string(),
        args.rom_path.to_string_lossy().to_string(),
        "--appendconfig".to_string(),
        args.overrides.to_string_lossy().to_string(),
    ];
    if args.verbose {
        argv.push("--verbose".to_string());
    }
    match args.role {
        Role::P1 => argv.push("-H".to_string()),
        Role::P2 | Role::Spectator => {
            argv.push("-C".to_string());
            argv.push(args.peer.to_string());
        }
    }
    argv.push("--port".to_string());
    argv.push(args.port.to_string());
    argv.push("--nick".to_string());
    argv.push(args.nickname.to_string());
    argv
}

pub(super) fn overrides_path(overrides_dir: &Path, role: Role) -> PathBuf {
    overrides_dir.join(format!("netplay-{}.cfg", role.key()))
}

pub(super) fn write_overrides(provider: &RetroArchProvider, role: Role) -> Result<PathBuf, String> {
    let dir = provider.overrides_dir.join(role.key());
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
    content.push_str(&format!("netplay_ip_port = \"{}\"\n", provider.port));
    content.push_str(&format!("netplay_nickname = \"{}\"\n", provider.nickname));
    content.push_str(&format!("savefile_directory = \"{}\"\n", saves.display()));
    content.push_str(&format!("savestate_directory = \"{}\"\n", states.display()));
    if role == Role::Spectator {
        content.push_str("netplay_start_as_spectator = \"true\"\n");
    }

    let path = overrides_path(&provider.overrides_dir, role);
    std::fs::write(&path, &content)
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    log::debug!(target: "retroarch", "appendconfig {}:\n{}", path.display(), content);
    Ok(path)
}

pub(super) fn sanitize_value(value: &str) -> String {
    value.replace(['"', '\n', '\r'], "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::retroarch::core::DEFAULT_PROGRAM;
    use crate::providers::retroarch::test_support::{provider, request, Scratch};
    use crate::providers::Provider;

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
                    .join("cfg")
                    .join("netplay-p1.cfg")
                    .to_str()
                    .unwrap(),
                "--verbose",
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
                    .join("cfg")
                    .join("netplay-p2.cfg")
                    .to_str()
                    .unwrap(),
                "--verbose",
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
        let connect = spec.args.iter().position(|arg| arg == "-C").unwrap();
        assert_eq!(spec.args[connect + 1], "100.64.0.2");
        let overrides =
            std::fs::read_to_string(overrides_path(&provider.overrides_dir, Role::Spectator))
                .unwrap();
        assert!(overrides.contains("netplay_start_as_spectator = \"true\""));
        provider
            .spec(&request(Role::P2, &rom, "100.64.0.2"))
            .unwrap();
        let player =
            std::fs::read_to_string(overrides_path(&provider.overrides_dir, Role::P2)).unwrap();
        assert!(!player.contains("netplay_start_as_spectator"));
    }

    #[test]
    fn overrides_disable_pause_and_never_touch_the_user_config() {
        let scratch = Scratch::new("overrides");
        let rom = scratch.rom();
        provider(&scratch)
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let overrides =
            std::fs::read_to_string(scratch.dir.join("cfg").join("netplay-p1.cfg")).unwrap();
        assert!(overrides.contains("pause_nonactive = \"false\""));
        assert!(overrides.contains("config_save_on_exit = \"false\""));
        assert!(overrides.contains("netplay_nat_traversal = \"false\""));
    }

    #[test]
    fn verbose_flag_is_gated_on_the_setting() {
        let scratch = Scratch::new("verbose-off");
        let rom = scratch.rom();
        let mut provider = provider(&scratch);
        provider.verbose = false;
        let spec = provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        assert!(!spec.args.iter().any(|arg| arg == "--verbose"));
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
        let connect = spec.args.iter().position(|arg| arg == "-C").unwrap();
        assert_eq!(spec.args[connect + 1], "127.0.0.1");
    }

    #[test]
    fn missing_rom_fails_before_launching() {
        let scratch = Scratch::new("missing");
        let rom = scratch.dir.join("nope.zip");
        assert!(provider(&scratch)
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .is_err());
    }
}
