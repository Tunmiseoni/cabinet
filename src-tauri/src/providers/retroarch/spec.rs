use super::hotkeys;
use super::RetroArchProvider;
use crate::config::RetroArchInput;
use crate::providers::Role;
use std::path::{Path, PathBuf};

pub(super) struct Args<'a> {
    pub core: &'a Path,
    pub rom_path: &'a Path,
    pub overrides: &'a Path,
    pub base_config: Option<&'a Path>,
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
    ];
    if let Some(base) = args.base_config {
        argv.push("-c".to_string());
        argv.push(base.to_string_lossy().to_string());
    }
    argv.push("--appendconfig".to_string());
    argv.push(args.overrides.to_string_lossy().to_string());
    // RetroArch emits the `[Netplay]` lines the observer parses (`Got connection from`,
    // `joined as player N`, `disconnected`) only at verbose verbosity, so this is not optional:
    // without it the lobby's seats and queue never populate. See `netplay.rs`.
    argv.push("--verbose".to_string());
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

/// The 12 preset keys mapped to their FBNeo Classic RetroPad bind suffix.
fn preset_binds(input: &RetroArchInput) -> [(&'static str, &'static str, &str); 12] {
    [
        ("up", "Up", &input.up),
        ("down", "Down", &input.down),
        ("left", "Left", &input.left),
        ("right", "Right", &input.right),
        ("y", "Light Punch", &input.light_punch),
        ("x", "Medium Punch", &input.medium_punch),
        ("l", "Heavy Punch", &input.heavy_punch),
        ("b", "Light Kick", &input.light_kick),
        ("a", "Medium Kick", &input.medium_kick),
        ("r", "Heavy Kick", &input.heavy_kick),
        ("start", "Start", &input.start),
        ("select", "Coin", &input.coin),
    ]
}

/// Write the keyboard preset for a seated instance. RetroArch netplay reads a participant's local
/// input from the *first local device of the matching type* (`get_self_input_state` in
/// `network/netplay/netplay_frontend.c`), not from the player slot it was assigned, so the
/// keyboard always has to be on `input_player1_*`. The seat is independent of the connect
/// direction, so both host and client bind here; a seat-2 instance also keeps the
/// `input_player2_*` binds as a hedge. No seat -> no input (a spectator).
fn write_preset_binds(
    content: &mut String,
    input: &RetroArchInput,
    seat: Option<u8>,
) -> crate::error::Result<()> {
    if seat.is_none() {
        return Ok(());
    }
    let mut prefixes: Vec<&str> = vec!["input_player1"];
    if seat == Some(2) {
        prefixes.push("input_player2");
    }
    for prefix in prefixes {
        for (suffix, label, key) in preset_binds(input) {
            let key = key.trim();
            if !hotkeys::is_valid_key(key) {
                return Err(format!("invalid RetroArch key \"{key}\" for {label}").into());
            }
            content.push_str(&format!(
                "{prefix}_{suffix} = \"{}\"\n",
                sanitize_value(key)
            ));
        }
    }
    Ok(())
}

