use super::{HostStatus, Permission, PlacementMode, Rect, WindowHost, WindowInfo};
use crate::sync::MutexExt;
use accessibility_sys::{
    kAXTrustedCheckOptionPrompt, AXIsProcessTrusted, AXIsProcessTrustedWithOptions,
};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use std::collections::HashMap;
use std::sync::Mutex;

mod enumerate;
mod place;
#[cfg(test)]
mod tests;

use enumerate::{descendant_pids, enumerate_windows};
use place::apply_position_and_size;

pub const PERMISSION_REQUIRED: &str = "accessibility-permission-required";

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

    fn list_windows(&self, owner_pids: &[i32]) -> crate::error::Result<Vec<WindowInfo>> {
        if owner_pids.is_empty() {
            return Ok(Vec::new());
        }
        let descendants = descendant_pids(owner_pids);
        Ok(enumerate_windows()
            .into_iter()
            .filter(|window| descendants.contains(&window.owner_pid))
            .collect())
    }

    fn place(&self, window_id: u32, target: Rect) -> crate::error::Result<PlacementMode> {
        if !target.is_valid() {
            return Err("invalid viewport rect".into());
        }
        if !is_trusted() {
            return Err(PERMISSION_REQUIRED.into());
        }
        let all = enumerate_windows();
        let window = all
            .iter()
            .find(|window| window.id == window_id)
            .cloned()
            .ok_or_else(|| format!("window {window_id} not found"))?;
        let first = {
            let mut originals = self.originals.lock_or_recover();
            originals.insert(window_id, window.bounds).is_none()
        };
        unsafe {
            apply_position_and_size(&window, target, first)?;
        }
        Ok(PlacementMode::Placement)
    }

    fn release(&self, window_id: u32) -> crate::error::Result<()> {
        let original = self.originals.lock_or_recover().remove(&window_id);
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
