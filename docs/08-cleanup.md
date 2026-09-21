# Cleanup: applied refactors and deferred proposals

Status: **applied pass done 2026-09-20** (balanced, behavior-preserving). A **second pass done
2026-09-21** cleared the post-removal drift (docs, `uninstall-linux.sh`), removed dead frontend/Rust
code, deleted the two stale `handoff-*.md` docs, added `cargo fmt --check` to the gate, and applied
the logging/diagnostics follow-ups (§6 F4/F5/F14/F15/F16). The heavier refactors (§2–§5, §7) are
**proposed and deferred**; none block Phase 3. This file is the backlog for a
future cleanup session, so day-to-day feature work has a written target instead of ad-hoc churn.

> **Removed 2026-09-21:** the rooms/lobbies/KotH subsystem, the local lifetime score ledger, and
> the emulator overlay/result watcher were deleted entirely (see `docs/04-design.md` §3/§5). Some
> paths referenced in the §1 table below (`scores.rs`, `control.rs`, `service.rs`) no longer exist;
> the table is kept as a record of that pass.

## 1. Applied (2026-09-20)

A balanced pass — consistent logging, shared helpers/constants, tighter module boundaries,
session API ergonomics, and a lighter frontend data layer. No behavior or dependency changes.

| Area | Change | Where |
|---|---|---|
| Logging | Replaced the last production `eprintln!` with `log` macros, so every backend message reaches `app_log_dir`; tests keep their prints. | `lib.rs`, `commands.rs`, `service.rs`, `control.rs` |
| Locking | Added `sync::MutexExt::lock_or_recover` (recovers a poisoned lock, warns once) and replaced ~43 production `.lock().unwrap()` sites. A poisoned worker can no longer panic the next lock. | `sync.rs`, `discovery/tailscale/service/control/session/scores.rs` |
| Constants | New `constants.rs` for cross-cutting timeouts/intervals (discovery probe, TCP probe + clamp bounds, host wait, health, monitor). | `constants.rs` |
| Time | New `time.rs` with `now_ms()` and `utc_stamp()` (moved out of `logging.rs`), replacing three duplicate `now_ms` copies. | `time.rs`, `session.rs`, `service.rs`, `control.rs` |
| Config helpers | `load_config`, `provider_for`, `config_and_provider` replace ~15 repeated `Config::load(&config_file(..)?)` + `resolve_provider` blocks. | `commands.rs` |
| Provider seam | `resolve_launcher`/`resolve_provider` moved out of `commands.rs` into `provider.rs`; removed the thin `retroarch_provider` wrapper. | `provider.rs` |
| Module move | `launcher/retroarch.rs` → `retroarch.rs` (it is a `Provider`, not a `Launcher`). | `retroarch.rs` |
| Module split | Diagnostics extracted from the `commands.rs` god module into `diagnostics.rs`; `commands.rs` shrank ~780 → ~470 lines. | `diagnostics.rs` |
| Session API | `session::LaunchOptions` replaces five positional booleans on `launch`/`launch_many`. | `session.rs`, `commands.rs`, `service.rs` |
| Core path | `retroarch::managed_core_path` centralizes the managed-core layout; the live parity test now derives it cross-OS instead of hardcoding a macOS path. | `retroarch.rs` |
| Frontend | Split `lib/api.ts` into `lib/types.ts` (types + event names) and `lib/api.ts` (invoke wrappers, `export * from "./types"`); added `lib/hooks.ts` (`useTauriEvent`, `usePolling`, `useAsyncTask`) and collapsed the 17 `try/catch → setAppError` handlers and 4 listener/polling effects in `App.tsx`. | `frontend/src/lib/{types,api,hooks,utils}.ts`, `App.tsx` |

Conventions this established are recorded in `AGENTS.md` (log via `log`, lock via
`lock_or_recover`, shared magic values in `constants.rs` / `time.rs`).

## 2. Deferred — typed errors (`CabinetError`)

