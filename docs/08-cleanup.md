# Cleanup: applied refactors and deferred proposals

Status: **applied pass done 2026-09-20** (balanced, behavior-preserving). A **second pass done
2026-09-21** cleared the post-removal drift (docs, `uninstall-linux.sh`), removed dead frontend/Rust
code, deleted the two stale `handoff-*.md` docs, added `cargo fmt --check` to the gate, and applied
the logging/diagnostics follow-ups (§6 F4/F5/F14/F15/F16). A **third pass done 2026-09-21** applied
the structural refactors — the `providers/` tree, the neutral `contracts` module, and the
`retroarch.rs` / `windowing/macos.rs` splits plus the shared launcher helpers (§3, §4). A **fourth
pass done 2026-09-21** landed the frontend query layer (§5), completed the identifier genericization
(with the leaks the 2026-09-19 scrub missed), extended diagnostics redaction, and applied the
smaller convention fixes. This file is the backlog for a future cleanup session, so day-to-day
feature work has a written target instead of ad-hoc churn. A **fifth pass done 2026-09-21** landed
the two deferred heavy refactors — typed errors (§2) and the `time` crate (§7) — plus the
oversized-file splits for `tailscale`, `diagnostics`, `windowing/macos`, `launcher/linux`, and
`commands`, the backend env/PATH/peer-override dedups, and the remaining frontend query/component
cleanup. Only a few low-value items remain deferred (see the statuses below).

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

## 2. Applied (2026-09-21) — typed errors (`CabinetError`)

Landed with `thiserror`:

- `error.rs` defines `CabinetError` (`Message`, `Tailscale`, `Launch`, `Io`, `Json`, `Tauri`,
  `Opener`) with `From` impls for `std::io::Error` / `serde_json::Error` / `tauri::Error` /
  `tauri_plugin_opener::Error` plus `From<String>` / `From<&str>` (the last two map to `Message`, so
  existing `Err(format!(…))` sites stay short). `Result<T>` aliases `Result<T, CabinetError>`.
- Every internal signature (88 `Result<_, String>` sites across all modules) and the `Launcher` /
  `Provider` / `WindowHost` traits now return `CabinetError`.
- The `#[tauri::command]` boundary uses `CommandResult<T>` = `Result<T, CommandError>`, where
  `CommandError(CabinetError)` implements `Serialize` by writing the `Display` string — so the
  frontend contract (a rejected promise carrying a string) is unchanged.
- `?` now converts io/json/tauri errors directly; user-facing strings are preserved (contextual
  `map_err(format!(…))` stays as `Message`), so no log or UI message text changed.

## 3. Applied (2026-09-21) — module layout

The provider/launcher layering is now explicit. `src-tauri/src/` reads:

```
src-tauri/src/
├─ contracts.rs          InstallInfo, LaunchSpec, PeerOverride (neutral; no launcher/provider deps)
├─ error.rs              CabinetError, CommandError, Result/CommandResult aliases
├─ env.rs                home_dir(), on_path()/path_program_on_path()
├─ providers/
│  ├─ mod.rs             Provider trait, ProviderKind, Role, Capabilities, MatchRequest, resolve_provider
│  ├─ fightcade.rs       FightCadeProvider + per-OS resolve_launcher
│  └─ retroarch/
│     ├─ mod.rs          RetroArchProvider + Provider impl
│     ├─ core.rs         platform/core identity + path resolution
│     ├─ parity.rs       ParityStatus, frozen constants, sha256/core-git
│     ├─ spec.rs         overrides writing + launch args/spec
│     └─ test_support.rs #[cfg(test)] Scratch/provider/request fixtures
├─ launcher/{mod,macos,windows}.rs, launcher/linux/{mod,detect,layouts}.rs
├─ commands/{mod,config,launch,cabinet}.rs
├─ diagnostics/{mod,redact,bundle}.rs
├─ tailscale/{mod,status,ping}.rs
└─ windowing/{mod,linux,windows}.rs, windowing/macos/{mod,enumerate,place,tests}.rs
```

