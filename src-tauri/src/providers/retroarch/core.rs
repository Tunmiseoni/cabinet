use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
pub(super) const DEFAULT_PROGRAM: &str = "/Applications/RetroArch.app/Contents/MacOS/RetroArch";
#[cfg(target_os = "linux")]
pub(super) const DEFAULT_PROGRAM: &str = "retroarch";
#[cfg(target_os = "windows")]
pub(super) const DEFAULT_PROGRAM: &str = "retroarch.exe";

#[cfg(target_os = "macos")]
const FROZEN_CORE_SHA256: &str = "6472c6312fe6ad49a8001efabbdc4d2b542848a7c964d0a082cd736e01e8a4eb";
#[cfg(target_os = "linux")]
const FROZEN_CORE_SHA256: &str = "a154a08d0f97ff1c66e6ec22a5c209854f31eb2ed7d831b2cb4c0101d48a2448";
#[cfg(target_os = "windows")]
const FROZEN_CORE_SHA256: &str = "0a92f3b61dba68b34df0a93afe1b24b98debb3c25dfe63bf21c02034fdfa1179";

pub(super) const fn core_file_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "fbneo_libretro.dylib"
    }
    #[cfg(target_os = "linux")]
    {
        "fbneo_libretro.so"
    }
    #[cfg(target_os = "windows")]
    {
        "fbneo_libretro.dll"
    }
}

pub(super) fn platform_tag() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        if cfg!(target_arch = "aarch64") {
            "macos-arm64"
        } else {
            "macos-x86_64"
        }
    }
    #[cfg(target_os = "linux")]
    {
        if cfg!(target_arch = "aarch64") {
            "linux-aarch64"
        } else {
            "linux-x86_64"
        }
    }
    #[cfg(target_os = "windows")]
    {
        "windows-x86_64"
    }
}

pub(super) fn frozen_core_sha256() -> &'static str {
    FROZEN_CORE_SHA256
}

fn standard_core_dirs(home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = home {
            dirs.push(home.join("Library/Application Support/RetroArch/cores"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            dirs.push(PathBuf::from(xdg).join("retroarch/cores"));
        }
        if let Some(home) = home {
            dirs.push(home.join(".config/retroarch/cores"));
            dirs.push(home.join(".var/app/org.libretro.RetroArch/config/retroarch/cores"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = home;
        dirs.push(PathBuf::from("C:/RetroArch-Win64/cores"));
        dirs.push(PathBuf::from("C:/RetroArch/cores"));
        if let Some(appdata) = std::env::var_os("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("RetroArch/cores"));
        }
    }

    dirs
}

pub(super) fn core_candidates(program: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(parent) = program
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        candidates.push(parent.join("cores").join(core_file_name()));
    }
    candidates.extend(
        standard_core_dirs(home)
            .into_iter()
            .map(|dir| dir.join(core_file_name())),
    );
    candidates
}

pub(super) fn resolve_core(
    configured: Option<&str>,
    candidates: &[PathBuf],
    managed: PathBuf,
) -> PathBuf {
    if let Some(value) = configured.map(str::trim).filter(|value| !value.is_empty()) {
        return PathBuf::from(value);
    }
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .cloned()
        .unwrap_or(managed)
}

pub(super) fn managed_core_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir
        .join("cores")
        .join(platform_tag())
        .join(core_file_name())
}

pub(super) fn is_on_path(program: &Path) -> bool {
    crate::env::path_program_on_path(program)
}

#[cfg(test)]
mod tests {
    use crate::providers::retroarch::test_support::Scratch;
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn resolve_core_prefers_the_configured_path() {
        let scratch = Scratch::new("configured");
        let resolved = resolve_core(
            Some("/custom/fbneo.dylib"),
            &[scratch.dir.join("standard.dylib")],
            scratch.dir.join("managed.dylib"),
        );
        assert_eq!(resolved, PathBuf::from("/custom/fbneo.dylib"));
    }

    #[test]
    fn resolve_core_uses_the_first_existing_candidate() {
        let scratch = Scratch::new("candidates");
        let absent = scratch.dir.join("absent.dylib");
        let present = scratch.dir.join("present.dylib");
        std::fs::write(&present, b"core").unwrap();
        let resolved = resolve_core(
            Some("   "),
            &[absent, present.clone()],
            scratch.dir.join("managed.dylib"),
        );
        assert_eq!(resolved, present);
    }

    #[test]
    fn resolve_core_falls_back_to_the_managed_path() {
        let scratch = Scratch::new("managed");
        let managed = scratch.dir.join("managed.dylib");
        let resolved = resolve_core(None, &[scratch.dir.join("absent.dylib")], managed.clone());
        assert_eq!(resolved, managed);
    }

    #[test]
    fn managed_core_path_uses_the_platform_tag_and_file_name() {
        let path = managed_core_path(Path::new("/data/the-cabinet"));
        assert_eq!(
            path,
            PathBuf::from("/data/the-cabinet")
                .join("cores")
                .join(platform_tag())
                .join(core_file_name())
        );
    }

    #[test]
    fn core_candidates_start_with_the_program_directory() {
        let program = Path::new("/opt/RetroArch/retroarch");
        let candidates = core_candidates(program, Some(Path::new("/home/player")));
        assert_eq!(
            candidates.first(),
            Some(&PathBuf::from("/opt/RetroArch/cores").join(core_file_name()))
        );
    }

    #[test]
    fn core_candidates_include_the_host_standard_dir() {
        let home = Path::new("/home/player");
        let candidates = core_candidates(Path::new("retroarch"), Some(home));
        #[cfg(target_os = "macos")]
        assert!(candidates.contains(
            &home
                .join("Library/Application Support/RetroArch/cores")
                .join(core_file_name())
        ));
        #[cfg(target_os = "linux")]
        assert!(candidates.contains(&home.join(".config/retroarch/cores").join(core_file_name())));
        #[cfg(target_os = "windows")]
        assert!(candidates
            .iter()
            .any(|path| path.to_string_lossy().contains("RetroArch-Win64")));
    }
}
