# AGENTS.md

Guidance for agents working in this repository.

## About this project

Research and tooling for playing **FightCade 2 FBNeo** games with friends over a **Tailscale** tailnet, bypassing ISP CGNAT with the emulator's `quark:direct` mode. The plan (see [`docs/04-design.md`](docs/04-design.md)) is a cross-platform Tauri app named **cabinet**, providing a lobby, king-of-the-hill queue, room discovery over the tailnet, per-player win/loss/draw tracking, and low-bandwidth input-relay spectating. Stack: Tauri v2 (Rust backend) + Vite web frontend.

## Important: the user uses speech-to-text

The user dictates prompts via speech-to-text transcription. Expect and tolerate:

- Homophones and near-miss words (e.g. "retro arc" = RetroArch, "guilty gear's Drive" = Guilty Gear Strive, "lan scripts" = LAN scripts).
- Dropped or repeated words, run-on sentences, and inconsistent casing/punctuation.
- Product names spelled differently across messages.

**Do not silently guess when a term is ambiguous.** If a mis-transcription would change the technical decision, state the interpretation you are using and ask for confirmation. Prefer confirming intent over building on a guessed meaning.

## Changes to the host machine (always ask first)

Downloads, installs, and large or long-term changes to the host machine are **allowed, but never without asking first**. This is a consent-and-awareness requirement, not a hard boundary — it is not meant to stop the work, only to make sure the user knows about it.

Ask before doing any of these:

- Downloading or installing software, toolchains, or packages (Rust, Tauri, system libraries, Homebrew/npm/cargo installs).
- Changes that persist beyond the session (services, launch agents, scheduled jobs, shell profile / `PATH` edits, mounted drives).
- Modifying files outside this repository (e.g. `/Applications/FightCade2.app`, the Wine prefix, system config).
- Anything large in size, network-heavy, or slow to reverse.

State what you intend to do, why, and how to undo it, then wait for confirmation. Prefer reversible, repo-local changes; when a global change is genuinely needed, say so explicitly.

## Repository layout

- `docs/` — investigation and design. `01`–`03` are the historical troubleshooting record; `04-design.md` is the current spec.
- `scripts/` — platform launchers (`fcade-lan-macos.sh`, `fcade-lan-linux.sh`, `fcade-lan-windows.bat`, `fcade-lan-windows-firewall.bat`).
- No build system yet; the scripts are currently the working implementation.

## Environment facts (observed)

- Primary machine: Apple M1 (arm64), 8 GB RAM, macOS 26.x.
- FightCade: `/Applications/FightCade2.app` (v2.1.45); FBNeo runs as a 32-bit PE under the bundled `wine32on64` (Rosetta 2).
- Wine prefix: `Contents/Resources/.wine32`; emulator cwd `Contents/MacOS/emulator/fbneo`.
- `quark:direct` arg format: `quark:direct,<rom>,<localPort>,<peerIP>,<peerPort>,<side>,0` with side 0 = P1 (local 7001 / peer 7000), side 1 = P2 (mirror).
- Tailnet is currently **direct, <100 ms** between peers after a router restart (earlier DERP-relay notes are stale; see amended `01`/`02`).
- Other machines: a CachyOS friend (FightCade Flatpak `com.fightcade.Fightcade`) and a Windows friend (native FightCade at `%APPDATA%\Fightcade`).

## Conventions

- Keep documentation in `docs/`; do not create new top-level notes.
- Prefer editing existing docs over creating new ones, except where `04-design.md` is the designated spec.
- Do not add comments to code unless asked.
- Match the existing shell-script style and the docs' use of tables.

## Environment setup

Rust via the official installer (not Homebrew's keg-only `rustup`, which conflicts with the `rust` formula):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo install tauri-cli --version '^2'
```

Node/npm (already present via nvm) is needed for the Vite frontend only. Per-OS system dependencies: see the Prerequisites subsection in [`docs/04-design.md`](docs/04-design.md).

## Testing

- There is no automated test suite. The platform launchers are the current verification surface.
- Direct-connect can be tested on a single machine over loopback (two player instances, plus a spectator once available).
- For manual end-to-end tests, at least two peers must be online on the tailnet.

## Git

- The repository is intended to be initialized as **`cabinet`** and kept **private** initially. Do not initialize, commit, or push until the user explicitly asks.
- Before making the repo public, follow the Publication checklist in [`docs/04-design.md`](docs/04-design.md) (tailnet IPs, public IPs, LAN IPs, account handles must be scrubbed from history).
- When committing later: commit `Cargo.lock` (this is an application, not a library); never commit `target/`, `node_modules/`, emulator binaries, ROMs, or `.env*`.
