use crate::config::{self, Config};
use crate::providers::{self, Provider};
use tauri::{AppHandle, Manager};

pub(crate) fn config_file(app: &AppHandle) -> crate::error::Result<std::path::PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|err| format!("cannot resolve config dir: {err}"))?;
    Ok(config::config_path(dir))
}

pub(crate) fn load_config(app: &AppHandle) -> crate::error::Result<Config> {
    Ok(Config::load(&config_file(app)?))
}

pub(crate) fn provider_for(app: &AppHandle, dev: bool) -> crate::error::Result<Box<dyn Provider>> {
    let cfg = load_config(app)?;
    providers::resolve_provider(app, &cfg, dev)
}

pub(crate) fn config_and_provider(
    app: &AppHandle,
    dev: bool,
) -> crate::error::Result<(Config, Box<dyn Provider>)> {
    let cfg = load_config(app)?;
    let provider = providers::resolve_provider(app, &cfg, dev)?;
    Ok((cfg, provider))
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> crate::error::CommandResult<Config> {
    Ok(load_config(&app)?)
}

#[tauri::command]
pub fn set_config(app: AppHandle, config: Config) -> crate::error::CommandResult<Config> {
    let path = config_file(&app)?;
    config
        .save(&path)
        .map_err(|err| format!("cannot save config: {err}"))?;
    Ok(config)
}
