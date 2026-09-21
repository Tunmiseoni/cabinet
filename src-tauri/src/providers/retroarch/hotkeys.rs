use crate::config::RetroArchInput;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub action: String,
    pub config_key: String,
    pub key: String,
    pub collides: bool,
}

struct Hotkey {
    config_key: &'static str,
    action: &'static str,
    default_key: &'static str,
}

// Curated table of RetroArch's keyboard hotkeys and their upstream defaults. Only keys
// that RetroArch 1.22.2 actually supports are listed; unknown keys are never disabled.
const DEFAULT_HOTKEYS: &[Hotkey] = &[
    Hotkey {
        config_key: "input_menu_toggle",
        action: "Toggle menu",
        default_key: "f1",
    },
    Hotkey {
        config_key: "input_exit_emulator",
        action: "Exit",
        default_key: "escape",
    },
    Hotkey {
        config_key: "input_toggle_fast_forward",
        action: "Toggle fast-forward",
        default_key: "space",
    },
    Hotkey {
        config_key: "input_hold_fast_forward",
        action: "Hold fast-forward",
        default_key: "l",
    },
    Hotkey {
        config_key: "input_hold_slowmotion",
        action: "Hold slow motion",
        default_key: "e",
    },
    Hotkey {
        config_key: "input_rewind",
        action: "Rewind",
        default_key: "r",
    },
    Hotkey {
        config_key: "input_save_state",
        action: "Save state",
        default_key: "f2",
    },
    Hotkey {
        config_key: "input_load_state",
        action: "Load state",
        default_key: "f4",
    },
    Hotkey {
        config_key: "input_state_slot_increase",
        action: "Next state slot",
        default_key: "f7",
    },
    Hotkey {
        config_key: "input_state_slot_decrease",
        action: "Previous state slot",
        default_key: "f6",
    },
    Hotkey {
        config_key: "input_screenshot",
        action: "Screenshot",
        default_key: "f8",
    },
    Hotkey {
        config_key: "input_audio_mute",
        action: "Mute audio",
        default_key: "f9",
    },
    Hotkey {
        config_key: "input_toggle_fullscreen",
        action: "Toggle fullscreen",
        default_key: "f",
    },
    Hotkey {
        config_key: "input_grab_mouse_toggle",
        action: "Toggle mouse grab",
        default_key: "f11",
    },
    Hotkey {
        config_key: "input_desktop_menu_toggle",
        action: "Toggle desktop menu",
        default_key: "f5",
    },
    Hotkey {
        config_key: "input_fps_toggle",
        action: "Toggle FPS display",
        default_key: "f3",
    },
    Hotkey {
        config_key: "input_reset",
        action: "Reset",
        default_key: "h",
    },
    Hotkey {
        config_key: "input_pause_toggle",
        action: "Pause",
        default_key: "p",
    },
    Hotkey {
        config_key: "input_frame_advance",
        action: "Frame advance",
        default_key: "k",
    },
    Hotkey {
        config_key: "input_cheat_toggle",
        action: "Toggle cheats",
        default_key: "u",
    },
    Hotkey {
        config_key: "input_cheat_index_plus",
        action: "Next cheat",
        default_key: "y",
    },
    Hotkey {
        config_key: "input_cheat_index_minus",
        action: "Previous cheat",
        default_key: "t",
    },
    Hotkey {
        config_key: "input_shader_next",
        action: "Next shader",
        default_key: "m",
    },
    Hotkey {
        config_key: "input_shader_prev",
        action: "Previous shader",
        default_key: "n",
    },
    Hotkey {
        config_key: "input_shader_toggle",
        action: "Toggle shader",
        default_key: "comma",
    },
    Hotkey {
        config_key: "input_volume_up",
        action: "Volume up",
        default_key: "add",
    },
    Hotkey {
        config_key: "input_volume_down",
        action: "Volume down",
        default_key: "subtract",
    },
    Hotkey {
        config_key: "input_netplay_game_watch",
        action: "Spectate / play",
        default_key: "i",
    },
    Hotkey {
        config_key: "input_netplay_player_chat",
        action: "Netplay chat",
        default_key: "tilde",
    },
    Hotkey {
        config_key: "input_toggle_statistics",
        action: "Toggle statistics",
        default_key: "nul",
    },
    Hotkey {
        config_key: "input_overlay_next",
        action: "Next overlay",
        default_key: "nul",
    },
    Hotkey {
        config_key: "input_disk_eject_toggle",
        action: "Eject disk",
        default_key: "nul",
    },
    Hotkey {
        config_key: "input_disk_next",
        action: "Next disk",
        default_key: "nul",
    },
];

