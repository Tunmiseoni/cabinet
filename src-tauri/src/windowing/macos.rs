use super::{HostStatus, Permission, PlacementMode, Rect, WindowHost, WindowInfo};
use accessibility_sys::{
    kAXErrorSuccess, kAXPositionAttribute, kAXRaiseAction, kAXSizeAttribute, kAXTrustedCheckOptionPrompt,
    kAXValueTypeCGPoint, kAXValueTypeCGSize, kAXWindowsAttribute, AXIsProcessTrusted,
    AXIsProcessTrustedWithOptions, AXUIElementCopyAttributeValue, AXUIElementCreateApplication,
    AXUIElementPerformAction, AXUIElementRef, AXUIElementSetAttributeValue, AXValueCreate,
    AXValueGetType, AXValueGetValue, AXValueRef,
};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFRelease, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly,
};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;

pub const PERMISSION_REQUIRED: &str = "accessibility-permission-required";

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[derive(Default)]
pub struct MacosWindowHost {
    originals: Mutex<HashMap<u32, Rect>>,
}

impl WindowHost for MacosWindowHost {
    fn platform(&self) -> &'static str {
        "macos"
    }

    fn status(&self) -> HostStatus {
        let trusted = is_trusted();
        HostStatus {
            platform: self.platform().to_string(),
            supported: true,
            permission: if trusted {
                Permission::Granted
            } else {
                Permission::Denied
            },
            mode: if trusted {
                PlacementMode::Placement
            } else {
                PlacementMode::FrameFollow
            },
            detail: if trusted {
                "Accessibility granted — the emulator window can be placed into the cabinet".to_string()
            } else {
                "Accessibility not granted — the game stays in its own window (grant access to host it here)"
                    .to_string()
            },
        }
    }

    fn list_windows(&self, owner_pids: &[i32]) -> Result<Vec<WindowInfo>, String> {
        if owner_pids.is_empty() {
            return Ok(Vec::new());
        }
        let descendants = descendant_pids(owner_pids);
        Ok(enumerate_windows()
            .into_iter()
            .filter(|window| descendants.contains(&window.owner_pid))
            .collect())
    }

    fn place(&self, window_id: u32, target: Rect) -> Result<PlacementMode, String> {
        if !target.is_valid() {
            return Err("invalid viewport rect".to_string());
        }
        if !is_trusted() {
            return Err(PERMISSION_REQUIRED.to_string());
        }
        let all = enumerate_windows();
        let window = all
            .iter()
            .find(|window| window.id == window_id)
            .cloned()
            .ok_or_else(|| format!("window {window_id} not found"))?;
        let first = {
            let mut originals = self.originals.lock().unwrap_or_else(|e| e.into_inner());
            originals.insert(window_id, window.bounds).is_none()
        };
        unsafe {
            apply_position_and_size(&window, target, first)?;
        }
        Ok(PlacementMode::Placement)
    }

    fn release(&self, window_id: u32) -> Result<(), String> {
        let original = self
            .originals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&window_id);
        let Some(original) = original else {
            return Ok(());
        };
        if !is_trusted() {
            return Ok(());
        }
        let Some(window) = enumerate_windows()
            .into_iter()
            .find(|window| window.id == window_id)
        else {
            return Ok(());
        };
        unsafe {
            apply_position_and_size(&window, original, true)?;
        }
        Ok(())
    }
}

pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn prompt_permission() -> bool {
    let dictionary = CFDictionary::from_CFType_pairs(&[(
        unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) },
        CFBoolean::true_value(),
    )]);
    unsafe { AXIsProcessTrustedWithOptions(dictionary.as_concrete_TypeRef()) }
}

fn number_in(dict: &CFDictionary<CFString, CFType>, name: &str) -> Option<f64> {
    let name = CFString::new(name);
    dict.find(&name)
        .and_then(|value| value.downcast::<CFNumber>())
        .and_then(|number| number.to_f64())
}

fn dict_in(
    dict: &CFDictionary<CFString, CFType>,
    name: &str,
) -> Option<CFDictionary<CFString, CFType>> {
    let name = CFString::new(name);
    let item = dict.find(&name)?;
    let raw = item.as_CFTypeRef();
    if raw.is_null() {
        return None;
    }
    Some(unsafe { CFDictionary::wrap_under_get_rule(raw as CFDictionaryRef) })
}

