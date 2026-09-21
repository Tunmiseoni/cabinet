use super::{Runner, FLATPAK_APP, FLATPAK_EMULATOR_DIR, FLATPAK_WINE_ENTRY};
use crate::contracts::LaunchSpec;
use std::path::Path;

pub(super) fn shell_single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

pub(super) fn flatpak_spec(data_dir: &Path, quark: &str) -> LaunchSpec {
    let inner = format!(
        ". /app/bin/get-wine-prefix; cd {FLATPAK_EMULATOR_DIR}; \
         export WINEDEBUG=-all; \
         exec {FLATPAK_WINE_ENTRY} fcadefbneo.exe '{}' -w",
        shell_single_quote(quark)
    );
    LaunchSpec {
        program: std::path::PathBuf::from("flatpak"),
        args: vec![
            "run".to_string(),
            "--command=/bin/sh".to_string(),
            FLATPAK_APP.to_string(),
            "-c".to_string(),
            inner,
        ],
        cwd: data_dir.to_path_buf(),
        envs: Vec::new(),
    }
}

pub(super) fn native_spec(fb_dir: &Path, runner: &Runner, quark: &str) -> LaunchSpec {
    match runner {
        Runner::Direct => LaunchSpec {
            program: fb_dir.join("fcadefbneo"),
            args: vec![quark.to_string(), "-w".to_string()],
            cwd: fb_dir.to_path_buf(),
            envs: vec![("WINEDEBUG".to_string(), "-all".to_string())],
        },
        Runner::Wine(wine) => LaunchSpec {
            program: std::path::PathBuf::from(wine),
            args: vec![
                "fcadefbneo.exe".to_string(),
                quark.to_string(),
                "-w".to_string(),
            ],
            cwd: fb_dir.to_path_buf(),
            envs: vec![("WINEDEBUG".to_string(), "-all".to_string())],
        },
    }
}