// Key names RetroArch accepts in a config (`input_config_translate_str_to_rk`). Single
// ASCII letters are accepted implicitly and are not listed here.
const KEY_NAMES: &[&str] = &[
    "left",
    "right",
    "up",
    "down",
    "enter",
    "kp_enter",
    "tab",
    "insert",
    "del",
    "end",
    "home",
    "rshift",
    "shift",
    "ctrl",
    "alt",
    "space",
    "escape",
    "add",
    "subtract",
    "kp_plus",
    "kp_minus",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
    "f13",
    "f14",
    "f15",
    "num0",
    "num1",
    "num2",
    "num3",
    "num4",
    "num5",
    "num6",
    "num7",
    "num8",
    "num9",
    "pageup",
    "pagedown",
    "keypad0",
    "keypad1",
    "keypad2",
    "keypad3",
    "keypad4",
    "keypad5",
    "keypad6",
    "keypad7",
    "keypad8",
    "keypad9",
    "period",
    "capslock",
    "numlock",
    "backspace",
    "multiply",
    "divide",
    "print_screen",
    "scroll_lock",
    "tilde",
    "backquote",
    "pause",
    "quote",
    "comma",
    "minus",
    "slash",
    "semicolon",
    "equals",
    "leftbracket",
    "backslash",
    "rightbracket",
    "kp_period",
    "kp_equals",
    "rctrl",
    "ralt",
    "caret",
    "underscore",
    "exclaim",
    "quotedbl",
    "hash",
    "dollar",
    "ampersand",
    "leftparen",
    "rightparen",
    "asterisk",
    "plus",
    "colon",
    "less",
    "greater",
    "question",
    "at",
    "rmeta",
    "lmeta",
    "lsuper",
    "rsuper",
    "mode",
    "compose",
    "help",
    "sysreq",
    "break",
    "menu",
    "power",
    "euro",
    "undo",
    "clear",
    "oem102",
    "back",
    "forward",
    "refresh",
    "bstop",
    "search",
    "favorites",
    "homepage",
    "mute",
    "volumedown",
    "volumeup",
    "next",
    "prev",
    "stop",
    "play",
    "email",
    "media",
    "app1",
    "app2",
    "nul",
];

fn is_disabled(key: &str) -> bool {
    let key = key.trim();
    key.is_empty() || key.eq_ignore_ascii_case("nul")
}

pub(crate) fn is_valid_key(key: &str) -> bool {
    let key = key.trim();
    if key.is_empty() {
        return false;
    }
    if key.len() == 1 && key.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return true;
    }
    KEY_NAMES.iter().any(|name| name.eq_ignore_ascii_case(key))
}

fn parse_host_config(path: &Path) -> Option<HashMap<String, String>> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut map = HashMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !DEFAULT_HOTKEYS
            .iter()
            .any(|hotkey| hotkey.config_key == key)
        {
            continue;
        }
        let value = value.trim().trim_matches('"').trim();
        map.insert(key.to_string(), value.to_string());
    }
    Some(map)
}

/// The effective hotkey bindings for this machine: curated defaults, overridden by the
/// host `retroarch.cfg` (read-only) when one is available. Disabled (`nul`) binds are
/// omitted.
pub(crate) fn effective_bindings(host_cfg: Option<&Path>) -> Vec<Binding> {
    let overrides = host_cfg.and_then(parse_host_config).unwrap_or_default();
    DEFAULT_HOTKEYS
        .iter()
        .filter_map(|hotkey| {
            let key = overrides
                .get(hotkey.config_key)
                .map(String::as_str)
                .unwrap_or(hotkey.default_key);
            if is_disabled(key) {
                return None;
            }
            Some(Binding {
                action: hotkey.action.to_string(),
                config_key: hotkey.config_key.to_string(),
                key: key.trim().to_ascii_lowercase(),
                collides: false,
            })
        })
        .collect()
}

