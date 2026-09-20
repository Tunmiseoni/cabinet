use super::{HostStatus, Permission, PlacementMode, Rect, WindowHost, WindowInfo};

pub struct WindowsWindowHost;

impl WindowHost for WindowsWindowHost {
    fn platform(&self) -> &'static str {
        "windows"
    }

    fn status(&self) -> HostStatus {
        HostStatus {
            platform: self.platform().to_string(),
            supported: false,
            permission: Permission::NotRequired,
            mode: PlacementMode::Unsupported,
            detail: "Windows window hosting is not implemented yet; the emulator opens as its own window"
                .to_string(),
        }
    }

    fn list_windows(&self, _owner_pids: &[i32]) -> Result<Vec<WindowInfo>, String> {
        Ok(Vec::new())
    }

    fn place(&self, _window_id: u32, _target: Rect) -> Result<PlacementMode, String> {
        Err("Windows window hosting is not implemented yet".to_string())
    }

    fn release(&self, _window_id: u32) -> Result<(), String> {
        Ok(())
    }
}
