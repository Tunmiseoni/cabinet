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

pub(crate) fn frozen_core_sha256() -> &'static str {
    FROZEN_CORE_SHA256
}

pub(super) const CORES_RELEASE_TAG: &str = "retroarch-cores-v1";

const CORES_RELEASE_BASE: &str = "https://github.com/Tunmiseoni/the-cabinet/releases/download";

pub(super) fn is_published_platform() -> bool {
    matches!(
        platform_tag(),
        "macos-arm64" | "linux-x86_64" | "windows-x86_64"
    )
}

pub(super) fn core_asset_name() -> String {
    let extension = core_file_name().rsplit('.').next().unwrap_or("");
    format!("fbneo_libretro-{}.{}", platform_tag(), extension)
}

pub(super) fn core_download_url() -> String {
    format!(
        "{CORES_RELEASE_BASE}/{CORES_RELEASE_TAG}/{}",
        core_asset_name()
    )
}

pub(super) fn sha256_file(path: &Path) -> crate::error::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path).map_err(|err| err.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
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

fn autoconfig_dirs(program: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = home {
            dirs.push(home.join("Library/Application Support/RetroArch/autoconfig"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            dirs.push(PathBuf::from(xdg).join("retroarch/autoconfig"));
        }
        if let Some(home) = home {
            dirs.push(home.join(".config/retroarch/autoconfig"));
            dirs.push(home.join(".var/app/org.libretro.RetroArch/config/retroarch/autoconfig"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = home;
        dirs.push(PathBuf::from("C:/RetroArch-Win64/autoconfig"));
        dirs.push(PathBuf::from("C:/RetroArch/autoconfig"));
        if let Some(appdata) = std::env::var_os("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("RetroArch/autoconfig"));
        }
    }

    if let Some(parent) = program
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        dirs.push(parent.join("autoconfig"));
    }

    dirs
}

pub(super) fn resolve_autoconfig_dir(program: &Path, home: Option<&Path>) -> Option<PathBuf> {
    autoconfig_dirs(program, home)
        .into_iter()
        .find(|dir| dir.is_dir())
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

pub(crate) fn download_managed_core(app_data_dir: &Path) -> crate::error::Result<PathBuf> {
    if !is_published_platform() {
        return Err(format!("no published frozen core for {}", platform_tag()).into());
    }
    let dest = managed_core_path(app_data_dir);
    let parent = dest
        .parent()
        .ok_or_else(|| "invalid core destination".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;

    let url = core_download_url();
    log::info!("downloading frozen core from {url}");
    let response = ureq::get(&url)
        .call()
        .map_err(|err| format!("download failed: {err}"))?;
    let mut body = response.into_body();

    let temp = dest.with_extension("download");
    {
        let mut file = std::fs::File::create(&temp)
            .map_err(|err| format!("cannot create {}: {err}", temp.display()))?;
        let mut reader = body.as_reader();
        std::io::copy(&mut reader, &mut file)
            .map_err(|err| format!("cannot write {}: {err}", temp.display()))?;
    }

    let actual = sha256_file(&temp)?;
    let expected = frozen_core_sha256();
    if actual != expected {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("core sha256 mismatch: expected {expected}, got {actual}").into());
    }
    std::fs::rename(&temp, &dest)
        .map_err(|err| format!("cannot install {}: {err}", dest.display()))?;
    log::info!("installed frozen core at {}", dest.display());
    Ok(dest)
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

    #[test]
    fn autoconfig_candidates_include_the_host_standard_dir() {
        let home = Path::new("/home/player");
        let dirs = autoconfig_dirs(Path::new("retroarch"), Some(home));
        #[cfg(target_os = "macos")]
        assert!(dirs.contains(&home.join("Library/Application Support/RetroArch/autoconfig")));
        #[cfg(target_os = "linux")]
        assert!(dirs.contains(&home.join(".config/retroarch/autoconfig")));
        #[cfg(target_os = "windows")]
        assert!(dirs
            .iter()
            .any(|path| path.to_string_lossy().contains("RetroArch-Win64")));
    }

    #[test]
    fn resolve_autoconfig_dir_prefers_the_first_existing_directory() {
        let scratch = Scratch::new("autoconfig");
        let program = scratch.dir.join("RetroArch.app/Contents/MacOS/retroarch");
        let program_autoconfig = program.parent().unwrap().join("autoconfig");
        std::fs::create_dir_all(&program_autoconfig).unwrap();
        let home = scratch.dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(
            resolve_autoconfig_dir(&program, Some(&home)),
            Some(program_autoconfig)
        );
    }

    #[test]
    fn resolve_autoconfig_dir_is_none_when_nothing_exists() {
        let scratch = Scratch::new("no-autoconfig");
        let program = scratch.dir.join("bin/retroarch");
        let home = scratch.dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        assert!(resolve_autoconfig_dir(&program, Some(&home)).is_none());
    }

    #[test]
    fn core_asset_name_combines_the_platform_tag_and_extension() {
        let name = core_asset_name();
        let extension = core_file_name().rsplit('.').next().unwrap();
        assert!(name.starts_with("fbneo_libretro-"));
        assert!(name.contains(platform_tag()));
        assert!(name.ends_with(&format!(".{extension}")));
    }

    #[test]
    fn core_download_url_points_at_the_cores_release_asset() {
        let url = core_download_url();
        assert!(url.contains(CORES_RELEASE_TAG));
        assert!(url.ends_with(&core_asset_name()));
    }

    #[test]
    fn download_rejects_unpublished_platforms() {
        if is_published_platform() {
            return;
        }
        let scratch = Scratch::new("download-unsupported");
        assert!(download_managed_core(&scratch.dir).is_err());
    }

    #[test]
    fn sha256_file_matches_a_known_digest() {
        let scratch = Scratch::new("sha");
        let path = scratch.dir.join("blob");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    #[ignore = "downloads the published frozen core for this platform from GitHub"]
    fn live_downloads_the_frozen_core() {
        if !is_published_platform() {
            eprintln!("skipping: no published core for {}", platform_tag());
            return;
        }
        let scratch = Scratch::new("live-download");
        let core = download_managed_core(&scratch.dir).expect("download core");
        assert!(core.is_file());
        assert_eq!(sha256_file(&core).unwrap(), frozen_core_sha256());
    }
}