- **Large-file splits (fifth pass).** `tailscale.rs` → `{status,ping}` (types/binary resolution in
  `mod`); `diagnostics.rs` → `{redact,bundle}` (commands in `mod`); `windowing/macos/mod.rs` →
  `{enumerate,place}` (AX placement vs CGWindowList enumeration); `launcher/linux.rs` →
  `{detect,layouts}` (the launch-spec builders moved to `layouts`); `commands.rs` →
  `{config,launch,cabinet}` (all `#[tauri::command]` fns live beside their helpers, and `lib.rs`
  references the full paths, since Tauri's generated command macros are not re-exportable).
- **Dedups (fifth pass).** `home_dir()` and the PATH probes collapsed into `env.rs`; the per-launcher
  `peer_override: Option<String>` + `loopback()` pattern collapsed into `contracts::PeerOverride`
  (shared by the three launchers and `RetroArchProvider`). The per-module test `Scratch`/temp-dir
  scaffolding is **still duplicated** — low value, deferred.

- **`providers/` tree.** `provider.rs` → `providers/mod.rs` and `retroarch.rs` →
  `providers/retroarch/`. `contracts.rs` is top-level (not `providers/contracts.rs`) so the edge
  stays `contracts ← launcher ← providers`.
- **Large-file splits.** `retroarch.rs` split into `core`/`parity`/`spec`/`mod`, tests beside their
  subjects; `windowing/macos.rs` split into `windowing/macos/{mod,tests}.rs`.
- **Per-OS launcher detection consolidated.** `Launcher::info(installed, detail)` provides the
  shared `InstallInfo` shape; `MatchConfig::quark_arg_overriding(peer)` replaces the duplicated
  `quark()` in the Linux/Windows launchers and the inline clone in macOS; all three now share
  `loopback(self)`.

## 4. Applied (2026-09-21) — shared contract types

`InstallInfo` and `LaunchSpec` moved out of `launcher/mod.rs` into a neutral `contracts.rs`;
`launcher`, `providers`, `commands`, and `session` import them from there. This removes the
provider→launcher import for the two contract types. The planned `providers/contracts.rs` was
rejected: it would make `launcher → providers`, inverting the edge this refactor removes, so
`contracts.rs` sits at the crate root instead.

## 5. Applied (2026-09-21) — frontend data layer

`App.tsx` used to own all fetching, polling, and error state, with manual `sameJson` de-duplication
and a `refreshX` callback per resource. The new `frontend/src/lib/query.ts` exposes a small
`useInvoke<T>(key, fetcher, { enabled, pollMs })` hook built on `lib/hooks.ts`:

- a module-level result cache keyed by string, with JSON de-duplication so an unchanged payload does
  not re-render (replacing the hand-written `sameJson` calls in `App.tsx`; `sameJson` now lives in
  `lib/utils.ts` and is used only by the cache),
- `enabled` for conditional queries (health runs only when peers are online; its key includes the
  online IPs so it reloads when the set changes),
- `pollMs` for optional background polling via `usePolling` (silent refresh),
- `refresh()` for explicit reloads and `mutate()` for local updates (used after saving config).

`App.tsx` now consumes five queries (config, peers, ROMs, launcher, health) instead of a dozen
`useState`/`useCallback` pairs; the Refresh button and post-save flow call the queries' `refresh`.
No dependency was added (React Query remains the heavier alternative). Frontend gate: `npm run build`.

## 5b. Applied (2026-09-21) — fourth pass (identifiers, redaction, conventions)

| Area | Change | Where |
|---|---|---|
| Identifiers | Genericized docs/scripts/test fixtures to the sanctioned placeholder set (`AGENTS.md`); fixed three real leaks the 2026-09-19 scrub missed — two real machine hostnames in the Tailscale test fixture and a real account handle in the config tests (values intentionally not reproduced here). | `docs/*`, `README.md`, `AGENTS.md`, `scripts/fcade-lan-*`, `tailscale.rs`, `config.rs` |
| Redaction | `diagnostics::redact` now also strips IPv6, `*.ts.net` MagicDNS names, and the configured `handle`/`defaultPeerIp`; tests added. | `diagnostics.rs` |
| Constants | `TAILSCALE_STATUS_CACHE_TTL`, probe attempt/retry intervals, and the diagnostics session/log-tail limits moved to `constants.rs`. | `constants.rs`, `tailscale.rs`, `probe.rs`, `diagnostics.rs` |
| Locking | The last raw `.lock().unwrap_or_else(...)` sites (macOS window host) and the silent `if let Ok(..) = sink.lock()` in `capture_stream` now use `lock_or_recover`. | `windowing/macos/mod.rs`, `logging.rs` |
| Visibility / dead code | `RetroArchProvider` re-export narrowed to `pub(crate)`; the `windowing/mod.rs` `#[allow(dead_code)]` blanket scoped per-platform via `cfg_attr`. | `providers/mod.rs`, `windowing/mod.rs` |
| Provider seam | `Provider::requires_rom_file()` replaces the `ProviderKind::Retroarch` branch in `commands::optional_rom_file`; `lib::verbose_logging_requested` reuses `commands::load_config`. | `providers/mod.rs`, `providers/retroarch/mod.rs`, `commands.rs`, `lib.rs` |
| Settings | The vestigial `handle` field is relabeled **Default nickname** (it is only the RetroArch nickname fallback now). | `frontend/src/components/SettingsDialog.tsx` |

Verification: `./scripts/test.sh` (frontend build + `cargo fmt --check` + `cargo test` + clippy `-D warnings`), mirrored by the 3-OS `.github/workflows/ci.yml` matrix.

## 5c. Applied (2026-09-21) — fifth pass (frontend)

| Area | Change | Where |
|---|---|---|
| Query layer | The one-off `matchStatus().then(setMatch)` effect became a `useInvoke("match", matchStatus)` query; `MATCH_EVENT` now calls `matchQuery.mutate` instead of a bare `setState`. | `App.tsx` |
| Parity | `LaunchCard`'s manual `cancelled`-flag parity fetch became `useInvoke(\`parity:${rom}:${kind}\`, …)` with `enabled` gating. | `LaunchCard.tsx` |
| Polling | `MatchView`'s two raw `setInterval` loops now use `usePolling` (the re-assert interval is enabled only while a window is attached). | `MatchView.tsx` |
| Splits | Presentational components extracted: `LaunchWarningDialog`, `MatchStatusPanel`, `RetroArchSettings`, `DiagnosticsSection` (each 46–75 lines). | `components/*` |
| API/types | `lib/api.ts` no longer re-exports types; type-only imports now come from `lib/types` directly. `lib/utils.ts` (`sameJson`) folded into `lib/query.ts` and deleted. | `lib/*`, `App.tsx`, `components/*` |

## 6. Applied (2026-09-21) — logging & diagnostics follow-ups

`docs/07-retroarch-spike.md` §14 already tracks the live-run findings (F1–F20). Of those, these
belong to a logging/diagnostics cleanup:

| Ref | Task | Status |
|---|---|---|
| F4 | Gate `--verbose` behind `verboseLogging`; timestamp captured emulator lines instead of writing them raw. | **Done 2026-09-21** |
| F5 | Session dirs are UTC while the app log is local; unify or record the offset. | **Done 2026-09-21** (app log, capture, and session dirs all UTC) |
| F14 | Include the latest session's `emulator-*.log` tail in the diagnostics bundle. | **Done 2026-09-21** |
| F15 | Redact tailnet IPs / home paths from the diagnostics bundle (repo is public). | **Done 2026-09-21** (structured sections redacted — home paths, IPv4/IPv6, `*.ts.net`, configured handle/peer; raw log tail carries a warning) |
| F16 | Prune session dirs / `emulator-*.log` (only the app log rotates today). | **Done 2026-09-21** (`SESSION_KEEP` in `constants.rs`) |

F12 (the stale `handoff-*.md` docs) was resolved in the 2026-09-21 cleanup by deleting both handoffs;
the F-table row in `docs/07-retroarch-spike.md` §14 is marked done.

## 7. Applied (2026-09-21) — `time` crate

`time.rs`'s hand-rolled `civil_from_days` (Hinnant algorithm) is gone; `now_ms()` and `utc_stamp()`
now use `time::OffsetDateTime::now_utc()` and `time::macros::format_description!`, keeping the same
`YYYYMMDD-HHMMSS` shape and the same call sites (`session`, `logging`, `diagnostics`). `time` is
already in the lockfile as a transitive dependency, so the direct dependency adds no new tree.

## 8. Constraints and verification

- Repo is **public**: use only the sanctioned placeholders in `AGENTS.md`, never commit logs, cores, or ROMs.
- Any change must pass `./scripts/test.sh` (frontend build + `cargo fmt --check` + `cargo test` +
  `cargo clippy --all-targets -- -D warnings`) on macOS, Linux, and Windows.
- Prefer `git mv` for moves so history is preserved.

## 9. Pointers

- Spec: `docs/04-design.md` §3 (decisions) and §4 (architecture).
- Live-run findings: `docs/07-retroarch-spike.md` §14.
- Conventions: `AGENTS.md`.
