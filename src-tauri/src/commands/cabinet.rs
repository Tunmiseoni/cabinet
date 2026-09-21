use crate::session;
use crate::windowing::{self, PlacementMode, Rect, WindowInfo};
use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CabinetStatus {
    pub platform: String,
    pub supported: bool,
    pub permission: windowing::Permission,
    pub mode: PlacementMode,
    pub detail: String,
    pub owner_pids: Vec<i32>,
    pub windows: Vec<WindowInfo>,
}

pub(crate) fn session_pids(app: &AppHandle) -> Vec<i32> {
    session::status(app)
        .instances
        .iter()
        .filter_map(|instance| instance.pid.map(|pid| pid as i32))
        .collect()
}

#[tauri::command(async)]
pub fn cabinet_status(app: AppHandle) -> crate::error::CommandResult<CabinetStatus> {
    let host = &app.state::<windowing::Host>().0;
    let status = host.status();
    let owner_pids = session_pids(&app);
    let windows = host.list_windows(&owner_pids).unwrap_or_default();
    log::debug!(
        "cabinet_status: platform={} supported={} permission={:?} mode={:?} pids={:?} windows={}",
        status.platform,
        status.supported,
        status.permission,
        status.mode,
        owner_pids,
        windows.len()
    );
    Ok(CabinetStatus {
        platform: status.platform,
        supported: status.supported,
        permission: status.permission,
        mode: status.mode,
        detail: status.detail,
        owner_pids,
        windows,
    })
}

#[tauri::command(async)]
pub fn cabinet_place(
    app: AppHandle,
    window_id: u32,
    rect: Rect,
) -> crate::error::CommandResult<PlacementMode> {
    let host = &app.state::<windowing::Host>().0;
    let allowed = host.list_windows(&session_pids(&app))?;
    if !allowed.iter().any(|window| window.id == window_id) {
        return Err(format!("window {window_id} is not part of the current match").into());
    }
    Ok(host.place(window_id, rect.round())?)
}

#[tauri::command(async)]
pub fn cabinet_release(app: AppHandle, window_id: u32) -> crate::error::CommandResult<()> {
    Ok(app.state::<windowing::Host>().0.release(window_id)?)
}

#[tauri::command(async)]
pub fn cabinet_request_permission() -> crate::error::CommandResult<bool> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::windowing::macos::prompt_permission())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("no permission is required on this platform".into())
    }
}