pub(super) fn write_overrides(
    provider: &RetroArchProvider,
    role: Role,
    seat: Option<u8>,
    nickname: &str,
    start_as_spectator: bool,
) -> crate::error::Result<PathBuf> {
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
    // macOS: pin MoltenVK for the session. RetroArch's Metal driver is broken on
    // Apple Silicon + macOS 26 (~8 fps; libretro/RetroArch#18442), and a
    // menu-selected driver is not persisted (`config_save_on_exit = "false"`), so
    // the host config can hide that RetroArch is running Metal. Linux/Windows keep
    // the user's driver.
    if cfg!(target_os = "macos") {
        content.push_str("video_driver = \"vulkan\"\n");
    }
    content.push_str("pause_nonactive = \"false\"\n");
    content.push_str("netplay_nat_traversal = \"false\"\n");
    content.push_str("netplay_public_announce = \"false\"\n");
    content.push_str("netplay_check_frames = \"600\"\n");
    // Newer RetroArch skips the core-info savestate gate when this is set. Harmless on builds that
    // predate the setting (unknown keys are ignored); the real fix is placing the core where
    // RetroArch scans it. See core::ensure_core_visible.
    content.push_str("core_info_savestate_bypass = \"true\"\n");
    content.push_str("netplay_ping_show = \"true\"\n");
    content.push_str("netplay_allow_slaves = \"true\"\n");
    content.push_str("netplay_require_slaves = \"false\"\n");
    content.push_str("netplay_max_connections = \"8\"\n");
    content.push_str("network_cmd_enable = \"true\"\n");
    content.push_str(&format!(
        "network_cmd_port = \"{}\"\n",
        provider.command_port_for(role)
    ));
    content.push_str("input_libretro_device_p1 = \"5\"\n");
    content.push_str("input_libretro_device_p2 = \"5\"\n");
    // The seat a participant actually takes is chosen by which device it requests; the input
    // bind prefix does not select it. Only the host asks explicitly, so its "play as" choice is
    // honored; a client leaves this unset and RetroArch assigns it the first free device.
    if role == Role::P1 {
        if let Some(host_seat) = seat {
            content.push_str(&format!(
                "netplay_request_device_p1 = \"{}\"\n",
                host_seat == 1
            ));
            content.push_str(&format!(
                "netplay_request_device_p2 = \"{}\"\n",
                host_seat == 2
            ));
        }
    }
    if provider.input_enabled {
        write_preset_binds(&mut content, &provider.input, seat)?;
        let host_cfg = if provider.isolated_config {
            None
        } else {
            provider.host_config.as_deref()
        };
        for config_key in hotkeys::colliding_hotkeys(host_cfg, &provider.input) {
            content.push_str(&format!("{config_key} = \"nul\"\n"));
        }
    }
    content.push_str(&format!("netplay_ip_port = \"{}\"\n", provider.port));
    content.push_str(&format!("netplay_nickname = \"{}\"\n", nickname));
    if provider.max_ping_ms > 0 {
        content.push_str(&format!(
            "netplay_max_ping = \"{}\"\n",
            provider.max_ping_ms
        ));
    }
    content.push_str("savestate_auto_load = \"false\"\n");
    content.push_str(&format!("savefile_directory = \"{}\"\n", saves.display()));
    content.push_str(&format!("savestate_directory = \"{}\"\n", states.display()));
    if role == Role::Spectator {
        content.push_str("netplay_start_as_spectator = \"true\"\n");
        if provider.mute_spectators {
            content.push_str("audio_mute_enable = \"true\"\n");
        }
    } else if start_as_spectator {
        content.push_str("netplay_start_as_spectator = \"true\"\n");
    }

    let path = overrides_path(&provider.overrides_dir, role);
    std::fs::write(&path, &content)
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    log::debug!(target: "retroarch", "appendconfig {}:\n{}", path.display(), content);
    Ok(path)
}

pub(super) fn base_config_path(overrides_dir: &Path) -> PathBuf {
    overrides_dir.join("base.cfg")
}

