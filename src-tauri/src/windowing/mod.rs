use serde::{Deserialize, Serialize};

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn is_valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
            && self.width > 1.0
            && self.height > 1.0
    }

    pub fn round(self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
            width: self.width.round(),
            height: self.height.round(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: u32,
    pub owner_pid: i32,
    pub owner_name: String,
    pub title: String,
    pub bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Granted,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Denied,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    NotRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlacementMode {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Placement,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    FrameFollow,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    pub platform: String,
    pub supported: bool,
    pub permission: Permission,
    pub mode: PlacementMode,
    pub detail: String,
}

pub trait WindowHost: Send + Sync {
    fn platform(&self) -> &'static str;
    fn status(&self) -> HostStatus;
    fn list_windows(&self, owner_pids: &[i32]) -> Result<Vec<WindowInfo>, String>;
    fn place(&self, window_id: u32, target: Rect) -> Result<PlacementMode, String>;
    fn release(&self, window_id: u32) -> Result<(), String>;
}

#[cfg_attr(
    any(target_os = "macos", target_os = "windows", target_os = "linux"),
    allow(dead_code)
)]
pub struct UnsupportedWindowHost;

impl WindowHost for UnsupportedWindowHost {
    fn platform(&self) -> &'static str {
        "unsupported"
    }

    fn status(&self) -> HostStatus {
        HostStatus {
            platform: self.platform().to_string(),
            supported: false,
            permission: Permission::NotRequired,
            mode: PlacementMode::Unsupported,
            detail: "this build has no window host for the current platform".to_string(),
        }
    }

    fn list_windows(&self, _owner_pids: &[i32]) -> Result<Vec<WindowInfo>, String> {
        Ok(Vec::new())
    }

    fn place(&self, _window_id: u32, _target: Rect) -> Result<PlacementMode, String> {
        Err("window hosting is not supported on this platform".to_string())
    }

    fn release(&self, _window_id: u32) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub fn resolve_host() -> Box<dyn WindowHost> {
    Box::new(macos::MacosWindowHost::default())
}

#[cfg(target_os = "windows")]
pub fn resolve_host() -> Box<dyn WindowHost> {
    Box::new(windows::WindowsWindowHost)
}

#[cfg(target_os = "linux")]
pub fn resolve_host() -> Box<dyn WindowHost> {
    Box::new(linux::LinuxWindowHost)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn resolve_host() -> Box<dyn WindowHost> {
    Box::new(UnsupportedWindowHost)
}

pub struct Host(pub Box<dyn WindowHost>);

impl Host {
    pub fn new() -> Self {
        Self(resolve_host())
    }
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_degenerate_rects() {
        assert!(!Rect::new(0.0, 0.0, 0.0, 100.0).is_valid());
        assert!(!Rect::new(0.0, 0.0, 100.0, -1.0).is_valid());
        assert!(!Rect::new(f64::NAN, 0.0, 100.0, 100.0).is_valid());
        assert!(!Rect::new(0.0, 0.0, 1.0, 100.0).is_valid());
    }

    #[test]
    fn accepts_and_rounds_a_usable_rect() {
        let rect = Rect::new(10.4, 20.6, 640.5, 480.4);
        assert!(rect.is_valid());
        assert_eq!(rect.round(), Rect::new(10.0, 21.0, 641.0, 480.0));
    }

    #[test]
    fn unsupported_host_reports_unsupported() {
        let host = UnsupportedWindowHost;
        let status = host.status();
        assert!(!status.supported);
        assert_eq!(status.mode, PlacementMode::Unsupported);
        assert_eq!(status.permission, Permission::NotRequired);
        assert!(host.place(1, Rect::new(0.0, 0.0, 100.0, 100.0)).is_err());
    }
}
