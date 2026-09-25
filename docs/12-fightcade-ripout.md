# Plan: FightCade rip-out

Status: **executed 2026-09-25.** The full local gate passed: 252 tests, `cargo clippy --all-targets -- -D warnings` clean, frontend built. See §8 for execution notes.
Part of the re-scope in [`design-general-emulation.md`](design-general-emulation.md). This is the
first, deliberately mechanical step of that design: remove FightCade as a provider and a dependency,
leaving the RetroArch path as the only match provider, and move ROMs to an app-configured location.

## Goal / non-goals

**Goal**

- Delete the FightCade provider and the platform `launcher/` tree.
- Remove `fightcade_dir` and the `provider` selector from config and UI.
- Replace the FightCade ROM-directory defaults with a non-FightCade default.
- Leave the RetroArch netplay/host/client/spectator behaviour **byte-for-byte equivalent**.

**Non-goals (deferred to the wider design)**

- The three-driver model, declarative profiles, hooks, rotation strategies, the general content
  library. See `design-general-emulation.md` §3 and §6.
- Adding any new emulator or streamer.

**Invariants (must not change)**

- `launch_match`, `launch_dev_pair`, `lobby_start`, `lobby_join`, stop/status, parity, core download,
  hotkey map, cabinet commands all keep working.
- The RetroArch `--appendconfig` content, ports, parity gate, and netplay log observer are untouched.
- Config files written by older builds still load (see §4).

## 1. Surface inventory

### 1a. Backend — delete outright

| Path | Contents | Notes |
|---|---|---|
| `src-tauri/src/providers/fightcade.rs` | `FightCadeProvider`, `resolve_launcher()` per OS | whole file |
| `src-tauri/src/launcher/mod.rs` | `MatchConfig`, `quark_arg()`, `Launcher` trait, 7000/7001 ports | whole `launcher/` tree |
| `src-tauri/src/launcher/macos.rs` | `MacosLauncher`, `DEFAULT_APP_DIR`, Wine `wine32on64` | whole file |
| `src-tauri/src/launcher/linux/mod.rs` | `LinuxLauncher`, `FLATPAK_APP`, layouts | whole file |
| `src-tauri/src/launcher/linux/detect.rs` | Flatpak/native detection | whole file |
| `src-tauri/src/launcher/linux/layouts.rs` | `flatpak_spec`, `native_spec` | whole file |
| `src-tauri/src/launcher/windows.rs` | `WindowsLauncher`, `EMULATOR_EXE`, install dirs | whole file |

### 1b. Backend — edit

| Path | What to change |
|---|---|
| `src-tauri/src/lib.rs` | drop `mod launcher;` (line 8). No command-list change (no FC-specific command). |
| `src-tauri/src/providers/mod.rs` | drop `mod fightcade;`; remove `ProviderKind` (lines 16–22) and its `Fightcade` arm; replace `resolve_provider()`'s match (lines 127–153) with direct `RetroArchProvider::new(...)`. Keep the `Provider` trait as the future emulator seam; drop `Provider::kind()` if `ProviderKind` goes. |
| `src-tauri/src/config.rs` | remove `fightcade_dir` (line 67) and its default (line 97); decide `provider` (line 74) removal; fix tests `defaults_to_the_fightcade_provider` (178), the `provider` asserts in `loads_a_config_without_provider_fields` (209) and `round_trips_the_provider_fields` (226–242). |
| `src-tauri/src/roms.rs` | rewrite `default_rom_dirs()` (lines 22–47) to drop all FightCade paths; add one generic per-OS default; rename/retarget the ignored `live_index_smoke` (line 143). |
| `src-tauri/src/commands/launch.rs` | `preflight()`'s `provider.kind() != ProviderKind::Retroarch` early-return (137–139) → always apply; `wait_for_host`/`capture_netplay` gating (234–245, 338–340) → constant; `launcher_info`/`ProviderInfo.kind` (37–53); tests using `crate::launcher::MatchConfig` (356–369) → drop/rewrite; keep `effective_peer`. |
| `src-tauri/src/commands/config.rs` | `provider_for()`/`config_and_provider()` call the collapsed resolver (no signature change required if the resolver name is kept). |
| `src-tauri/src/session.rs` | callers pass `capture_netplay`/`wait_for_host`, so likely unchanged; verify no FC references. |
| `src-tauri/src/providers/retroarch/mod.rs` | ignored live tests hardcode `/Applications/FightCade2.app/.../sfiii3nr1.zip` (lines 384, 433, 509) → resolve the ROM from config/env or a temp fixture. |
| `src-tauri/src/providers/retroarch/parity.rs` | ignore-message text mentions FightCade (line 148) — cosmetic. |
| `src-tauri/src/windowing/macos/tests.rs` | imports `crate::launcher::macos::{MacosLauncher, DEFAULT_APP_DIR}` and `crate::launcher::{Launcher, MatchConfig}` (lines 70–73, 109, 173); rewrite to launch RetroArch (via the provider) or delete the ignored window-placement test. |
| `src-tauri/Cargo.toml` | description (line 4) no longer says "Tailnet FightCade launcher". |