Current state: every fallible function returns `Result<T, String>`, and `?` across modules needs
manual `format!`/`map_err`. Proposal:

- Add `thiserror`, define `CabinetError` with variants (`Io`, `Json`, `Tauri`, `Tailscale`,
  `Launch`, …) and `From` impls for `std::io::Error` / `serde_json::Error`.
- Convert to `String` **only** at the Tauri command boundary (Tauri commands serialize their
  error; a newtype `CommandError(CabinetError)` implementing `Serialize` keeps the frontend
  contract unchanged).
- Rationale for deferring: it touches essentially every module and must stay clippy-clean on the
  3-OS matrix; the payoff is ergonomics, not a fixed bug.

## 3. Deferred — module layout

- **`providers/` tree.** `launcher/` currently holds the FightCade `Launcher` trait *and* the
  per-OS launchers, while `retroarch.rs` (moved to top level in §1) is a provider that imports
  `launch` contracts. A later pass should make the layering explicit:
  `providers/{mod.rs, fightcade.rs, retroarch/{mod.rs, spec.rs, parity.rs, core.rs}}`.
- **Split the two large files.** `retroarch.rs` (~800 lines) mixes spec building, parity, core
  provisioning, and tests; `windowing/macos.rs` (~740 lines) mixes the AX/CGWindow implementation
  with a large live-test module. Split implementation from tests/roles.
- **Consolidate per-OS launcher detection.** `launcher/{macos,linux,windows}.rs` repeat the same
  detect/emit shape; a shared trait helper would remove the duplication.

## 4. Deferred — shared contract types

`LaunchSpec` and `InstallInfo` live in `launcher/mod.rs` but are consumed by the provider layer.
Moving them to `provider.rs` (or a neutral `contracts` module) removes the provider→launcher
import for these two types. Deferred because it inverts a dependency edge and is pure churn until
§3 lands.

## 5. Deferred — frontend data layer

`App.tsx` still owns all fetching, polling, and error state. A query/state layer (React Query, or
a small `useInvoke` cache) would remove prop-drilling and the manual `sameJson` de-duplication,
and would give retries/staleness for free. `lib/hooks.ts` is the seam this would build on.

## 6. Deferred — logging & diagnostics follow-ups

`docs/07-retroarch-spike.md` §14 already tracks the live-run findings (F1–F20). Of those, these
belong to a logging/diagnostics cleanup:

| Ref | Task | Status |
|---|---|---|
| F4 | Gate `--verbose` behind `verboseLogging`; timestamp captured emulator lines instead of writing them raw. | **Done 2026-09-21** |
| F5 | Session dirs are UTC while the app log is local; unify or record the offset. | **Done 2026-09-21** (app log, capture, and session dirs all UTC) |
| F14 | Include the latest session's `emulator-*.log` tail in the diagnostics bundle. | **Done 2026-09-21** |
| F15 | Redact tailnet IPs / home paths from the diagnostics bundle (repo is public). | **Done 2026-09-21** (structured sections redacted; raw log tail carries a warning) |
| F16 | Prune session dirs / `emulator-*.log` (only the app log rotates today). | **Done 2026-09-21** (`SESSION_KEEP` in `constants.rs`) |

## 7. Deferred — `time` crate

`time.rs` hand-rolls the civil-date conversion. Replacing it with the `time` crate removes ~25
lines and a class of date bugs, at the cost of a new dependency. Deferred under the project's
"avoid new deps casually" stance.

## 8. Constraints and verification

- Repo is **public**: placeholders only (`100.x.x.x`), never commit logs, cores, or ROMs.
- Any change must pass `./scripts/test.sh` (frontend build + `cargo fmt --check` + `cargo test` +
  `cargo clippy --all-targets -- -D warnings`) on macOS, Linux, and Windows.
- Prefer `git mv` for moves so history is preserved.

## 9. Pointers

- Spec: `docs/04-design.md` §3 (decisions) and §4 (architecture).
- Live-run findings: `docs/07-retroarch-spike.md` §14.
- Conventions: `AGENTS.md`.