pub(crate) fn effective_bindings_with_collisions(
    host_cfg: Option<&Path>,
    input: &RetroArchInput,
) -> Vec<Binding> {
    let preset: Vec<String> = input
        .key_values()
        .iter()
        .map(|key| key.trim().to_ascii_lowercase())
        .collect();
    let mut bindings = effective_bindings(host_cfg);
    for binding in &mut bindings {
        binding.collides = preset
            .iter()
            .any(|key| key.eq_ignore_ascii_case(&binding.key));
    }
    bindings
}

/// The `input_*` config keys whose effective bind is one of the preset's keys, and so
/// must be neutralized for a Cabinet session.
pub(crate) fn colliding_hotkeys(host_cfg: Option<&Path>, input: &RetroArchInput) -> Vec<String> {
    effective_bindings_with_collisions(host_cfg, input)
        .into_iter()
        .filter(|binding| binding.collides)
        .map(|binding| binding.config_key)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::retroarch::test_support::Scratch;

    fn write_cfg(scratch: &Scratch, body: &str) -> std::path::PathBuf {
        let path = scratch.dir.join("retroarch.cfg");
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn defaults_collide_with_the_preset_keys() {
        let keys = colliding_hotkeys(None, &RetroArchInput::default());
        for expected in [
            "input_toggle_fast_forward",
            "input_hold_fast_forward",
            "input_frame_advance",
            "input_cheat_toggle",
            "input_netplay_game_watch",
        ] {
            assert!(
                keys.contains(&expected.to_string()),
                "missing {expected}: {keys:?}"
            );
        }
    }

    #[test]
    fn the_host_config_overrides_a_default_hotkey() {
        let scratch = Scratch::new("hotkeys-override");
        let path = write_cfg(&scratch, "input_toggle_fast_forward = \"backslash\"\n");
        let keys = colliding_hotkeys(Some(&path), &RetroArchInput::default());
        assert!(!keys.contains(&"input_toggle_fast_forward".to_string()));
        assert!(keys.contains(&"input_hold_fast_forward".to_string()));
    }

    #[test]
    fn a_nul_bind_disables_a_hotkey() {
        let scratch = Scratch::new("hotkeys-nul");
        let path = write_cfg(&scratch, "input_cheat_toggle = \"nul\"\n");
        let keys = colliding_hotkeys(Some(&path), &RetroArchInput::default());
        assert!(!keys.contains(&"input_cheat_toggle".to_string()));
    }

    #[test]
    fn bindings_carry_the_action_and_the_collision_flag() {
        let scratch = Scratch::new("hotkeys-bindings");
        let path = write_cfg(&scratch, "input_frame_advance = \"nul\"\n");
        let bindings = effective_bindings_with_collisions(Some(&path), &RetroArchInput::default());
        assert!(!bindings
            .iter()
            .any(|binding| binding.config_key == "input_frame_advance"));
        let fast_forward = bindings
            .iter()
            .find(|binding| binding.config_key == "input_toggle_fast_forward")
            .unwrap();
        assert!(fast_forward.collides);
        assert_eq!(fast_forward.action, "Toggle fast-forward");
        assert_eq!(fast_forward.key, "space");
    }

    #[test]
    fn key_validation_matches_the_retroarch_key_names() {
        assert!(is_valid_key("u"));
        assert!(is_valid_key("Z"));
        assert!(is_valid_key("space"));
        assert!(is_valid_key("num1"));
        assert!(is_valid_key("backslash"));
        assert!(is_valid_key("nul"));
        assert!(is_valid_key("f3"));
        assert!(!is_valid_key(""));
        assert!(!is_valid_key("1"));
        assert!(!is_valid_key("foo"));
    }
}
