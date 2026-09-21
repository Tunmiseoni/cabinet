use super::{HostStatus, Permission, PlacementMode, Rect, WindowHost, WindowInfo};

pub struct LinuxWindowHost;

impl WindowHost for LinuxWindowHost {
    fn platform(&self) -> &'static str {
        "linux"
    }

    fn status(&self) -> HostStatus {
        HostStatus {
            platform: self.platform().to_string(),
            supported: false,
            permission: Permission::NotRequired,
            mode: PlacementMode::Unsupported,
            detail:
                "Linux window hosting is not implemented yet; the emulator opens as its own window"
                    .to_string(),
        }
    }

    fn list_windows(&self, _owner_pids: &[i32]) -> crate::error::Result<Vec<WindowInfo>> {
        Ok(Vec::new())
    }

    fn place(&self, _window_id: u32, _target: Rect) -> crate::error::Result<PlacementMode> {
        Err("Linux window hosting is not implemented yet".into())
    }

    fn release(&self, _window_id: u32) -> crate::error::Result<()> {
        Ok(())
    }
}
