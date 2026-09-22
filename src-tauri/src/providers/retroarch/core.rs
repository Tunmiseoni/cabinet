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

/// RetroArch resolves a loaded core's capability metadata by matching
/// `path_basename_nocompression(core_path)` against its own scanned core list (`core_info_find_internal`
/// in `core_info.c`). A miss leaves a zeroed entry, so `core_info_current_supports_netplay()` reports
/// no netplay support even for a perfectly good core. Placing this `.info` beside a core in a scanned
/// directory makes the lookup hit and declares deterministic savestates. See docs/10-lobby-spike.md (L9).
pub(super) fn core_info_file_name() -> String {
    let stem = core_file_name()
        .rsplit_once('.')
        .map_or(core_file_name(), |(stem, _)| stem);
    format!("{stem}.info")
}

pub(super) const CORE_INFO_BODY: &str = "\
display_name = \"FinalBurn Neo\"
corename = \"FBNeo\"
categories = \"Emulator\"
authors = \"FBNeo team\"
license = \"Non-commercial\"
savestate = \"true\"
savestate_features = \"deterministic\"
";

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

/// Locate the host's real `retroarch.cfg` (read-only) from the same roots as the
/// autoconfig search. Recent RetroArch stores it under `<root>/config/`; older installs
/// keep it at `<root>/`.
pub(super) fn resolve_host_config(program: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in autoconfig_dirs(program, home) {
        if let Some(root) = dir.parent() {
            candidates.push(root.join("retroarch.cfg"));
            candidates.push(root.join("config").join("retroarch.cfg"));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

/// Read a `key = "value"` line from a RetroArch config, ignoring comments and blank lines.
fn config_value(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or(value);
        return Some(value.to_string());
    }
    None
}

fn expand_tilde(value: &str, home: Option<&Path>) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value == "~" {
        return home.map(Path::to_path_buf);
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home.map(|home| home.join(rest));
    }
    Some(PathBuf::from(value))
}

/// The directories whose per-core subdirectories hold RetroArch's option files
/// (`<base>/<core>/<core>.opt`), most authoritative first.
fn core_options_bases(program: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    for dir in autoconfig_dirs(program, home) {
        if let Some(root) = dir.parent() {
            let base = root.join("config");
            if !bases.contains(&base) {
                bases.push(base);
            }
        }
    }
    bases
}

/// Whether a file looks like a core-options file for the FBNeo core, so a sibling core's `.opt`
/// is never mistaken for it.
fn has_fbneo_options(path: &Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    raw.lines().any(|line| {
        let line = line.trim();
        !line.is_empty()
            && !line.starts_with('#')
            && line
                .split_once('=')
                .is_some_and(|(name, _)| name.trim().starts_with("fbneo-"))
    })
}

/// Pick the option file RetroArch would load from a per-core options directory: the game-specific
/// one (when the host has game-specific options on), else the per-core file, else the most
/// recently written `.opt` (a folder-specific file).
fn pick_core_options_file(
    core_dir: &Path,
    rom_path: &Path,
    game_specific: bool,
) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(core_dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("opt"))
        })
        .filter(|path| has_fbneo_options(path))
        .collect();
    if candidates.is_empty() {
        return None;
    }

    let matches_stem = |path: &PathBuf, stem: &str| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value == stem)
    };
    if game_specific {
        if let Some(stem) = rom_path.file_stem().and_then(|value| value.to_str()) {
            if let Some(found) = candidates.iter().find(|path| matches_stem(path, stem)) {
                return Some(found.clone());
            }
        }
    }
    if let Some(name) = core_dir.file_name().and_then(|value| value.to_str()) {
        if let Some(found) = candidates.iter().find(|path| matches_stem(path, name)) {
            return Some(found.clone());
        }
    }

    candidates.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    });
    candidates.pop()
}

/// The host's global core-options file: the configured `core_options_path`, else
/// `retroarch-core-options.cfg` beside the config file (RetroArch's own fallback).
fn global_core_options_path(
    host_config: Option<&Path>,
    configured: Option<&str>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    let path = configured
        .and_then(|value| expand_tilde(value, home))
        .or_else(|| {
            host_config
                .and_then(Path::parent)
                .map(|dir| dir.join("retroarch-core-options.cfg"))
        })?;
    path.is_file().then_some(path)
}

