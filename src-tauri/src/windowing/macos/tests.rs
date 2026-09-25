use super::*;

#[test]
fn window_host_reports_macos() {
    let host = MacosWindowHost::default();
    assert_eq!(host.platform(), "macos");
    let status = host.status();
    assert!(status.supported);
}

#[test]
fn rejects_invalid_rect_before_touching_the_window() {
    let host = MacosWindowHost::default();
    let result = host.place(1, Rect::new(0.0, 0.0, 0.0, 0.0));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "invalid viewport rect");
}

#[test]
fn lists_no_windows_without_owner_pids() {
    let host = MacosWindowHost::default();
    assert!(host.list_windows(&[]).unwrap().is_empty());
}

#[test]
fn does_not_fall_back_to_unrelated_windows() {
    let host = MacosWindowHost::default();
    let windows = host.list_windows(&[-1]).unwrap();
    assert!(
        windows.is_empty(),
        "a pid with no matching window must not surface unrelated windows"
    );
}

#[test]
fn framing_windows_have_stable_keys() {
    let key = CFString::new("X");
    assert_eq!(key.to_string(), "X");
}

#[test]
#[ignore = "reads the live macOS on-screen window list"]
fn live_lists_windows() {
    let windows = enumerate_windows();
    eprintln!("windowing: found {} on-screen windows", windows.len());
    for window in windows.iter().take(20) {
        eprintln!(
            "windowing: id={} pid={} size={}x{} owner={:?} has_title={}",
            window.id,
            window.owner_pid,
            window.bounds.width.round(),
            window.bounds.height.round(),
            window.owner_name,
            !window.title.is_empty()
        );
    }
    assert!(
        !windows.is_empty(),
        "expected at least one on-screen window"
    );
}

#[test]
#[ignore = "reads the live macOS Accessibility permission state"]
fn live_reports_permission() {
    eprintln!("windowing: accessibility trusted = {}", is_trusted());
}

#[test]
#[ignore = "launches native RetroArch and moves its window"]
fn live_places_retroarch_window() {
    use crate::config::Config;
    use crate::providers::{MatchRequest, Provider, RetroArchProvider, Role};
    use std::path::PathBuf;
    use std::time::Duration;

    let home = std::env::var("HOME").unwrap_or_default();
    let core = PathBuf::from(&home)
        .join("Library/Application Support/RetroArch/cores/fbneo_libretro.dylib");
    let Some(rom) = std::env::var_os("CABINET_TEST_ROM").map(PathBuf::from) else {
        eprintln!("windowing: skipping — set CABINET_TEST_ROM to a local ROM zip");
        return;
    };
    if !core.is_file() || !rom.is_file() {
        eprintln!("windowing: skipping — RetroArch core or ROM not present");
        return;
    }

    let dir = std::env::temp_dir().join(format!("cabinet-retroarch-window-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = Config {
        retroarch_core: Some(core.to_string_lossy().to_string()),
        ..Config::default()
    };
    let provider = RetroArchProvider::new(&cfg, &dir, &dir, false);
    let spec = provider
        .spec(&MatchRequest {
            role: Role::P1,
            player_slot: None,
            rom_path: &rom,
            peer_ip: "127.0.0.1",
            start_as_spectator: false,
        })
        .unwrap();
    eprintln!("windowing: retroarch argv = {:?}", spec.args);

    let mut child = crate::process::command(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .envs(spec.envs.iter().cloned())
        .spawn()
        .expect("spawn retroarch");
    std::thread::sleep(Duration::from_secs(8));

    // A background-launched RetroArch may not become the frontmost app, so
    // its window is not on the active space and CGWindowList omits it. Real
    // launches are focus-stealing; nudge it the same way.
    let _ = crate::process::command("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to set frontmost of process \"RetroArch\" to true",
        ])
        .output();
    // RetroArch opens a small splash window before the game window settles.
    std::thread::sleep(Duration::from_secs(6));

    let mut target: Option<WindowInfo> = None;
    for _ in 0..15 {
        if let Some(found) = enumerate_windows()
            .into_iter()
            .find(|window| window.owner_pid == child.id() as i32)
        {
            target = Some(found);
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }

    let Some(target) = target else {
        for window in enumerate_windows() {
            eprintln!(
                "windowing: candidate id={} pid={} owner={:?}",
                window.id, window.owner_pid, window.owner_name
            );
        }
        let _ = child.kill();
        let _ = child.wait();
        std::fs::remove_dir_all(&dir).ok();
        eprintln!(
            "windowing: no RetroArch window found for pid {}",
            child.id()
        );
        return;
    };

    let host = MacosWindowHost::default();
    eprintln!(
        "windowing: retroarch target id={} owner={:?} bounds={:?}",
        target.id, target.owner_name, target.bounds
    );
    let result = host.place(target.id, Rect::new(120.0, 120.0, 720.0, 480.0));
    eprintln!(
        "windowing: retroarch placing id={} owner={:?} result={result:?}",
        target.id, target.owner_name
    );

    std::thread::sleep(Duration::from_secs(1));
    let after = enumerate_windows()
        .into_iter()
        .find(|window| window.id == target.id);
    eprintln!(
        "windowing: retroarch after = {:?}",
        after.as_ref().map(|window| window.bounds)
    );

    let _ = child.kill();
    let _ = child.wait();
    std::fs::remove_dir_all(&dir).ok();

    let moved = after
        .map(|window| {
            (window.bounds.x - 120.0).abs() < 40.0 && (window.bounds.width - 720.0).abs() < 40.0
        })
        .unwrap_or(false);
    assert!(
        moved,
        "expected the RetroArch window to be moved and resized"
    );
}