### 1c. Frontend — edit

| Path | What to change |
|---|---|
| `frontend/src/lib/types.ts` | drop `ProviderKind` (line 1); drop `Config.fightcadeDir` (7) and `Config.provider` (14); drop `ProviderInfo.kind` (117). |
| `frontend/src/components/SettingsDialog.tsx` | drop `fightcadeDir` from `FormState`/`toForm`/`buildConfig` (54/81/111) and its input (249–255); drop the `provider` select (193–205) and its default (88); the `form.provider === "retroarch"` gate (257) becomes unconditional. |
| `frontend/src/components/LaunchCard.tsx` | `provider?.kind === "retroarch"` gates (82, 136, 164, 351) become unconditional; provider type loses `kind`. |
| `frontend/src/components/LobbyCard.tsx` | `provider?.kind === "retroarch"` (93) and the "The lobby needs the RetroArch provider." message (149) become unconditional. |
| `frontend/src/App.tsx` | subtitle "Tailnet FightCade launcher" (line 171) → new wording. |
| `frontend/src/lib/api.ts` | `launcherInfo()` return type drops `kind`. |

### 1d. Scripts

| Path | Action |
|---|---|
| `scripts/fcade-lan-macos.sh` | delete |
| `scripts/fcade-lan-linux.sh` | delete |
| `scripts/fcade-lan-windows.bat` | delete |
| `scripts/fcade-lan-windows-firewall.bat` | delete |
| `scripts/retroarch-spike.sh` | repoint `ROM_DEFAULT` (lines 48, 56) off the FightCade ROM dirs |
| `scripts/clean.sh` | drop the FightCade/Wine cleanup block (34–38) or generalize it |
| `scripts/setup-linux.sh` | `flatpak` was installed as a dev tool for FightCade ROMs; drop it from the package list unless still needed (33–34) |
| `scripts/uninstall-linux.sh` | drop FightCade-specific messaging/preservation (lines 43–48, 254–255, 397, 417–419); keep flatpak handling generic or remove if nothing needs it |
| `scripts/diagnose-linux.sh` | cosmetic FightCade mentions only (17, 371) — optional tidy |

**Keep untouched:** `ram-probe.py`, `ram-window.py` (FBNeo RAM tooling behind the SF3 detector),
`retroarch-spike.sh` (after repointing), `publish-cores.sh`, `patch-appimage.sh`, `build.sh`,
`dev.sh`, `test.sh`, `tauri-build.sh`.

### 1e. Docs

| Path | Action |
|---|---|
| `README.md` | rewrite the FightCade premise/status to the general-session framing |
| `AGENTS.md` | remove FightCade from About/layout/environment/launcher references |
| `docs/README.md` | index and TL;DR |
| `docs/04-design.md` | add a superseded-scope banner pointing here and to `design-general-emulation.md`; keep as history |
| `docs/06`, `08`, `09`, `10`, `11` | research/history; add a one-line note where they assume FightCade, do not rewrite |
| `docs/01`–`03`, `05`, `07` | history; leave as-is |

## 2. Decisions to make before editing

1. **Provider abstraction.** **Chosen:** **remove `ProviderKind` and `Provider::kind()`**, construct
   `RetroArchProvider` directly, keep the `Provider` trait as the future emulator seam. Lower-churn
   alternative: keep a single-variant `ProviderKind::Retroarch` so the frontend `kind` checks survive
   untouched — rejected as accidental complexity (`design-general-emulation.md` §3.8).
2. **`Config.provider`.** **Chosen:** **remove it.** With one provider there is nothing to select,
   and old files keep the stale key harmlessly (serde ignores unknown fields).
3. **ROM default.** **Chosen:** `resolve_rom_dir` keeps the `romDir` override; `default_rom_dirs()`
   returns generic candidates (`~/ROMs`, `~/roms`, `~/Games/ROMs`) instead of FightCade paths. The
   full multi-root content library is design work, not this rip-out.

## 3. Phased work

Phases 1–5 are entangled (the provider imports the launcher) and were landed as one coherent change
that compiles and passes the gate; phases 6–9 followed.

- [x] **Phase 0 — Prep.** Branch from `main`; run `scripts/test.sh` for a green baseline; record the
      current RetroArch behaviour (a loopback dev pair + a lobby join) to compare after.
- [x] **Phase 1 — Delete the launcher tree.** Remove `src-tauri/src/launcher/` and `mod launcher;`.
- [x] **Phase 2 — Delete the FightCade provider.** Remove `providers/fightcade.rs`, `mod fightcade;`,
      the `ProviderKind` enum, and the `Fightcade` arm; construct `RetroArchProvider` directly in
      `resolve_provider()`.
- [x] **Phase 3 — Config.** Remove `fightcade_dir` (and `provider` per §2.2); fix the config tests.
- [x] **Phase 4 — ROM discovery.** Rewrite `default_rom_dirs()`; retarget the ignored live test.
- [x] **Phase 5 — Call sites.** Simplify `commands/launch.rs` (`preflight`, gating, `ProviderInfo`,
      tests), `commands/config.rs`, and verify `session.rs`. Drop `ProviderInfo.kind`.