/// Locate the core-options file a launch would otherwise read, so a Cabinet session can carry the
/// player's own FBNeo options with our preset on top. Mirrors RetroArch's resolution
/// (`runloop_init_core_options_path`): a game-specific `.opt` when `game_specific_options` is on,
/// else the per-core `.opt`, else the global `core_options_path`.
pub(super) fn resolve_core_options_source(
    program: &Path,
    host_config: Option<&Path>,
    rom_path: &Path,
    home: Option<&Path>,
) -> Option<PathBuf> {
    let host = host_config.and_then(|path| std::fs::read_to_string(path).ok());
    let value = |key: &str| host.as_deref().and_then(|raw| config_value(raw, key));

    let configured = value("core_options_path");
    if value("global_core_options").as_deref() == Some("true") {
        return global_core_options_path(host_config, configured.as_deref(), home);
    }

    let game_specific = value("game_specific_options").as_deref() == Some("true");
    for base in core_options_bases(program, home) {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        let mut core_dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        core_dirs.sort();
        for core_dir in core_dirs {
            if let Some(found) = pick_core_options_file(&core_dir, rom_path, game_specific) {
                return Some(found);
            }
        }
    }

    global_core_options_path(host_config, configured.as_deref(), home)
}

/// The directories RetroArch may scan for cores, most authoritative first: the host config's
/// `libretro_directory`, then the program-relative `cores/`, then the per-OS standard paths.
pub(super) fn retroarch_core_dirs(
    host_config: Option<&Path>,
    program: &Path,
    home: Option<&Path>,
) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let push = |dirs: &mut Vec<PathBuf>, dir: PathBuf| {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    };

    if let Some(config) = host_config {
        if let Ok(contents) = std::fs::read_to_string(config) {
            if let Some(value) = config_value(&contents, "libretro_directory") {
                if let Some(dir) = expand_tilde(&value, home) {
                    push(&mut dirs, dir);
                }
            }
        }
    }
    if let Some(parent) = program
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        push(&mut dirs, parent.join("cores"));
    }
    for dir in standard_core_dirs(home) {
        push(&mut dirs, dir);
    }
    dirs
}

/// The directory to install the core into: the first candidate that already exists, else the first
/// non-program standard path (so a fresh install lands where RetroArch scans by default).
pub(super) fn resolve_retroarch_core_dir(
    host_config: Option<&Path>,
    program: &Path,
    home: Option<&Path>,
) -> Option<PathBuf> {
    let dirs = retroarch_core_dirs(host_config, program, home);
    if let Some(existing) = dirs.iter().find(|dir| dir.is_dir()) {
        return Some(existing.clone());
    }
    standard_core_dirs(home).into_iter().next()
}

/// Write the FBNeo `.info` into `dir` if it is absent, so RetroArch can resolve the core's
/// netplay capability. Returns the path either way.
pub(super) fn write_core_info(dir: &Path) -> crate::error::Result<PathBuf> {
    let path = dir.join(core_info_file_name());
    if path.exists() {
        return Ok(path);
    }
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    std::fs::write(&path, CORE_INFO_BODY)
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    Ok(path)
}

/// Guarantee a core named exactly `fbneo_libretro.<ext>` exists in `core_dir`, so RetroArch's
/// basename lookup hits. An existing file is left untouched; otherwise the core we are about to
/// load (`source`) is copied in. Returns whether the copy happened.
pub(super) fn ensure_core_visible(source: &Path, core_dir: &Path) -> crate::error::Result<bool> {
    let dest = core_dir.join(core_file_name());
    if dest.is_file() {
        return Ok(false);
    }
    std::fs::create_dir_all(core_dir)
        .map_err(|err| format!("cannot create {}: {err}", core_dir.display()))?;
    std::fs::copy(source, &dest).map_err(|err| {
        format!(
            "cannot place the FBNeo core in {} ({err}) — check permissions, or install FBNeo through RetroArch's core downloader",
            core_dir.display()
        )
    })?;
    write_core_info(core_dir)?;
    log::info!(
        "placed the FBNeo core in {} so RetroArch can resolve its netplay support",
        core_dir.display()
    );
    Ok(true)
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
    managed: &[PathBuf],
) -> PathBuf {
    if let Some(value) = configured.map(str::trim).filter(|value| !value.is_empty()) {
        return PathBuf::from(value);
    }
    candidates
        .iter()
        .chain(managed.iter())
        .find(|candidate| candidate.is_file())
        .cloned()
        .or_else(|| managed.first().cloned())
        .unwrap_or_default()
}

