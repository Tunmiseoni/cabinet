use crate::logging;
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

mod bundle;
mod redact;

pub(crate) use bundle::diagnostics_text;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsResult {
    pub path: String,
    pub log_dir: String,
}

#[tauri::command]
pub fn log_dir(app: AppHandle) -> crate::error::CommandResult<String> {
    Ok(logging::app_log_dir(&app)?.to_string_lossy().to_string())
}

#[tauri::command]
pub fn open_logs_dir(app: AppHandle) -> crate::error::CommandResult<String> {
    let dir = logging::app_log_dir(&app)?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|err| format!("cannot open {}: {err}", dir.display()))?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command(async)]
pub fn collect_diagnostics(app: AppHandle) -> crate::error::CommandResult<DiagnosticsResult> {
    let dir = logging::app_log_dir(&app)?;
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let path = dir.join(format!("diagnostics-{}.txt", crate::time::utc_stamp()));
    let text = diagnostics_text(&app);
    std::fs::write(&path, text).map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    log::info!("diagnostics written to {}", path.display());
    let _ = app
        .opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>);
    Ok(DiagnosticsResult {
        path: path.to_string_lossy().to_string(),
        log_dir: dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn log_frontend(level: String, message: String, context: Option<String>) {
    use std::str::FromStr;
    let level = log::Level::from_str(&level).unwrap_or(log::Level::Info);
    let suffix = context
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" ({value})"))
        .unwrap_or_default();
    log::log!(target: "webview", level, "{message}{suffix}");
}
