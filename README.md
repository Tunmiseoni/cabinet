# The Cabinet

A cross-platform desktop launcher for playing **FightCade 2 FBNeo** games with friends over a **Tailscale** tailnet. It drives the emulator's built-in `quark:direct` mode, so matches connect peer-to-peer and bypass ISP CGNAT entirely — no FightCade matchmaking, no port forwarding, no paid VPN.

The network problem this solves, and the full design, are documented in [`docs/04-design.md`](docs/04-design.md) (the historical troubleshooting record is `docs/01`–`03`).

**FightCade must be installed, but it is never launched.** The app bypasses the FightCade client entirely (no `fcade`, no matchmaking) and uses the installation only as the provider of the `quark:direct` emulator binary, Wine (macOS), the ROM directory, and `fcadefbneo.ini`. Uninstalling FightCade breaks the launchers. See [`docs/06-redesign.md`](docs/06-redesign.md) §2.

## Status

**Phase 1 (macOS + Linux + Windows launchers, connection health), the RetroArch spectator provider, Cabinet mode, and the GitHub Releases distribution pipeline are implemented.** Current app:

- Peer registry and per-peer **connection health** — RTT, `direct` vs `DERP (relay)`, with a pre-match warning when a peer is relayed or above the latency threshold.
- ROM index from the configured emulator's ROM directory.
- **Launcher** for macOS FightCade (bundled Wine), **Linux FightCade** (Flatpak `com.fightcade.Fightcade`, or a native/Wine install), and **Windows FightCade** (native `fcadefbneo.exe`) with spawn/stop and process-exit detection.
- A **Dev pair** button that starts both sides on `127.0.0.1` for single-machine testing.
- **RetroArch provider (Phase 0, opt-in)** — a selectable match provider beside FightCade that drives RetroArch netplay for host/client/**spectator**, with a parity gate on the frozen FBNeo core + ROM. Verified live across macOS/Linux/Windows (2026-09-20) at ~55–83 ms; this is the spectator path. Settings adds a netplay **max-ping cap**, **spectator mute** (on), and an opt-in **isolated session config**; a **Download frozen core** button fetches and sha256-verifies the frozen core for your OS. See [`docs/07-retroarch-spike.md`](docs/07-retroarch-spike.md).
- **Cabinet mode (off by default, macOS)** — with the setting enabled, launching a match opens a full-screen cabinet bezel and hosts the emulator window inside it via the macOS Accessibility API. Needs a one-time Accessibility grant; without it the game stays a separate window. See [`docs/06-redesign.md`](docs/06-redesign.md).


## Download

Installers are published to [GitHub Releases](https://github.com/Tunmiseoni/the-cabinet/releases): a `.dmg` for Apple Silicon macOS, a `-setup.exe` (NSIS) for Windows, and `.AppImage`/`.deb` for Linux. No GitHub account is needed.

The builds are **not notarized**:

- **macOS** — the app is ad-hoc signed (valid bundle signature), but Apple has not notarized it. A freshly downloaded copy is quarantined and shows **"unidentified developer"**: right-click the app and choose **Open**, or go to System Settings → Privacy & Security → **Open Anyway**. If you see "damaged and can't be opened" instead, the signature was stripped — run `xattr -cr "/Applications/The Cabinet.app"` and re-open.
- **Windows** — SmartScreen may warn; choose **More info → Run anyway**.

On Windows, run `scripts/fcade-lan-windows-firewall.bat` once (elevated) to allow inbound UDP for the emulator.

## Priorities

Spectating has cleared its Phase 0 gate — the RetroArch netplay/spectator spike passed live on the tailnet (2026-09-20: a macOS host + Linux client + Windows spectator stayed synced ~11 min at ~55–83 ms, feel rated acceptable), so RetroArch is the primary spectator path and the `ggponet.dll` shim is skipped. Phase 3 integration follows; the two-concurrent-spectator tailnet run is deferred. Recently landed: a netplay max-ping cap, default-on spectator mute (both live-verified on loopback), an opt-in isolated session config (loopback-retested; default stays off), and an in-app frozen-core download backed by the published `retroarch-cores-v1` release. See `docs/04-design.md` §7 and [`docs/07-retroarch-spike.md`](docs/07-retroarch-spike.md) for the open items.

A UX redesign is also proposed — **Cabinet mode**, hosting the emulator inside the app so a match is one window, plus a single-window information architecture. It is gated on a `/grill-me` session; the proposal, per-OS feasibility, and the required grill agenda are in [`docs/06-redesign.md`](docs/06-redesign.md).

## Quickstart

Prerequisite: Rust via the official installer (not Homebrew's `rustup`), plus Node/npm.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

npm install                      # root: Tauri CLI
npm install --prefix frontend    # frontend: React/Vite/Tailwind/shadcn
```

Run and build with the helper scripts (each sources the Rust env and installs deps if missing):

| Script | What it does |
|---|---|
| `scripts/dev.sh` | Run the app in development (Vite on `:1420` + Rust backend) |
| `scripts/build.sh` | Produce a production bundle for the current OS |
| `scripts/setup-linux.sh` | Linux one-shot: install system/Rust/JS deps, then build (`--dev` to run) |
| `scripts/uninstall-linux.sh` | Reverse a Linux source build: remove the repo dir, the deps it installed, and the Rust toolchain it added (dry-run by default; `--apply` to act) |
| `scripts/diagnose-linux.sh` | One-shot blank-window diagnosis for Linux: inspects the graphics stack and AppImage, tests the known launch workarounds (including the AppImage EGL fix), and writes `the-cabinet-report.txt` to the current directory (read-only; no sudo) |
| `scripts/test.sh` | Frontend typecheck/build, `cargo fmt --check`, `cargo test`, `cargo clippy -D warnings` |
| `scripts/clean.sh` | Remove build artifacts (`--deps` also removes `node_modules`; `--wine` stops stray Wine/emulator processes) |

Opt-in tests that touch real hardware (Tailscale, ROM dir, emulator launch) are ignored by default:

```sh
(cd src-tauri && cargo test -- --ignored --nocapture)
```

### Linux (CachyOS) friend — build and run

Tauri does not cross-compile cleanly, so build on the Linux machine. The helper
script installs the Tauri system dependencies, the Rust toolchain, and JS deps,
then builds:

```sh
git clone https://github.com/Tunmiseoni/the-cabinet.git && cd the-cabinet   # or copy the folder over
./scripts/setup-linux.sh                 # deps + production bundle
./scripts/setup-linux.sh --dev           # deps + run in dev mode
```

The bundle is written under `src-tauri/target/release/bundle/`. The app detects
the FightCade Flatpak (`com.fightcade.Fightcade`) automatically and launches
FBNeo inside its sandbox; a native/Wine install is used as a fallback, and the
ROM directory is auto-detected. If detection fails, set the FightCade directory
in Settings. Ensure Tailscale is running and the emulator's inbound netplay
traffic is allowed through the local firewall.

On Linux the app prefers the FightCade Flatpak (`com.fightcade.Fightcade`), falls
back to a native/Wine install (including `~/Games/*`), and can derive the install
from an already-detected ROM directory. Host tools are spawned with the AppImage's
bundled `LD_LIBRARY_PATH` stripped, so `flatpak`/`wine` load the system libraries.

Bundling applies a Linux-only post-build step: `scripts/patch-appimage.sh`
strips over-bundled Wayland/X11 libraries that break EGL on Mesa 25+ hosts
(`EGL_BAD_PARAMETER`) and repacks the AppImage. It is a no-op once upstream Tauri
excludes those libraries; the removal steps live in the script's header and in
[`docs/04-design.md`](docs/04-design.md) §3.

If the window opens blank/white, the app disables the WebKitGTK DMA-BUF
renderer automatically. If it still fails, run it from a terminal to see the
error and try the compositing fallback:

```sh
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./The.Cabinet_*_amd64.AppImage
```

If you would rather not diagnose it by hand, run the one-shot script — it tries
each workaround and writes a single report to send back:

```sh
curl -fsSLO https://raw.githubusercontent.com/Tunmiseoni/the-cabinet/main/scripts/diagnose-linux.sh
bash diagnose-linux.sh
```

The report is written to the directory you run the command from (`the-cabinet-report.txt`,
plus `the-cabinet-report-shots/` if a screenshot tool is available), and the exact
path is printed at the end.

A second, distinct blank-window cause is the AppImage bundler sweeping the build
machine's Wayland/X11 stack into the bundle. On Mesa 25+ systems (for example
CachyOS) `WebKitWebProcess` then aborts with
`Could not create default EGL display: EGL_BAD_PARAMETER` before any window
appears, and no `WEBKIT_*` flag helps. The script tests the fix for this too
(running the bundled app with those libraries removed). Upstream: tauri
[#15976](https://github.com/tauri-apps/tauri/issues/15976).

#### Switching a Linux source build to a release

Once released installers are available, the source build's toolchain is no
longer needed. `scripts/uninstall-linux.sh` removes only what the build
installed (it reconstructs that from `/var/log/pacman.log`, and leaves Rust
alone if it predates the run). It never touches the FightCade Flatpak.

```sh
curl -fsSLO https://raw.githubusercontent.com/Tunmiseoni/the-cabinet/main/scripts/uninstall-linux.sh
bash uninstall-linux.sh              # dry run: report what would go
bash uninstall-linux.sh --apply      # remove repo dir, added deps, Rust
bash uninstall-linux.sh --apply --install-appimage   # ...and install the release
```

If the repo directory is already gone, the script cannot infer when the build
ran; pass `--since <when>` (an ISObasic timestamp such as
`2026-09-19T03:00:00+0100`) so it will remove the Rust toolchain too, or
`--keep-rust` to leave Rust in place. Re-runs are safe: packages that are
already gone are skipped.

On Arch/CachyOS the release `.deb` does not apply — use the `.AppImage`. If
FUSE2 is missing, install `fuse2` or run the AppImage with
`--appimage-extract-and-run`.

## Repository layout

```
the-cabinet/
├─ frontend/     Vite + React + TypeScript + Tailwind v4 + shadcn/ui (alias @/* -> frontend/src/*)
├─ src-tauri/    Rust backend (Tauri v2)
│  └─ src/       config, contracts, tailscale, roms, launcher, providers, session, windowing, commands
├─ scripts/      helper scripts + the original per-OS FightCade launchers
└─ docs/         investigation record (01–03) and current spec (04)
```

The shell launchers in `scripts/` (`fcade-lan-macos.sh`, `fcade-lan-linux.sh`, `fcade-lan-windows.bat`) are the reference implementation the app reproduces.

## Notes

- The repository is **public** at [`Tunmiseoni/the-cabinet`](https://github.com/Tunmiseoni/the-cabinet) to enable anonymous GitHub Releases. Its full history was scrubbed of tailnet/public/LAN addresses and account handles on 2026-09-19 (see `docs/04-design.md` §8), so the docs use placeholders.
- Never commit `target/`, `node_modules/`, emulator binaries, ROMs, or `.env*`.
