# The Cabinet

A cross-platform desktop lobby for playing **FightCade 2 FBNeo** games with friends over a **Tailscale** tailnet. It drives the emulator's built-in `quark:direct` mode, so matches connect peer-to-peer and bypass ISP CGNAT entirely — no FightCade matchmaking, no port forwarding, no paid VPN.

The network problem this solves, and the full design, are documented in [`docs/04-design.md`](docs/04-design.md) (the historical troubleshooting record is `docs/01`–`03`).

## Status

**Phase 1 (macOS + Linux + Windows launchers, connection health, result watcher, local score ledger), Phase 2.1–2.6 (rooms/KotH, shared ledger, overlay auto-report, mid-match re-ping), and the GitHub Releases distribution pipeline are implemented.** Current app:

- Peer registry and per-peer **connection health** — RTT, `direct` vs `DERP (relay)`, with a pre-match warning when a peer is relayed or above the latency threshold.
- ROM index from the configured emulator's ROM directory.
- **Launcher** for macOS FightCade (bundled Wine), **Linux FightCade** (Flatpak `com.fightcade.Fightcade`, or a native/Wine install), and **Windows FightCade** (native `fcadefbneo.exe`) with spawn/stop and process-exit detection.
- **Match-result detection** — polls the emulator's `fbneo/fightcade/` overlay files (`winner.txt`, scores, characters) during and after a session and surfaces the result in the launcher card. Requires `bVidSaveOverlayFiles 1` in the FightCade FBNeo config.
- **Lifetime scores** — a local per-opponent win/loss ledger (plus overall totals, win rate, and streaks), persisted to the app config dir. Game counts come from overlay score increments; loopback Dev-pair games are excluded. Resettable from Settings.
- **Rooms (Phase 2)** — host a king-of-the-hill room or discover and join a peer's room over the tailnet (UDP discovery + a secret-gated TCP control channel). The room card shows champion/challenger/queue and the host-persisted shared scoreboard, and the app auto-launches your `quark:direct` match when the host assigns it to you. Results are auto-reported from the emulator overlay, with manual "I won"/"I lost" buttons as fallback; the match peer's RTT/path is re-pinged during play.
- A **Dev pair** button that starts both sides on `127.0.0.1` for single-machine testing.

## Download

Installers are published to [GitHub Releases](https://github.com/Tunmiseoni/the-cabinet/releases): a `.dmg` for Apple Silicon macOS, a `-setup.exe` (NSIS) for Windows, and `.AppImage`/`.deb` for Linux. No GitHub account is needed.

The builds are **unsigned**:

- **macOS** — a freshly downloaded app is quarantined by Gatekeeper and reports as **"damaged and can't be opened"**. Right-click → Open does *not* clear this; run `xattr -dr com.apple.quarantine "/Applications/The Cabinet.app"`, then open it normally.
- **Windows** — SmartScreen may warn; choose **More info → Run anyway**.

On Windows, run `scripts/fcade-lan-windows-firewall.bat` once (elevated) to allow inbound UDP for the emulator.

## Priorities

The next step is the 4-person live test (Phase 2.7): the GitHub Actions per-OS release pipeline and the **Windows launcher adapter** are now in place, so all four machines can play from released builds. After that: spectating (Phase 3, gated on a RetroArch spike). See `docs/04-design.md` §7 for the open items.

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
| `scripts/test.sh` | Frontend typecheck/build, `cargo test`, `cargo clippy -D warnings` |
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
in Settings. Ensure Tailscale is running and ports `47810/47811` (UDP/TCP) are
allowed through the local firewall.

If the window opens blank/white, the app disables the WebKitGTK DMA-BUF
renderer automatically. If it still fails, run it from a terminal to see the
error and try the compositing fallback:

```sh
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./The.Cabinet_*_amd64.AppImage
```

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
│  └─ src/       config, tailscale, roms, launcher, session, results, scores, player, discovery, control, room, service, commands
├─ scripts/      helper scripts + the original per-OS FightCade launchers
└─ docs/         investigation record (01–03) and current spec (04)
```

The shell launchers in `scripts/` (`fcade-lan-macos.sh`, `fcade-lan-linux.sh`, `fcade-lan-windows.bat`) are the reference implementation the app reproduces.

## Notes

- The repository is **public** at [`Tunmiseoni/the-cabinet`](https://github.com/Tunmiseoni/the-cabinet) to enable anonymous GitHub Releases. Its full history was scrubbed of tailnet/public/LAN addresses and account handles on 2026-09-19 (see `docs/04-design.md` §8), so the docs use placeholders.
- Never commit `target/`, `node_modules/`, emulator binaries, ROMs, or `.env*`.
