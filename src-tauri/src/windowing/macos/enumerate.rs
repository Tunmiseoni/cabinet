use crate::windowing::{Rect, WindowInfo};
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly,
};

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

pub(super) fn enumerate_windows() -> Vec<WindowInfo> {
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

pub(super) fn descendant_pids(roots: &[i32]) -> Vec<i32> {
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
