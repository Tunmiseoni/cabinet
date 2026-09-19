# cabinet

A cross-platform desktop lobby for playing **FightCade 2 FBNeo** games with friends over a **Tailscale** tailnet. It drives the emulator's built-in `quark:direct` mode, so matches connect peer-to-peer and bypass ISP CGNAT entirely — no FightCade matchmaking, no port forwarding, no paid VPN.

The network problem this solves, and the full design, are documented in [`docs/04-design.md`](docs/04-design.md) (the historical troubleshooting record is `docs/01`–`03`).

## Status

**Phase 1 (macOS-first) is implemented.** Current app:

- Peer registry and per-peer **connection health** — RTT, `direct` vs `DERP (relay)`, with a pre-match warning when a peer is relayed or above the latency threshold.
- ROM index from the configured emulator's ROM directory.
- **Launcher** for macOS FightCade (bundled Wine) with spawn/stop and process-exit detection.
- **Match-result detection** — polls the emulator's `fbneo/fightcade/` overlay files (`winner.txt`, scores, characters) during and after a session and surfaces the result in the launcher card. Requires `bVidSaveOverlayFiles 1` in the FightCade FBNeo config.
- **Lifetime scores** — a local per-opponent win/loss ledger (plus overall totals, win rate, and streaks), persisted to the app config dir. Game counts come from overlay score increments; loopback Dev-pair games are excluded. Resettable from Settings.
- A **Dev pair** button that starts both sides on `127.0.0.1` for single-machine testing.

Still to come: room discovery + king-of-the-hill + score ledger (Phase 2), spectating (Phase 3, gated on a RetroArch spike), packaging (Phase 4). Linux/Windows launcher adapters are not implemented yet — non-macOS builds return an error.

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
| `scripts/test.sh` | Frontend typecheck/build, `cargo test`, `cargo clippy -D warnings` |
| `scripts/clean.sh` | Remove build artifacts (`--deps` also removes `node_modules`; `--wine` stops stray Wine/emulator processes) |

Opt-in tests that touch real hardware (Tailscale, ROM dir, emulator launch) are ignored by default:

```sh
(cd src-tauri && cargo test -- --ignored --nocapture)
```

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