fn string_in(dict: &CFDictionary<CFString, CFType>, name: &str) -> Option<String> {
    let name = CFString::new(name);
    dict.find(&name)
        .and_then(|value| value.downcast::<CFString>())
        .map(|string| string.to_string())
}

fn window_from_dict(dict: &CFDictionary<CFString, CFType>) -> Option<WindowInfo> {
    let layer = number_in(dict, "kCGWindowLayer").unwrap_or(0.0);
    if layer != 0.0 {
        return None;
    }
    let id = number_in(dict, "kCGWindowNumber")?;
    let owner_pid = number_in(dict, "kCGWindowOwnerPID")?;
    if owner_pid <= 0.0 || id <= 0.0 {
        return None;
    }
    let bounds = dict_in(dict, "kCGWindowBounds")?;
    let rect = Rect::new(
        number_in(&bounds, "X")?,
        number_in(&bounds, "Y")?,
        number_in(&bounds, "Width")?,
        number_in(&bounds, "Height")?,
    );
    if rect.width < 1.0 || rect.height < 1.0 {
        return None;
    }
    Some(WindowInfo {
        id: id as u32,
        owner_pid: owner_pid as i32,
        owner_name: string_in(dict, "kCGWindowOwnerName").unwrap_or_default(),
        title: string_in(dict, "kCGWindowName").unwrap_or_default(),
        bounds: rect,
    })
}

fn enumerate_windows() -> Vec<WindowInfo> {
    let Some(array) = copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID,
    ) else {
        return Vec::new();
    };
    let mut windows = Vec::new();
    for index in 0..array.len() {
        let Some(item) = array.get(index) else {
            continue;
        };
        let raw = *item as CFDictionaryRef;
        if raw.is_null() {
            continue;
        }
        let dict = unsafe { CFDictionary::<CFString, CFType>::wrap_under_get_rule(raw) };
        if let Some(window) = window_from_dict(&dict) {
            windows.push(window);
        }
    }
    windows
}

fn descendant_pids(roots: &[i32]) -> Vec<i32> {
    if roots.is_empty() {
        return Vec::new();
    }
    let Ok(output) = crate::process::command("ps")
        .args(["-axo", "pid=,ppid="])
        .output()
    else {
        return roots.to_vec();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let pairs: Vec<(i32, i32)> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse().ok()?;
            let ppid = parts.next()?.parse().ok()?;
            Some((pid, ppid))
        })
        .collect();

    let mut set = roots.to_vec();
    let mut changed = true;
    while changed {
        changed = false;
        for (pid, ppid) in &pairs {
            if set.contains(ppid) && !set.contains(pid) {
                set.push(*pid);
                changed = true;
            }
        }
    }
    set
}

unsafe fn ax_windows(pid: i32) -> Option<CFArray<*const c_void>> {
    let app = AXUIElementCreateApplication(pid);
    if app.is_null() {
        return None;
    }
    let attribute = CFString::new(kAXWindowsAttribute);
    let mut value: CFTypeRef = ptr::null();
    let error = AXUIElementCopyAttributeValue(app, attribute.as_concrete_TypeRef(), &mut value);
    CFRelease(app as CFTypeRef);
    if error != kAXErrorSuccess || value.is_null() {
        return None;
    }
    Some(CFArray::wrap_under_create_rule(value as CFArrayRef))
}

unsafe fn ax_string(element: AXUIElementRef, attribute: &str) -> Option<String> {
    let mut value: CFTypeRef = ptr::null();
    let attribute = CFString::new(attribute);
    if AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value)
        != kAXErrorSuccess
        || value.is_null()
    {
        return None;
    }
    let string = CFString::wrap_under_create_rule(value as CFStringRef);
    Some(string.to_string())
}

unsafe fn ax_point(element: AXUIElementRef, attribute: &str) -> Option<CGPoint> {
    let mut value: CFTypeRef = ptr::null();
    let attribute = CFString::new(attribute);
    if AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value)
        != kAXErrorSuccess
        || value.is_null()
    {
        return None;
    }
    let mut point = CGPoint::default();
    let is_point = AXValueGetType(value as AXValueRef) == kAXValueTypeCGPoint;
    let ok = is_point
        && AXValueGetValue(
            value as AXValueRef,
            kAXValueTypeCGPoint,
            &mut point as *mut CGPoint as *mut c_void,
        );
    CFRelease(value);
    if ok {
        Some(point)
    } else {
        None
    }
}