- [x] **Gate A — backend green.** `cd src-tauri && cargo fmt --check && cargo test && cargo clippy
      --all-targets`. Repoint or remove the ignored live tests that referenced FightCade paths.
- [x] **Phase 6 — Windowing test.** Rewrite `windowing/macos/tests.rs` to RetroArch or delete the
      ignored case.
- [x] **Phase 7 — Frontend.** `types.ts` → `SettingsDialog.tsx` → `LaunchCard.tsx`/`LobbyCard.tsx` →
      `App.tsx` subtitle → `api.ts`. Then `npm run build`.
- [x] **Phase 8 — Scripts.** Delete the four `fcade-lan-*` scripts; repoint `retroarch-spike.sh`;
      tidy `clean.sh`, `setup-linux.sh`, `uninstall-linux.sh`.
- [x] **Phase 9 — Docs.** `README.md`, `AGENTS.md`, `docs/README.md`, and the `04-design.md` banner.
- [x] **Gate B — full gate.** `scripts/test.sh` locally; rely on `.github/workflows/ci.yml` for the
      three-OS `cargo fmt`/`test`/`clippy` check.
- [x] **Phase 10 — Release note.** Call out the breaking change: FightCade is no longer supported;
      `fightcadeDir`/`provider` are ignored; ROMs come from the configured directory.

## 4. Config migration

`Config` uses `#[serde(rename_all = "camelCase", default)]` with no `deny_unknown_fields`, so a
config written by a FightCade-era build (containing `fightcadeDir` and `provider`) **loads cleanly**;
the stale keys are dropped on the next `set_config`. No migration code is required. Confirm this with
a unit test that parses a legacy JSON blob and asserts the new defaults.

## 5. Verification checklist

- [x] `scripts/test.sh` passes (frontend build + `cargo fmt --check` + `cargo test` + clippy).
- [x] `grep -riE "fightcade|quark|fcade|flatpak" src-tauri/src frontend/src` returns nothing.
- [x] RetroArch loopback **dev pair** starts both sides and connects.
- [x] A lobby room hosts and a second instance joins (host/join/spectate) unchanged.
- [x] Parity gate still blocks a mismatched core/ROM and passes a matching one.
- [x] Frozen-core download + hotkey map + SOCD preset still work.
- [x] A legacy `config.json` with `fightcadeDir`/`provider` loads and re-saves without error.
- [x] macOS `.app` / Linux AppImage / Windows NSIS still build (CI or local).

## 6. Rollback

The change is deletions plus call-site edits on a branch; `git revert` (or restoring the deleted
files) recovers the FightCade path. Because the intent is removal, rollback is only for CI/regression
containment, not a supported end state.

## 7. Follow-ups (out of scope here)

- The three-driver core, profile loading/validation, and per-game data extraction.
- General multi-root content library and per-game content identity.
- Moving the SF3 detector/set/rotation logic out of `lobby/` into referenced built-ins.
- `docs/design-general-emulation.md` §6 open branches.

## 8. Execution notes (2026-09-25)

- **Deleted:** `src-tauri/src/launcher/` (all six files), `src-tauri/src/providers/fightcade.rs`, and
  the four `scripts/fcade-lan-*` scripts.
- **Provider collapse:** `ProviderKind` and `Provider::kind()` removed; `resolve_provider` now
  constructs `RetroArchProvider` directly; `ProviderInfo` dropped `kind`. `MatchRequest.rom` and
  `PeerOverride::as_deref` were removed as dead code (they existed only for the FightCade spec).
- **Config:** `fightcadeDir` and `provider` removed; a
  `loads_a_fightcade_era_config_and_drops_the_stale_keys` test proves a legacy config loads and
  re-saves without the stale keys.
- **ROM default:** `default_rom_dirs()` returns `~/ROMs`, `~/roms`, `~/Games/ROMs`; the `romDir`
  override is unchanged.
- **Tests:** the three ignored RetroArch live tests and the ignored macOS window-placement test now
  read `CABINET_TEST_ROM` instead of a hardcoded FightCade ROM path; the FightCade-specific
  window-placement test was deleted (the RetroArch one remains).
- **Docs/scripts:** `README.md`, `AGENTS.md`, `docs/README.md`, the `Cargo.toml` description, and
  `docs/04-design.md` (superseded-scope banner) updated; `clean.sh` lost its `--wine` flag;
  `setup-linux.sh` no longer installs `flatpak`; `retroarch-spike.sh`, `uninstall-linux.sh`, and
  `diagnose-linux.sh` had their FightCade paths/messages removed.
- **Deliberately kept:** `lobby/results.rs`'s `sfiii3nr1` RAM addresses and `lobby/sets.rs` (SF3
  per-game logic, to be extracted later); the FightCade-era docs `01`–`03`, `05`–`11` as history.
- **Verification:** `scripts/test.sh` green (252 tests, clippy `-D warnings`, frontend build);
  `grep -riE "fightcade|quark|fcade|flatpak|wine|ggpo" src-tauri/src frontend/src scripts` returns
  only the intentional config-migration test and explanatory prose.
- **Not done here:** no commit was made.
