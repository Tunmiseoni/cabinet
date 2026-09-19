# cabinet

A cross-platform desktop lobby for playing **FightCade 2 FBNeo** games with friends over a **Tailscale** tailnet. It drives the emulator's built-in `quark:direct` mode, so matches connect peer-to-peer and bypass ISP CGNAT entirely — no FightCade matchmaking, no port forwarding, no paid VPN.

The network problem this solves, and the full design, are documented in [`docs/04-design.md`](docs/04-design.md) (the historical troubleshooting record is `docs/01`–`03`).

## Status

**Phase 1 (macOS-first) is implemented.** Current app:

- Peer registry and per-peer **connection health** — RTT, `direct` vs `DERP (relay)`, with a pre-match warning when a peer is relayed or above the latency threshold.
- ROM index from the configured emulator's ROM directory.
- **Launcher** for macOS FightCade (bundled Wine) and **Linux FightCade** (Flatpak `com.fightcade.Fightcade`, or a native/Wine install) with spawn/stop and process-exit detection.
- **Match-result detection** — polls the emulator's `fbneo/fightcade/` overlay files (`winner.txt`, scores, characters) during and after a session and surfaces the result in the launcher card. Requires `bVidSaveOverlayFiles 1` in the FightCade FBNeo config.
- **Lifetime scores** — a local per-opponent win/loss ledger (plus overall totals, win rate, and streaks), persisted to the app config dir. Game counts come from overlay score increments; loopback Dev-pair games are excluded. Resettable from Settings.
- **Rooms (Phase 2)** — host a king-of-the-hill room or discover and join a peer's room over the tailnet (UDP discovery + a secret-gated TCP control channel). The room card shows champion/challenger/queue and the host-persisted shared scoreboard, and the app auto-launches your `quark:direct` match when the host assigns it to you. Results are auto-reported from the emulator overlay, with manual "I won"/"I lost" buttons as fallback; the match peer's RTT/path is re-pinged during play.
- A **Dev pair** button that starts both sides on `127.0.0.1` for single-machine testing.

**Priorities.** The next blocker is a convenient way to get builds onto the other machines and push updates iteratively (`docs/04-design.md` §7); the 4-person live test (Phase 2.7) and further multi-machine testing are deferred until that is solved. After delivery: spectating (Phase 3, gated on a RetroArch spike) and the Windows launcher adapter.

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
git clone <repo> cabinet && cd cabinet   # or copy the folder over
./scripts/setup-linux.sh                 # deps + production bundle
./scripts/setup-linux.sh --dev           # deps + run in dev mode
```

The bundle is written under `src-tauri/target/release/bundle/`. The app detects
the FightCade Flatpak (`com.fightcade.Fightcade`) automatically and launches
FBNeo inside its sandbox; a native/Wine install is used as a fallback, and the
ROM directory is auto-detected. If detection fails, set the FightCade directory
in Settings. Ensure Tailscale is running and ports `47810/47811` (UDP/TCP) are
allowed through the local firewall.

## Repository layout

```
cabinet/
├─ frontend/     Vite + React + TypeScript + Tailwind v4 + shadcn/ui (alias @/* -> frontend/src/*)
├─ src-tauri/    Rust backend (Tauri v2)
│  └─ src/       config, tailscale, roms, launcher, session, commands
├─ scripts/      helper scripts + the original per-OS FightCade launchers
└─ docs/         investigation record (01–03) and current spec (04)
```

The shell launchers in `scripts/` (`fcade-lan-macos.sh`, `fcade-lan-linux.sh`, `fcade-lan-windows.bat`) are the reference implementation the app reproduces.

## Notes

- The repository is **private** for now. Before making it public, follow the publication checklist in [`docs/04-design.md`](docs/04-design.md) — tailnet/public/LAN addresses and account handles must be scrubbed from git history.
- Never commit `target/`, `node_modules/`, emulator binaries, ROMs, or `.env*`.
