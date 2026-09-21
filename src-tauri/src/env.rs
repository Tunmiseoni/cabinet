use std::path::{Path, PathBuf};

pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub(crate) fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn path_program_on_path(program: &Path) -> bool {
    if program.components().count() > 1 {
        return program.is_file();
    }
    program
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| on_path(name).is_some())
        .unwrap_or(false)
}
