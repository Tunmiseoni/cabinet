use super::{HostStatus, Permission, PlacementMode, Rect, WindowHost, WindowInfo};
use accessibility_sys::{
    kAXErrorSuccess, kAXPositionAttribute, kAXRaiseAction, kAXSizeAttribute,
    kAXTrustedCheckOptionPrompt, kAXValueTypeCGPoint, kAXValueTypeCGSize, kAXWindowsAttribute,
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElementCopyAttributeValue,
    AXUIElementCreateApplication, AXUIElementPerformAction, AXUIElementRef,
    AXUIElementSetAttributeValue, AXValueCreate, AXValueGetType, AXValueGetValue, AXValueRef,
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
                "Accessibility granted — the emulator window can be placed into the cabinet"
                    .to_string()
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

unsafe fn ax_set_point(
    element: AXUIElementRef,
    attribute: &str,
    point: CGPoint,
) -> Result<(), String> {
    let value = AXValueCreate(
        kAXValueTypeCGPoint,
        &point as *const CGPoint as *const c_void,
    );
    if value.is_null() {
        return Err("AXValueCreate(point) failed".to_string());
    }
    let attribute = CFString::new(attribute);
    let error =
        AXUIElementSetAttributeValue(element, attribute.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
    if error == kAXErrorSuccess {
        Ok(())
    } else {
        Err(format!("AX position set failed: {error}"))
    }
}

unsafe fn ax_set_size(
    element: AXUIElementRef,
    attribute: &str,
    size: CGSize,
) -> Result<(), String> {
    let value = AXValueCreate(kAXValueTypeCGSize, &size as *const CGSize as *const c_void);
    if value.is_null() {
        return Err("AXValueCreate(size) failed".to_string());
    }
    let attribute = CFString::new(attribute);
    let error =
        AXUIElementSetAttributeValue(element, attribute.as_concrete_TypeRef(), value as CFTypeRef);
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
        return Err(format!(
            "no Accessibility window matched {:?}",
            window.title
        ));
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
mod tests;