pub(super) fn managed_core_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir
        .join("cores")
        .join(platform_tag())
        .join(core_file_name())
}

/// Both names the app may have written for the managed FBNeo core: the canonical
/// `fbneo_libretro.<ext>` first, then the release asset name (`fbneo_libretro-<tag>.<ext>`)
/// that an earlier version placed under the same directory.
pub(super) fn managed_core_candidates(app_data_dir: &Path) -> Vec<PathBuf> {
    let canonical = managed_core_path(app_data_dir);
    let legacy = canonical.with_file_name(core_asset_name());
    vec![canonical, legacy]
}

/// If `resolved` points at a legacy asset-named managed core, normalize it to the canonical
/// managed name so later detects and downloads converge on one file. Handles both a stale
/// configured path to the old name and a legacy file left on disk. Best-effort: returns the
/// canonical path when it exists or the migration succeeds, otherwise the original path.
pub(super) fn adopt_managed_core(resolved: PathBuf, app_data_dir: &Path) -> PathBuf {
    let canonical = managed_core_path(app_data_dir);
    let legacy = canonical.with_file_name(core_asset_name());
    if resolved != legacy {
        return resolved;
    }
    if canonical.is_file() {
        log::info!(
            "configured core {} resolves to managed {}",
            legacy.display(),
            canonical.display()
        );
        return canonical;
    }
    if !legacy.is_file() {
        return canonical;
    }
    if let Some(parent) = canonical.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            log::warn!("cannot create {}: {err}", parent.display());
            return legacy;
        }
    }
    if std::fs::rename(&legacy, &canonical).is_ok() {
        log::info!(
            "migrated core {} -> {}",
            legacy.display(),
            canonical.display()
        );
        return canonical;
    }
    if std::fs::copy(&legacy, &canonical).is_ok() {
        let _ = std::fs::remove_file(&legacy);
        log::info!(
            "migrated core {} -> {}",
            legacy.display(),
            canonical.display()
        );
        return canonical;
    }
    log::warn!(
        "could not migrate core {} to {}",
        legacy.display(),
        canonical.display()
    );
    legacy
}

const PROGRESS_STEP_BYTES: u64 = 1024 * 1024;

/// Whether a chunk boundary is worth reporting: every [`PROGRESS_STEP_BYTES`] while receiving,
/// plus the final byte. Reporting the exact total matters for a known-size download so the bar
/// can reach 100%.
fn report_progress(downloaded: u64, reported: u64, total: Option<u64>) -> bool {
    if total.is_some_and(|total| downloaded >= total) {
        return downloaded != reported;
    }
    downloaded.saturating_sub(reported) >= PROGRESS_STEP_BYTES
}

