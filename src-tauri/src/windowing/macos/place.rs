use crate::windowing::{Rect, WindowInfo};
use accessibility_sys::{
    kAXErrorSuccess, kAXPositionAttribute, kAXRaiseAction, kAXSizeAttribute, kAXValueTypeCGPoint,
    kAXValueTypeCGSize, kAXWindowsAttribute, AXUIElementCopyAttributeValue,
    AXUIElementCreateApplication, AXUIElementPerformAction, AXUIElementRef,
    AXUIElementSetAttributeValue, AXValueCreate, AXValueGetType, AXValueGetValue, AXValueRef,
};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;
use std::ptr;

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
) -> crate::error::Result<()> {
    let value = AXValueCreate(
        kAXValueTypeCGPoint,
        &point as *const CGPoint as *const c_void,
    );
    if value.is_null() {
        return Err("AXValueCreate(point) failed".into());
    }
    let attribute = CFString::new(attribute);
    let error =
        AXUIElementSetAttributeValue(element, attribute.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
    if error == kAXErrorSuccess {
        Ok(())
    } else {
        Err(format!("AX position set failed: {error}").into())
    }
}

unsafe fn ax_set_size(
    element: AXUIElementRef,
    attribute: &str,
    size: CGSize,
) -> crate::error::Result<()> {
    let value = AXValueCreate(kAXValueTypeCGSize, &size as *const CGSize as *const c_void);
    if value.is_null() {
        return Err("AXValueCreate(size) failed".into());
    }
    let attribute = CFString::new(attribute);
    let error =
        AXUIElementSetAttributeValue(element, attribute.as_concrete_TypeRef(), value as CFTypeRef);
    CFRelease(value as CFTypeRef);
    if error == kAXErrorSuccess {
        Ok(())
    } else {
        Err(format!("AX size set failed: {error}").into())
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

pub(super) unsafe fn apply_position_and_size(
    window: &WindowInfo,
    target: Rect,
    raise: bool,
) -> crate::error::Result<()> {
    let Some(windows) = ax_windows(window.owner_pid) else {
        return Err(format!("no Accessibility windows for process {}", window.owner_pid).into());
    };
    let Some(element) = match_ax_element(&windows, window) else {
        return Err(format!("no Accessibility window matched {:?}", window.title).into());
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