unsafe fn ax_size(element: AXUIElementRef, attribute: &str) -> Option<CGSize> {
    let mut value: CFTypeRef = ptr::null();
    let attribute = CFString::new(attribute);
    if AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value)
        != kAXErrorSuccess
        || value.is_null()
    {
        return None;
    }
    let mut size = CGSize::default();
    let is_size = AXValueGetType(value as AXValueRef) == kAXValueTypeCGSize;
    let ok = is_size
        && AXValueGetValue(
            value as AXValueRef,
            kAXValueTypeCGSize,
            &mut size as *mut CGSize as *mut c_void,
        );
    CFRelease(value);
    if ok {
        Some(size)
    } else {
        None
    }
}

unsafe fn ax_set_point(element: AXUIElementRef, attribute: &str, point: CGPoint) -> Result<(), String> {
    let value = AXValueCreate(
        kAXValueTypeCGPoint,
        &point as *const CGPoint as *const c_void,
    );
    if value.is_null() {
        return Err("AXValueCreate(point) failed".to_string());
    }
    let attribute = CFString::new(attribute);
    let error = AXUIElementSetAttributeValue(element, attribute.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
    if error == kAXErrorSuccess {
        Ok(())
    } else {
        Err(format!("AX position set failed: {error}"))
    }
}

unsafe fn ax_set_size(element: AXUIElementRef, attribute: &str, size: CGSize) -> Result<(), String> {
    let value = AXValueCreate(kAXValueTypeCGSize, &size as *const CGSize as *const c_void);
    if value.is_null() {
        return Err("AXValueCreate(size) failed".to_string());
    }
    let attribute = CFString::new(attribute);
    let error = AXUIElementSetAttributeValue(element, attribute.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
    if error == kAXErrorSuccess {
        Ok(())
    } else {
        Err(format!("AX size set failed: {error}"))
    }
}

unsafe fn match_ax_element(
    windows: &CFArray<*const c_void>,
    window: &WindowInfo,
) -> Option<AXUIElementRef> {
    let mut best: Option<AXUIElementRef> = None;
    let mut best_score = 0i32;
    let mut usable: Vec<AXUIElementRef> = Vec::new();
    for index in 0..windows.len() {
        let Some(item) = windows.get(index) else {
            continue;
        };
        let element = *item as AXUIElementRef;
        let mut score = 0i32;
        if !window.title.is_empty() {
            if let Some(title) = ax_string(element, "AXTitle") {
                if title == window.title {
                    score += 10;
                }
            }
        }
        if let Some(point) = ax_point(element, kAXPositionAttribute) {
            if (point.x - window.bounds.x).abs() < 4.0 && (point.y - window.bounds.y).abs() < 4.0 {
                score += 5;
            }
        }
        if let Some(size) = ax_size(element, kAXSizeAttribute) {
            if (size.width - window.bounds.width).abs() < 4.0
                && (size.height - window.bounds.height).abs() < 4.0
            {
                score += 3;
            }
            if size.width < 1.0 || size.height < 1.0 {
                continue;
            }
        }
        usable.push(element);
        if score == 0 {
            continue;
        }
        if score > best_score {
            best_score = score;
            best = Some(element);
        }
    }

    // Window titles are hidden without Screen Recording permission, and some
    // native frontends (RetroArch) briefly report CG/AX bounds that disagree.
    // If the process exposes exactly one real window, that must be the target.
    if best.is_none() && usable.len() == 1 {
        return usable.first().copied();
    }
    best
}

unsafe fn apply_position_and_size(
    window: &WindowInfo,
    target: Rect,
    raise: bool,
) -> Result<(), String> {
    let Some(windows) = ax_windows(window.owner_pid) else {
        return Err(format!(
            "no Accessibility windows for process {}",
            window.owner_pid
        ));
    };
    let Some(element) = match_ax_element(&windows, window) else {
        return Err(format!("no Accessibility window matched {:?}", window.title));
    };
    ax_set_point(
        element,
        kAXPositionAttribute,
        CGPoint {
            x: target.x,
            y: target.y,
        },
    )?;
    ax_set_size(
        element,
        kAXSizeAttribute,
        CGSize {
            width: target.width,
            height: target.height,
        },
    )?;
    if raise {
        let action = CFString::new(kAXRaiseAction);
        AXUIElementPerformAction(element, action.as_concrete_TypeRef());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
        assert_eq!(result.unwrap_err(), "invalid viewport rect");
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
    #[ignore = "launches the FightCade emulator and moves its window (opens Wine windows)"]
    fn live_places_emulator_window() {
        use crate::launcher::macos::{MacosLauncher, DEFAULT_APP_DIR};
        use crate::launcher::{Launcher, MatchConfig};
        use std::path::PathBuf;
        use std::time::Duration;

        let launcher = MacosLauncher::loopback_with(PathBuf::from(DEFAULT_APP_DIR));
        let install = launcher.detect().unwrap();
        if !install.installed {
            eprintln!("windowing: skipping — {}", install.detail);
            return;
        }
        let config = MatchConfig::new("sfiii3nr1".into(), "127.0.0.1".into(), 0).unwrap();
        let spec = launcher.spec(&config).unwrap();
        let mut child = crate::process::command(&spec.program)
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(spec.envs.iter().cloned())
            .spawn()
            .expect("spawn emulator");

        std::thread::sleep(Duration::from_secs(8));

        let descendants = descendant_pids(&[child.id() as i32]);
        eprintln!("windowing: spawned pid={} descendants={descendants:?}", child.id());

        let mut target: Option<WindowInfo> = None;
        for _ in 0..15 {
            let windows = enumerate_windows();
            if let Some(found) = windows
                .iter()
                .find(|window| descendants.contains(&window.owner_pid))
                .or_else(|| {
                    windows.iter().find(|window| {
                        let owner = window.owner_name.to_ascii_lowercase();
                        owner.contains("wine") || owner.contains("fcade") || owner.contains("fbneo")
                    })
                })
                .cloned()
            {
                target = Some(found);
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }

        let Some(target) = target else {
            for window in enumerate_windows() {
                eprintln!(
                    "windowing: candidate id={} pid={} owner={:?} bounds={:?}",
                    window.id, window.owner_pid, window.owner_name, window.bounds
                );
            }
            let _ = child.kill();
            let _ = child.wait();
            eprintln!("windowing: no emulator window found");
            return;
        };

        eprintln!(
            "windowing: placing id={} owner={:?} from {:?}",
            target.id, target.owner_name, target.bounds
        );
        let host = MacosWindowHost::default();
        let result = host.place(target.id, Rect::new(120.0, 120.0, 720.0, 480.0));
        eprintln!("windowing: place result = {result:?}");

        std::thread::sleep(Duration::from_secs(1));
        let after = enumerate_windows()
            .into_iter()
            .find(|window| window.id == target.id);
        eprintln!("windowing: after = {:?}", after.as_ref().map(|w| w.bounds));

        let _ = child.kill();
        let _ = child.wait();

        let moved = after
            .map(|window| {
                (window.bounds.x - 120.0).abs() < 40.0
                    && (window.bounds.width - 720.0).abs() < 40.0
            })
            .unwrap_or(false);
        assert!(moved, "expected the emulator window to be moved and resized");
    }

    #[test]
    #[ignore = "launches native RetroArch and moves its window"]
    fn live_places_retroarch_window() {
        use crate::config::Config;
        use crate::launcher::retroarch::RetroArchProvider;
        use crate::provider::{MatchRequest, Provider, Role};
        use std::path::PathBuf;
        use std::time::Duration;

        let home = std::env::var("HOME").unwrap_or_default();
        let core = PathBuf::from(&home)
            .join("Library/Application Support/RetroArch/cores/fbneo_libretro.dylib");
        let rom = PathBuf::from(
            "/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo/ROMs/sfiii3nr1.zip",
        );
        if !core.is_file() || !rom.is_file() {
            eprintln!("windowing: skipping — RetroArch core or ROM not present");
            return;
        }

        let dir = std::env::temp_dir().join(format!(
            "cabinet-retroarch-window-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = Config {
            retroarch_core: Some(core.to_string_lossy().to_string()),
            ..Config::default()
        };
        let provider = RetroArchProvider::new(&cfg, &dir, &dir, false);
        let spec = provider
            .spec(&MatchRequest {
                role: Role::P1,
                rom: "sfiii3nr1",
                rom_path: &rom,
                peer_ip: "127.0.0.1",
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
            eprintln!("windowing: no RetroArch window found for pid {}", child.id());
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
                (window.bounds.x - 120.0).abs() < 40.0
                    && (window.bounds.width - 720.0).abs() < 40.0
            })
            .unwrap_or(false);
        assert!(moved, "expected the RetroArch window to be moved and resized");
    }
}
