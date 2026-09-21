use super::core::frozen_core_sha256;
use super::RetroArchProvider;
use serde::Serialize;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

pub const FROZEN_CORE_GIT: &str = "GIT6bb3167";
pub const FROZEN_ROM_SHA256: &str =
    "4ed142c90fc1a4632d20600d2f5b46caac813422caab62cce3fa9f85ee5cc4dc";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParityStatus {
    pub applicable: bool,
    pub ok: bool,
    pub core_path: String,
    pub core_git: Option<String>,
    pub core_sha256: Option<String>,
    pub expected_git: String,
    pub expected_sha256: String,
    pub rom_path: String,
    pub rom_sha256: Option<String>,
    pub expected_rom_sha256: String,
    pub detail: String,
}

pub(super) fn status(
    provider: &RetroArchProvider,
    rom_path: &Path,
) -> crate::error::Result<Option<ParityStatus>> {
    let expected_git = FROZEN_CORE_GIT.to_string();
    let expected_sha = frozen_core_sha256().to_string();
    let expected_rom = FROZEN_ROM_SHA256.to_string();

    let core_sha = sha256_file(&provider.core).ok();
    let core_git = core_git(&provider.core).ok().flatten();
    let rom_sha = sha256_file(rom_path).ok();

    let core_ok = core_sha.as_deref() == Some(expected_sha.as_str())
        && core_git.as_deref() == Some(expected_git.as_str());
    let rom_ok = rom_sha.as_deref() == Some(expected_rom.as_str());
    let ok = core_ok && rom_ok;

    let detail = if ok {
        "PARITY OK (core revision + sha256, ROM sha256)".to_string()
    } else {
        let mut parts = Vec::new();
        if core_sha.is_none() {
            parts.push(format!(
                "core not found at {} — set the FBNeo core in Settings",
                provider.core.display()
            ));
        } else if !core_ok {
            parts.push("core revision or sha256 differs from the frozen set".to_string());
        }
        if rom_sha.is_none() {
            parts.push(format!("ROM not readable: {}", rom_path.display()));
        } else if !rom_ok {
            parts.push("ROM sha256 differs from the frozen reference".to_string());
        }
        format!("PARITY MISMATCH — {}", parts.join("; "))
    };

    Ok(Some(ParityStatus {
        applicable: true,
        ok,
        core_path: provider.core.to_string_lossy().to_string(),
        core_git,
        core_sha256: core_sha,
        expected_git,
        expected_sha256: expected_sha,
        rom_path: rom_path.to_string_lossy().to_string(),
        rom_sha256: rom_sha,
        expected_rom_sha256: expected_rom,
        detail,
    }))
}

fn sha256_file(path: &Path) -> crate::error::Result<String> {
    use sha2::{Digest, Sha256};

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

fn core_git(path: &Path) -> crate::error::Result<Option<String>> {
    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        if &bytes[index..index + 3] == b"GIT" {
            let mut end = index + 3;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            if end > index + 3 && end - index <= 40 {
                found.insert(String::from_utf8_lossy(&bytes[index..end]).to_string());
            }
            index = end;
        } else {
            index += 1;
        }
    }
    Ok(found.into_iter().next())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::providers::retroarch::core::managed_core_path;
    use crate::providers::retroarch::test_support::{provider, Scratch};
    use crate::providers::Provider;
    use std::path::PathBuf;

    #[test]
    fn parity_detects_mismatch_against_the_frozen_set() {
        let scratch = Scratch::new("parity");
        let rom = scratch.rom();
        let status = provider(&scratch)
            .parity(&rom)
            .unwrap()
            .expect("retroarch has a parity gate");
        assert!(!status.ok);
        assert!(status.detail.starts_with("PARITY MISMATCH"));
        assert_eq!(status.expected_git, FROZEN_CORE_GIT);
        assert_eq!(status.expected_rom_sha256, FROZEN_ROM_SHA256);
    }

    #[test]
    fn reads_the_core_git_revision() {
        let scratch = Scratch::new("git");
        let core = scratch.dir.join("core.bin");
        std::fs::write(&core, b"....GIT6bb3167....extra GIT deadbeef").unwrap();
        assert_eq!(core_git(&core).unwrap().as_deref(), Some("GIT6bb3167"));
    }

    fn managed_core_on_this_machine() -> Option<PathBuf> {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let app_dir = if cfg!(target_os = "macos") {
            home.join("Library/Application Support/com.the-cabinet.app")
        } else if cfg!(target_os = "windows") {
            PathBuf::from(std::env::var_os("APPDATA")?).join("com.the-cabinet.app")
        } else {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/share"))
                .join("com.the-cabinet.app")
        };
        let core = managed_core_path(&app_dir);
        core.is_file().then_some(core)
    }

    #[test]
    #[ignore = "reads the app-managed frozen core and FightCade ROM on this machine"]
    fn live_managed_core_matches_the_frozen_set() {
        let Some(rom_dir) = crate::roms::resolve_rom_dir(&Config::default()) else {
            eprintln!("skipping: no ROM directory found");
            return;
        };
        let rom = rom_dir.join("sfiii3nr1.zip");
        if !rom.is_file() {
            eprintln!("skipping: {} not present", rom.display());
            return;
        }
        let Some(core) = managed_core_on_this_machine() else {
            eprintln!("skipping: managed core not present on this machine");
            return;
        };
        let scratch = Scratch::new("live-managed-parity");
        let cfg = Config {
            retroarch_core: Some(core.to_string_lossy().to_string()),
            ..Config::default()
        };
        let provider = RetroArchProvider::new(&cfg, &scratch.dir, &scratch.dir, false);
        let status = provider
            .parity(&rom)
            .unwrap()
            .expect("retroarch has a parity gate");
        eprintln!(
            "core={} git={:?} rom={} detail={}",
            status.core_path, status.core_git, status.rom_path, status.detail
        );
        assert!(
            status.ok,
            "expected the app-managed frozen core to match: {}",
            status.detail
        );
    }
}