pub(super) fn write_base_config(
    provider: &RetroArchProvider,
) -> crate::error::Result<Option<PathBuf>> {
    if !provider.isolated_config {
        return Ok(None);
    }
    std::fs::create_dir_all(&provider.overrides_dir)
        .map_err(|err| format!("cannot create {}: {err}", provider.overrides_dir.display()))?;

    let mut content = String::new();
    content.push_str("config_save_on_exit = \"false\"\n");
    content.push_str("savestate_auto_load = \"false\"\n");
    if let Some(dir) = &provider.autoconfig_dir {
        content.push_str(&format!("input_autoconfig_dir = \"{}\"\n", dir.display()));
    }

    let path = base_config_path(&provider.overrides_dir);
    std::fs::write(&path, &content)
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    log::debug!(target: "retroarch", "base config {}:\n{}", path.display(), content);
    Ok(Some(path))
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
        assert!(overrides.contains("core_info_savestate_bypass = \"true\""));
        assert!(overrides.contains("input_libretro_device_p1 = \"5\""));
        assert!(overrides.contains("input_libretro_device_p2 = \"5\""));
        assert!(overrides.contains("savestate_auto_load = \"false\""));
    }

    #[test]
    fn macos_sessions_pin_a_known_good_video_driver() {
        let scratch = Scratch::new("video-driver");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let content = overrides(&provider, Role::P1);
        if cfg!(target_os = "macos") {
            assert!(content.contains("video_driver = \"vulkan\""));
        } else {
            assert!(!content.contains("video_driver"));
        }
    }

    #[test]
    fn a_host_can_start_as_a_non_playing_spectator() {
        let scratch = Scratch::new("host-spectating");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        let mut request = request(Role::P1, &rom, "100.64.0.2");
        request.start_as_spectator = true;
        let spec = provider.spec(&request).unwrap();
        assert!(spec.args.iter().any(|arg| arg == "-H"));
        let content = overrides(&provider, Role::P1);
        assert!(content.contains("netplay_start_as_spectator = \"true\""));
        assert!(!content.contains("audio_mute_enable"));
    }

    #[test]
    fn command_interface_is_enabled_on_a_distinct_per_role_port() {
        let scratch = Scratch::new("command-port");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        for (role, expected) in [
            (Role::P1, 55355),
            (Role::P2, 55356),
            (Role::Spectator, 55357),
        ] {
            provider.spec(&request(role, &rom, "100.64.0.2")).unwrap();
            let content = overrides(&provider, role);
            assert!(content.contains("network_cmd_enable = \"true\""));
            assert!(
                content.contains(&format!("network_cmd_port = \"{expected}\"")),
                "wrong command port for {role:?}: {content}"
            );
        }
    }

    #[test]
    fn always_launches_verbose_so_netplay_is_observable() {
        let scratch = Scratch::new("verbose-on");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        let spec = provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        assert!(spec.args.iter().any(|arg| arg == "--verbose"));
    }

    #[test]
    fn dev_launch_rewrites_the_peer_to_loopback() {
        let scratch = Scratch::new("dev");
        let rom = scratch.rom();
        let mut provider = provider(&scratch);
        provider.peer_override = crate::contracts::PeerOverride::loopback();
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

    #[test]
    fn spectator_audio_is_muted_by_default() {
        let scratch = Scratch::new("mute-on");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        provider
            .spec(&request(Role::Spectator, &rom, "100.64.0.2"))
            .unwrap();
        let overrides =
            std::fs::read_to_string(overrides_path(&provider.overrides_dir, Role::Spectator))
                .unwrap();
        assert!(overrides.contains("audio_mute_enable = \"true\""));
    }

    #[test]
    fn spectator_audio_can_be_left_on() {
        let scratch = Scratch::new("mute-off");
        let rom = scratch.rom();
        let mut provider = provider(&scratch);
        provider.mute_spectators = false;
        provider
            .spec(&request(Role::Spectator, &rom, "100.64.0.2"))
            .unwrap();
        let overrides =
            std::fs::read_to_string(overrides_path(&provider.overrides_dir, Role::Spectator))
                .unwrap();
        assert!(!overrides.contains("audio_mute_enable"));
    }

    #[test]
    fn players_are_never_muted() {
        let scratch = Scratch::new("mute-player");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let overrides =
            std::fs::read_to_string(overrides_path(&provider.overrides_dir, Role::P1)).unwrap();
        assert!(!overrides.contains("audio_mute_enable"));
    }

    #[test]
    fn max_ping_is_emitted_only_when_set() {
        let scratch = Scratch::new("max-ping");
        let rom = scratch.rom();
        let mut provider = provider(&scratch);
        provider.max_ping_ms = 150;
        provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let overrides =
            std::fs::read_to_string(overrides_path(&provider.overrides_dir, Role::P1)).unwrap();
        assert!(overrides.contains("netplay_max_ping = \"150\""));

        provider.max_ping_ms = 0;
        provider
            .spec(&request(Role::P2, &rom, "100.64.0.2"))
            .unwrap();
        let overrides =
            std::fs::read_to_string(overrides_path(&provider.overrides_dir, Role::P2)).unwrap();
        assert!(!overrides.contains("netplay_max_ping"));
    }

    #[test]
    fn isolated_config_adds_a_base_config_with_the_autoconfig_dir() {
        let scratch = Scratch::new("isolated");
        let rom = scratch.rom();
        let autoconfig = scratch.dir.join("user-autoconfig");
        std::fs::create_dir_all(&autoconfig).unwrap();
        let mut provider = provider(&scratch);
        provider.isolated_config = true;
        provider.autoconfig_dir = Some(autoconfig.clone());

        let spec = provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let base = base_config_path(&provider.overrides_dir);
        let flag = spec.args.iter().position(|arg| arg == "-c").unwrap();
        assert_eq!(spec.args[flag + 1], base.to_str().unwrap());
        let append = spec
            .args
            .iter()
            .position(|arg| arg == "--appendconfig")
            .unwrap();
        assert!(flag < append);

        let content = std::fs::read_to_string(&base).unwrap();
        assert!(content.contains(&format!(
            "input_autoconfig_dir = \"{}\"",
            autoconfig.display()
        )));
        assert!(content.contains("config_save_on_exit = \"false\""));
        assert!(content.contains("savestate_auto_load = \"false\""));
    }

    #[test]
    fn isolated_config_is_off_by_default() {
        let scratch = Scratch::new("not-isolated");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        let spec = provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        assert!(!spec.args.iter().any(|arg| arg == "-c"));
        assert!(!base_config_path(&provider.overrides_dir).exists());
    }

    fn overrides(provider: &RetroArchProvider, role: Role) -> String {
        std::fs::read_to_string(overrides_path(&provider.overrides_dir, role)).unwrap()
    }

    #[test]
    fn preset_binds_are_written_for_each_player_role() {
        let scratch = Scratch::new("preset-roles");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let p1 = overrides(&provider, Role::P1);
        assert!(p1.contains("input_player1_up = \"space\""));
        assert!(p1.contains("input_player1_y = \"u\""));
        assert!(p1.contains("input_player1_x = \"i\""));
        assert!(p1.contains("input_player1_l = \"o\""));
        assert!(p1.contains("input_player1_b = \"j\""));
        assert!(p1.contains("input_player1_a = \"k\""));
        assert!(p1.contains("input_player1_r = \"l\""));
        assert!(p1.contains("input_player1_start = \"num1\""));
        assert!(p1.contains("input_player1_select = \"num5\""));
        assert!(!p1.contains("input_player2_"));
        assert!(p1.contains("netplay_request_device_p1 = \"true\""));
        assert!(p1.contains("netplay_request_device_p2 = \"false\""));

        provider
            .spec(&request(Role::P2, &rom, "100.64.0.2"))
            .unwrap();
        let p2 = overrides(&provider, Role::P2);
        assert!(p2.contains("input_player1_y = \"u\""));
        assert!(p2.contains("input_player2_y = \"u\""));
        assert!(!p2.contains("netplay_request_device"));

        provider
            .spec(&request(Role::Spectator, &rom, "100.64.0.2"))
            .unwrap();
        let spectator = overrides(&provider, Role::Spectator);
        assert!(!spectator.contains("input_player1_"));
        assert!(!spectator.contains("input_player2_"));
        assert!(!spectator.contains("netplay_request_device"));
    }

    #[test]
    fn a_client_seated_as_player_one_binds_player_one() {
        let scratch = Scratch::new("seat-client-p1");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        let mut request = request(Role::P2, &rom, "100.64.0.2");
        request.player_slot = Some(1);
        let spec = provider.spec(&request).unwrap();
        let connect = spec.args.iter().position(|arg| arg == "-C").unwrap();
        assert_eq!(spec.args[connect + 1], "100.64.0.2");
        let content = overrides(&provider, Role::P2);
        assert!(content.contains("input_player1_y = \"u\""));
        assert!(!content.contains("input_player2_"));
        assert!(!content.contains("netplay_request_device"));
    }

    #[test]
    fn a_client_seated_as_player_two_binds_both_prefixes_without_requesting_a_device() {
        let scratch = Scratch::new("seat-client-p2");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        let mut request = request(Role::P2, &rom, "100.64.0.2");
        request.player_slot = Some(2);
        provider.spec(&request).unwrap();
        let content = overrides(&provider, Role::P2);
        assert!(content.contains("input_player1_y = \"u\""));
        assert!(content.contains("input_player2_y = \"u\""));
        assert!(!content.contains("netplay_request_device"));
    }

    #[test]
    fn a_host_seated_as_player_two_requests_the_second_device() {
        let scratch = Scratch::new("seat-host-p2");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        let mut request = request(Role::P1, &rom, "100.64.0.2");
        request.player_slot = Some(2);
        let spec = provider.spec(&request).unwrap();
        assert!(spec.args.iter().any(|arg| arg == "-H"));
        let content = overrides(&provider, Role::P1);
        assert!(content.contains("input_player1_y = \"u\""));
        assert!(content.contains("input_player2_y = \"u\""));
        assert!(content.contains("netplay_request_device_p1 = \"false\""));
        assert!(content.contains("netplay_request_device_p2 = \"true\""));
    }

    #[test]
    fn a_host_seated_as_player_one_plays_instead_of_spectating() {
        let scratch = Scratch::new("seat-host-p1");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        let mut request = request(Role::P1, &rom, "100.64.0.2");
        request.player_slot = Some(1);
        let spec = provider.spec(&request).unwrap();
        assert!(spec.args.iter().any(|arg| arg == "-H"));
        let content = overrides(&provider, Role::P1);
        assert!(content.contains("input_player1_y = \"u\""));
        assert!(!content.contains("netplay_start_as_spectator"));
        assert!(content.contains("netplay_request_device_p1 = \"true\""));
        assert!(content.contains("netplay_request_device_p2 = \"false\""));
    }

    #[test]
    fn an_invalid_seat_is_rejected() {
        let scratch = Scratch::new("seat-invalid");
        let rom = scratch.rom();
        let mut request = request(Role::P2, &rom, "100.64.0.2");
        request.player_slot = Some(7);
        assert!(provider(&scratch).spec(&request).is_err());
    }

    #[test]
    fn preset_is_omitted_when_disabled() {
        let scratch = Scratch::new("preset-off");
        let rom = scratch.rom();
        let mut provider = provider(&scratch);
        provider.input_enabled = false;
        provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let content = overrides(&provider, Role::P1);
        assert!(!content.contains("input_player1_y"));
        assert!(!content.contains("input_cheat_toggle = \"nul\""));
        assert!(content.contains("netplay_request_device_p1 = \"true\""));
    }

    #[test]
    fn colliding_hotkeys_are_neutralized_and_others_preserved() {
        let scratch = Scratch::new("preset-hotkeys");
        let rom = scratch.rom();
        let provider = provider(&scratch);
        provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let content = overrides(&provider, Role::P1);
        for key in [
            "input_toggle_fast_forward",
            "input_hold_fast_forward",
            "input_frame_advance",
            "input_cheat_toggle",
            "input_netplay_game_watch",
        ] {
            assert!(
                content.contains(&format!("{key} = \"nul\"")),
                "not neutralized: {key}"
            );
        }
        assert!(!content.contains("input_save_state = \"nul\""));
        assert!(!content.contains("input_screenshot = \"nul\""));
    }

    #[test]
    fn the_host_config_controls_which_hotkeys_are_neutralized() {
        let scratch = Scratch::new("preset-hostcfg");
        let rom = scratch.rom();
        let host_cfg = scratch.dir.join("retroarch.cfg");
        std::fs::write(&host_cfg, "input_toggle_fast_forward = \"backslash\"\n").unwrap();
        let mut provider = provider(&scratch);
        provider.host_config = Some(host_cfg);
        provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let content = overrides(&provider, Role::P1);
        assert!(!content.contains("input_toggle_fast_forward = \"nul\""));
        assert!(content.contains("input_hold_fast_forward = \"nul\""));
    }

    #[test]
    fn isolated_config_ignores_the_host_config_for_hotkeys() {
        let scratch = Scratch::new("preset-isolated-hotkeys");
        let rom = scratch.rom();
        let host_cfg = scratch.dir.join("retroarch.cfg");
        std::fs::write(&host_cfg, "input_toggle_fast_forward = \"backslash\"\n").unwrap();
        let mut provider = provider(&scratch);
        provider.host_config = Some(host_cfg);
        provider.isolated_config = true;
        provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .unwrap();
        let content = overrides(&provider, Role::P1);
        assert!(content.contains("input_toggle_fast_forward = \"nul\""));
    }

    #[test]
    fn an_invalid_preset_key_is_rejected() {
        let scratch = Scratch::new("preset-invalid");
        let rom = scratch.rom();
        let mut provider = provider(&scratch);
        provider.input.heavy_kick = "1".to_string();
        assert!(provider
            .spec(&request(Role::P1, &rom, "100.64.0.2"))
            .is_err());
    }
}
