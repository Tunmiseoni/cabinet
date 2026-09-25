# The Cabinet

A cross-platform desktop app for playing games with friends over a **Tailscale** tailnet. It coordinates a shared session — discovering peers, showing connection health, and launching/connecting the emulator — so players connect peer-to-peer and bypass ISP CGNAT entirely, with no port forwarding and no paid VPN.

The design is documented in [`docs/design-general-emulation.md`](docs/design-general-emulation.md) (the agreed re-scope to a general multi-game session coordinator) and [`docs/04-design.md`](docs/04-design.md) (the earlier FightCade-era spec, whose scope is superseded). The historical troubleshooting record is `docs/01`–`03`.

**FightCade is no longer used.** The app drives RetroArch netplay directly — host, client, and spectator — with the frozen FBNeo core downloaded in-app. ROMs come from the directory set in Settings.

## Status

**Phase 1 (macOS + Linux + Windows launchers, connection health), the RetroArch spectator provider, Cabinet mode, and the GitHub Releases distribution pipeline are implemented.** Current app:

- Peer registry and per-peer **connection health** — RTT, `direct` vs `DERP (relay)`, with a pre-match warning when a peer is relayed or above the latency threshold.
- ROM index from the configured ROM directory.
- **Lobby (preferred path)** — host a RetroArch room (ROM + first-to-N fixed for its lifetime) or discover/join one on the tailnet, playing or watching as capacity allows. Manual `host-ip` join is the fallback when discovery is blocked. A **Stop match** control is always available while a session runs, including for joiners.
- **RetroArch launcher** — host/client/spectator netplay with spawn/stop and process-exit detection. Direct `Launch match` is a **developer-mode** diagnostic only (the card is hidden unless Developer mode is on).
- **Dev pair** button (developer mode) that starts both sides on `127.0.0.1` for single-machine testing.
- **RetroArch provider** — drives RetroArch netplay for host/client/**spectator**, with a parity gate on the frozen FBNeo core + ROM. Verified live across macOS/Linux/Windows (2026-09-20) at ~55–83 ms. Settings adds a netplay **max-ping cap**, **spectator mute** (on), an opt-in **isolated session config**, and an editable **keyboard preset** (on) that binds a 6-button fighting-game layout to FBNeo's Classic RetroPad for Cabinet launches and disables the RetroArch hotkeys that collide with it; a **Download frozen core** button fetches and sha256-verifies the frozen core for your OS. See [`docs/07-retroarch-spike.md`](docs/07-retroarch-spike.md).
- **Cabinet mode (off by default, macOS)** — with the setting enabled, launching a match opens a full-screen cabinet bezel and hosts the emulator window inside it via the macOS Accessibility API. Needs a one-time Accessibility grant; without it the game stays a separate window. See [`docs/06-redesign.md`](docs/06-redesign.md).


## Download

Installers are published to [GitHub Releases](https://github.com/Tunmiseoni/the-cabinet/releases): a `.dmg` for Apple Silicon macOS, a `-setup.exe` (NSIS) for Windows, and `.AppImage`/`.deb` for Linux. No GitHub account is needed.

The builds are **not notarized**:

- **macOS** — the app is ad-hoc signed (valid bundle signature), but Apple has not notarized it. A freshly downloaded copy is quarantined and shows **"unidentified developer"**: right-click the app and choose **Open**, or go to System Settings → Privacy & Security → **Open Anyway**. If you see "damaged and can't be opened" instead, the signature was stripped — run `xattr -cr "/Applications/The Cabinet.app"` and re-open.
- **Windows** — SmartScreen may warn; choose **More info → Run anyway**.

On Windows, allow inbound TCP on the RetroArch netplay port (default `55435`) so peers can connect.

## Priorities

The app is being re-scoped into a general multi-game session coordinator with three transport drivers (`native_online`, `netplay`, `stream`) and declarative per-game profiles. The agreed design is in [`docs/design-general-emulation.md`](docs/design-general-emulation.md); the first step — a full FightCade rip-out — is planned in [`docs/12-fightcade-ripout.md`](docs/12-fightcade-ripout.md).

RetroArch netplay/spectator is implemented and verified live on the tailnet (2026-09-20: a macOS host + Linux client + Windows spectator stayed synced ~11 min at ~55–83 ms, feel rated acceptable). The two-concurrent-spectator tailnet run is deferred. Recently landed: a netplay max-ping cap, default-on spectator mute, an opt-in isolated session config, an in-app frozen-core download backed by the published `retroarch-cores-v1` release, and an editable SF3 keyboard preset that neutralizes colliding RetroArch hotkeys for the session. See [`docs/07-retroarch-spike.md`](docs/07-retroarch-spike.md) for the open items.

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
| `scripts/clean.sh` | Remove build artifacts (`--deps` also removes `node_modules`) |

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

The bundle is written under `src-tauri/target/release/bundle/`. Set the ROM
directory in Settings (it defaults to `~/ROMs`). Ensure Tailscale is running and
the RetroArch netplay port (default `55435`) is allowed through the local
firewall.

Host tools are spawned with the AppImage's bundled `LD_LIBRARY_PATH` stripped, so
they load the system libraries rather than the bundle's copies.

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
alone if it predates the run).

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
│  └─ src/       config, contracts, tailscale, roms, providers, lobby, session, windowing, commands
├─ scripts/      helper scripts
└─ docs/         investigation record (01–03), the FightCade-era spec (04), the re-scope design, and plans
```

## Notes

- The repository is **public** at [`Tunmiseoni/the-cabinet`](https://github.com/Tunmiseoni/the-cabinet) to enable anonymous GitHub Releases. Its full history was scrubbed of tailnet/public/LAN addresses and account handles on 2026-09-19 (see `docs/04-design.md` §8), so the docs use placeholders.
- Never commit `target/`, `node_modules/`, emulator binaries, ROMs, or `.env*`.