pub(crate) fn download_managed_core(
    app_data_dir: &Path,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> crate::error::Result<PathBuf> {
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
    let total = response.body().content_length();
    let mut body = response.into_body();

    let temp = dest.with_extension("download");
    {
        let mut file = std::fs::File::create(&temp)
            .map_err(|err| format!("cannot create {}: {err}", temp.display()))?;
        let mut reader = body.as_reader();
        let mut buffer = [0u8; 64 * 1024];
        let mut downloaded: u64 = 0;
        let mut reported: u64 = 0;
        loop {
            let read = std::io::Read::read(&mut reader, &mut buffer)
                .map_err(|err| format!("cannot read the download: {err}"))?;
            if read == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buffer[..read])
                .map_err(|err| format!("cannot write {}: {err}", temp.display()))?;
            downloaded += read as u64;
            if report_progress(downloaded, reported, total) {
                reported = downloaded;
                on_progress(downloaded, total);
            }
        }
        if downloaded != reported {
            on_progress(downloaded, total);
        }
    }

    let actual = sha256_file(&temp)?;
    let expected = frozen_core_sha256();
    if actual != expected {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("core sha256 mismatch: expected {expected}, got {actual}").into());
    }
    std::fs::rename(&temp, &dest)
        .map_err(|err| format!("cannot install {}: {err}", dest.display()))?;
    let legacy = dest.with_file_name(core_asset_name());
    if legacy.is_file() {
        let _ = std::fs::remove_file(&legacy);
    }
    write_core_info(parent)?;
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
            &[scratch.dir.join("managed.dylib")],
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
            &[scratch.dir.join("managed.dylib")],
        );
        assert_eq!(resolved, present);
    }

    #[test]
    fn resolve_core_falls_back_to_the_managed_path() {
        let scratch = Scratch::new("managed");
        let managed = scratch.dir.join("managed.dylib");
        let resolved = resolve_core(
            None,
            &[scratch.dir.join("absent.dylib")],
            std::slice::from_ref(&managed),
        );
        assert_eq!(resolved, managed);
    }

    #[test]
    fn resolve_core_finds_the_canonical_managed_core() {
        let scratch = Scratch::new("managed-canonical");
        let canonical = scratch.dir.join("fbneo_libretro.dylib");
        let legacy = scratch.dir.join("fbneo_libretro-legacy.dylib");
        std::fs::write(&canonical, b"core").unwrap();
        std::fs::write(&legacy, b"core").unwrap();
        let resolved = resolve_core(None, &[], &[canonical.clone(), legacy]);
        assert_eq!(resolved, canonical);
    }

    #[test]
    fn resolve_core_finds_the_asset_named_managed_core() {
        let scratch = Scratch::new("managed-legacy");
        let canonical = scratch.dir.join("fbneo_libretro.dylib");
        let legacy = scratch.dir.join("fbneo_libretro-legacy.dylib");
        std::fs::write(&legacy, b"core").unwrap();
        let resolved = resolve_core(None, &[], &[canonical, legacy.clone()]);
        assert_eq!(resolved, legacy);
    }

    #[test]
    fn managed_core_candidates_pair_the_canonical_and_asset_names() {
        let dir = Path::new("/data/the-cabinet");
        let candidates = managed_core_candidates(dir);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0], managed_core_path(dir));
        assert_eq!(
            candidates[1].file_name().and_then(|name| name.to_str()),
            Some(core_asset_name().as_str())
        );
    }

    #[test]
    fn adopt_managed_core_renames_the_asset_named_core() {
        let scratch = Scratch::new("adopt-legacy");
        let app_data_dir = scratch.dir.join("app-data");
        let canonical = managed_core_path(&app_data_dir);
        let legacy = canonical.with_file_name(core_asset_name());
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, b"core").unwrap();

        let adopted = adopt_managed_core(legacy.clone(), &app_data_dir);

        assert_eq!(adopted, canonical);
        assert!(canonical.is_file());
        assert!(!legacy.exists());
    }

    #[test]
    fn adopt_managed_core_keeps_the_canonical_core() {
        let scratch = Scratch::new("adopt-canonical");
        let app_data_dir = scratch.dir.join("app-data");
        let canonical = managed_core_path(&app_data_dir);
        std::fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        std::fs::write(&canonical, b"core").unwrap();

        let adopted = adopt_managed_core(canonical.clone(), &app_data_dir);

        assert_eq!(adopted, canonical);
    }

    #[test]
    fn adopt_managed_core_normalizes_a_stale_configured_asset_path() {
        let scratch = Scratch::new("adopt-stale-config");
        let app_data_dir = scratch.dir.join("app-data");
        let canonical = managed_core_path(&app_data_dir);
        let legacy = canonical.with_file_name(core_asset_name());
        std::fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        std::fs::write(&canonical, b"core").unwrap();

        let adopted = adopt_managed_core(legacy, &app_data_dir);

        assert_eq!(adopted, canonical);
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
    fn resolve_host_config_finds_the_config_under_the_config_dir() {
        let scratch = Scratch::new("host-config");
        let program = scratch.dir.join("bin/retroarch");
        let bin = program.parent().unwrap();
        std::fs::create_dir_all(bin.join("autoconfig")).unwrap();
        std::fs::create_dir_all(bin.join("config")).unwrap();
        let cfg = bin.join("config/retroarch.cfg");
        std::fs::write(&cfg, "input_cheat_toggle = \"u\"\n").unwrap();
        let home = scratch.dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(resolve_host_config(&program, Some(&home)), Some(cfg));
    }

    #[test]
    fn resolve_host_config_prefers_a_root_level_config() {
        let scratch = Scratch::new("host-config-root");
        let program = scratch.dir.join("bin/retroarch");
        let bin = program.parent().unwrap();
        std::fs::create_dir_all(bin.join("config")).unwrap();
        let root_cfg = bin.join("retroarch.cfg");
        std::fs::write(&root_cfg, "").unwrap();
        std::fs::write(bin.join("config/retroarch.cfg"), "").unwrap();
        let home = scratch.dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(resolve_host_config(&program, Some(&home)), Some(root_cfg));
    }

    #[test]
    fn resolve_host_config_is_none_when_absent() {
        let scratch = Scratch::new("host-config-missing");
        let program = scratch.dir.join("bin/retroarch");
        let home = scratch.dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        assert!(resolve_host_config(&program, Some(&home)).is_none());
    }

    #[test]
    fn core_info_file_name_replaces_the_extension_with_info() {
        assert_eq!(core_info_file_name(), "fbneo_libretro.info");
    }

    #[test]
    fn config_value_reads_a_quoted_setting() {
        let contents = "# comment\nfoo = \"bar\"\nlibretro_directory = \"/cores/here\"\n";
        assert_eq!(
            config_value(contents, "libretro_directory").as_deref(),
            Some("/cores/here")
        );
        assert_eq!(config_value(contents, "missing"), None);
        assert_eq!(config_value("x = y", "x").as_deref(), Some("y"));
    }

    #[test]
    fn expand_tilde_uses_the_home_dir() {
        let home = Path::new("/home/player");
        assert_eq!(
            expand_tilde("~/r", Some(home)),
            Some(PathBuf::from("/home/player/r"))
        );
        assert_eq!(
            expand_tilde("~", Some(home)),
            Some(PathBuf::from("/home/player"))
        );
        assert_eq!(
            expand_tilde("/abs", Some(home)),
            Some(PathBuf::from("/abs"))
        );
        assert_eq!(expand_tilde("  ", Some(home)), None);
    }

    #[test]
    fn retroarch_core_dirs_prefers_the_configured_directory() {
        let scratch = Scratch::new("core-dirs");
        let config = scratch.dir.join("retroarch.cfg");
        std::fs::write(&config, "libretro_directory = \"~/retroarch-cores\"\n").unwrap();
        let home = Path::new("/home/player");
        let dirs = retroarch_core_dirs(Some(&config), Path::new("/opt/retroarch"), Some(home));
        assert_eq!(dirs.first(), Some(&home.join("retroarch-cores")));
    }

    #[test]
    fn resolve_retroarch_core_dir_prefers_the_existing_directory() {
        let scratch = Scratch::new("core-dir-existing");
        let configured = scratch.dir.join("configured-cores");
        std::fs::create_dir_all(&configured).unwrap();
        let config = scratch.dir.join("retroarch.cfg");
        std::fs::write(
            &config,
            format!("libretro_directory = \"{}\"\n", configured.display()),
        )
        .unwrap();
        assert_eq!(
            resolve_retroarch_core_dir(Some(&config), Path::new("/opt/retroarch"), None),
            Some(configured)
        );
    }

    #[test]
    fn ensure_core_visible_installs_a_missing_core_and_info() {
        let scratch = Scratch::new("ensure-core");
        let source = scratch.dir.join("source-core");
        std::fs::write(&source, b"core-bytes").unwrap();
        let core_dir = scratch.dir.join("retroarch/cores");

        assert!(ensure_core_visible(&source, &core_dir).unwrap());
        assert_eq!(
            std::fs::read(core_dir.join(core_file_name())).unwrap(),
            b"core-bytes"
        );
        let contents = std::fs::read_to_string(core_dir.join(core_info_file_name())).unwrap();
        assert!(contents.contains("savestate = \"true\""));
        assert!(contents.contains("savestate_features = \"deterministic\""));
        assert!(contents.contains("corename = \"FBNeo\""));
    }

    #[test]
    fn ensure_core_visible_leaves_an_existing_core_untouched() {
        let scratch = Scratch::new("ensure-core-existing");
        let source = scratch.dir.join("source-core");
        std::fs::write(&source, b"ours").unwrap();
        let core_dir = scratch.dir.join("retroarch/cores");
        std::fs::create_dir_all(&core_dir).unwrap();
        let existing = core_dir.join(core_file_name());
        std::fs::write(&existing, b"theirs").unwrap();

        assert!(!ensure_core_visible(&source, &core_dir).unwrap());
        assert_eq!(std::fs::read(&existing).unwrap(), b"theirs");
        assert!(!core_dir.join(core_info_file_name()).exists());
    }

    #[test]
    fn ensure_core_visible_reports_a_copy_failure() {
        let scratch = Scratch::new("ensure-core-fail");
        let error = ensure_core_visible(
            &scratch.dir.join("absent-core"),
            &scratch.dir.join("retroarch/cores"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("FBNeo core"));
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
        assert!(download_managed_core(&scratch.dir, |_, _| {}).is_err());
    }

    #[test]
    fn report_progress_throttles_to_the_step_and_reports_the_final_byte() {
        let total = Some(10 * PROGRESS_STEP_BYTES);
        assert!(!report_progress(PROGRESS_STEP_BYTES / 2, 0, total));
        assert!(report_progress(PROGRESS_STEP_BYTES, 0, total));
        assert!(report_progress(
            10 * PROGRESS_STEP_BYTES,
            9 * PROGRESS_STEP_BYTES,
            total
        ));
        assert!(!report_progress(
            10 * PROGRESS_STEP_BYTES,
            10 * PROGRESS_STEP_BYTES,
            total
        ));
    }

    #[test]
    fn report_progress_uses_the_step_when_the_total_is_unknown() {
        assert!(!report_progress(1024, 0, None));
        assert!(report_progress(PROGRESS_STEP_BYTES, 0, None));
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
        let core = download_managed_core(&scratch.dir, |_, _| {}).expect("download core");
        assert!(core.is_file());
        assert_eq!(sha256_file(&core).unwrap(), frozen_core_sha256());
    }

    #[test]
    fn has_fbneo_options_recognizes_only_fbneo_files() {
        let scratch = Scratch::new("fbneo-options-detect");
        let fbneo = scratch.dir.join("FBNeo.opt");
        std::fs::write(&fbneo, "fbneo-socd = \"3\"\n").unwrap();
        let snes = scratch.dir.join("snes9x.opt");
        std::fs::write(&snes, "snes9x_region = \"auto\"\n").unwrap();
        assert!(has_fbneo_options(&fbneo));
        assert!(!has_fbneo_options(&snes));
        assert!(!has_fbneo_options(&scratch.dir.join("missing.opt")));
    }

    #[test]
    fn core_options_bases_use_the_program_config_directory() {
        let program = Path::new("/opt/RetroArch/retroarch");
        let bases = core_options_bases(program, None);
        assert_eq!(bases, vec![PathBuf::from("/opt/RetroArch/config")]);
    }

    #[test]
    fn pick_core_options_file_prefers_the_per_core_file() {
        let scratch = Scratch::new("pick-per-core");
        let core_dir = scratch.dir.join("FinalBurn Neo");
        std::fs::create_dir_all(&core_dir).unwrap();
        std::fs::write(core_dir.join("FinalBurn Neo.opt"), "fbneo-socd = \"3\"\n").unwrap();
        std::fs::write(core_dir.join("roms.opt"), "fbneo-hiscores = \"enabled\"\n").unwrap();
        std::fs::write(core_dir.join("snes9x.opt"), "snes9x_region = \"auto\"\n").unwrap();
        let picked = pick_core_options_file(&core_dir, &scratch.dir.join("sfiii3nr1.zip"), true);
        assert_eq!(picked, Some(core_dir.join("FinalBurn Neo.opt")));
    }

    #[test]
    fn pick_core_options_file_prefers_the_game_specific_file_when_enabled() {
        let scratch = Scratch::new("pick-game");
        let core_dir = scratch.dir.join("FinalBurn Neo");
        std::fs::create_dir_all(&core_dir).unwrap();
        let per_core = core_dir.join("FinalBurn Neo.opt");
        std::fs::write(&per_core, "fbneo-socd = \"3\"\n").unwrap();
        let game = core_dir.join("sfiii3nr1.opt");
        std::fs::write(&game, "fbneo-socd = \"0\"\n").unwrap();
        let rom = scratch.dir.join("sfiii3nr1.zip");
        assert_eq!(pick_core_options_file(&core_dir, &rom, true), Some(game));
        assert_eq!(
            pick_core_options_file(&core_dir, &rom, false),
            Some(per_core)
        );
    }

    #[test]
    fn pick_core_options_file_is_none_without_fbneo_options() {
        let scratch = Scratch::new("pick-empty");
        let core_dir = scratch.dir.join("Some Core");
        std::fs::create_dir_all(&core_dir).unwrap();
        std::fs::write(core_dir.join("Some Core.opt"), "other = \"1\"\n").unwrap();
        assert!(pick_core_options_file(&core_dir, &scratch.dir.join("rom.zip"), true).is_none());
    }

    #[test]
    fn global_core_options_path_prefers_the_configured_file() {
        let scratch = Scratch::new("global-options");
        let config_dir = scratch.dir.join("RetroArch/config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let host_config = config_dir.join("retroarch.cfg");
        std::fs::write(&host_config, "").unwrap();
        let default_path = config_dir.join("retroarch-core-options.cfg");
        std::fs::write(&default_path, "fbneo-socd = \"3\"\n").unwrap();
        assert_eq!(
            global_core_options_path(Some(&host_config), None, None),
            Some(default_path)
        );

        let configured = scratch.dir.join("custom-options.cfg");
        std::fs::write(&configured, "fbneo-socd = \"3\"\n").unwrap();
        assert_eq!(
            global_core_options_path(Some(&host_config), configured.to_str(), None),
            Some(configured)
        );
    }

    #[test]
    fn resolve_core_options_source_honors_a_global_config() {
        let scratch = Scratch::new("source-global");
        let global = scratch.dir.join("global.cfg");
        std::fs::write(&global, "fbneo-socd = \"3\"\n").unwrap();
        let host_config = scratch.dir.join("retroarch.cfg");
        std::fs::write(
            &host_config,
            format!(
                "global_core_options = \"true\"\ncore_options_path = \"{}\"\n",
                global.display()
            ),
        )
        .unwrap();
        let resolved = resolve_core_options_source(
            Path::new("/opt/RetroArch/retroarch"),
            Some(&host_config),
            &scratch.dir.join("sfiii3nr1.zip"),
            None,
        );
        assert_eq!(resolved, Some(global));
    }

    #[test]
    fn resolve_core_options_source_finds_the_per_core_options() {
        let scratch = Scratch::new("source-per-core");
        let root = scratch.dir.join("RetroArch");
        let program = root.join("retroarch");
        let config_dir = root.join("config");
        std::fs::create_dir_all(config_dir.join("FinalBurn Neo")).unwrap();
        let per_core = config_dir.join("FinalBurn Neo/FinalBurn Neo.opt");
        std::fs::write(&per_core, "fbneo-socd = \"3\"\n").unwrap();
        let host_config = config_dir.join("retroarch.cfg");
        std::fs::write(&host_config, "game_specific_options = \"true\"\n").unwrap();

        let resolved = resolve_core_options_source(
            &program,
            Some(&host_config),
            &scratch.dir.join("sfiii3nr1.zip"),
            None,
        );
        assert_eq!(resolved, Some(per_core));
    }

    #[test]
    fn resolve_core_options_source_prefers_a_game_specific_file() {
        let scratch = Scratch::new("source-game");
        let root = scratch.dir.join("RetroArch");
        let program = root.join("retroarch");
        let config_dir = root.join("config");
        std::fs::create_dir_all(config_dir.join("FinalBurn Neo")).unwrap();
        let core_dir = config_dir.join("FinalBurn Neo");
        std::fs::write(core_dir.join("FinalBurn Neo.opt"), "fbneo-socd = \"3\"\n").unwrap();
        let game = core_dir.join("sfiii3nr1.opt");
        std::fs::write(&game, "fbneo-socd = \"0\"\n").unwrap();
        let host_config = config_dir.join("retroarch.cfg");
        std::fs::write(&host_config, "game_specific_options = \"true\"\n").unwrap();

        let resolved = resolve_core_options_source(
            &program,
            Some(&host_config),
            &scratch.dir.join("sfiii3nr1.zip"),
            None,
        );
        assert_eq!(resolved, Some(game));
    }
}
