# Design: cabinet — Tailnet FightCade lobby app (launcher + KotH + spectating)

Status: research complete, implementation to follow in a separate session.
Project name: **cabinet**. Git repository: **private** for now (see §8 Publication checklist).
Supersedes the "chosen route" framing in [`03-implementation-plan.md`](03-implementation-plan.md); the troubleshooting record (`01`–`03`) stays as history.

## TL;DR

- We already solved the hard network problem: `quark:direct` connects FBNeo peer-to-peer over Tailscale, bypassing FightCade matchmaking and the ISP CGNAT. Four friends have been playing this way all afternoon with a **direct, <100 ms** tailnet path.
- The proposed app is a **coordination layer**, not a FightCade clone. FightCade's server, `fcade` helper, and spectator relay are all closed; there is nothing to fork.
- The one genuinely hard feature is **input-relay spectating** (watch without video bandwidth). FightCade's spectator is server-brokered over a closed TCP protocol and is unreachable from `quark:direct`. **RetroArch netplay** provides the same low-bandwidth, input-based spectating natively, cross-platform, with no Wine or patched binaries.
- Decision: run a **one-day RetroArch spectator spike first**. If its netplay feels acceptable at <100 ms, make RetroArch the primary path and skip the risky `ggponet.dll` reverse-engineering entirely. Otherwise fall back to a time-boxed FightCade shim, then to "defer spectate".
- Features in scope: cross-platform Tauri lobby, **king of the hill**, room discovery over the tailnet, **per-player win/loss/draw tracking**.
- Stack: **Tauri v2 (Rust backend) + Vite web frontend** (see §3 and §6).

## 1. How FightCade works (established from the installed 2.1.45 client)

FightCade 2 is three layers:

| Layer | What it is | Open? |
|---|---|---|
| Electron shell | Nativefier wrapper around `web.fightcade.com` — chat/lobby UI | Closed (the server is the app) |
| `fcade` helper | PyInstaller binary: login, status, matchmaking, launches emulator | Closed |
| Emulator | `fcadefbneo.exe` (PE under bundled Wine on macOS) + `ggponet.dll` (GGPO rollback) | FBNeo/GGPO upstream open; FightCade fork/args closed |

Match flow: FightCade's **Quark** server introduces two players, they UDP hole-punch, then GGPO runs a direct session. CGNAT + symmetric NAT breaks the hole-punch, so the app hangs in chat. `quark:direct` skips Quark and hands GGPO explicit endpoints — the basis of the working scripts.

### Spectator architecture (key finding)

Static analysis of the installed binaries:

- `fcadefbneo.exe` supports `quark:direct`, `quark:served`, `quark:stream`, `quark:replay`, `quark:training`.
- `ggponet.dll` contains a `SpectatorBackend`, the export `ggpo_start_streaming`, and FightCade-only TCP code (`TcpProtocol`, `Sending streamed frame to quark`, `Received event %d from server`, `tcp.cpp`/`tcp_proto.cpp`).
- Imports from `ggponet.dll`: `ggpo_start_session`, `ggpo_start_streaming`, `ggpo_start_replay`, `ggpo_close_session`, `ggpo_idle`, `ggpo_synchronize_input`, `ggpo_get_stats`, `ggpo_advance_frame`, `ggpo_client_connect`, `ggpo_client_chat`. Exports add `ggpo_client_set_game_event`, `ggpo_set_frame_delay`, `ggpo_log`, `ggpo_logv`, `ggpo_start_synctest`, no `ggpo_start_spectating`.
- Upstream GGPO *does* support P2P spectating (`ggpo_start_spectating(..., host_ip, host_port)`; "any player may serve as host"), but FightCade replaced it with the server-side `ggpo_start_streaming`.

**Conclusion:** FightCade spectating = emulator opens TCP to FightCade's Quark server and is fed a stream. `quark:direct` has no spectator field and never contacts the server. True input-relay spectate therefore requires either (a) a `ggponet.dll` shim, or (b) a different netplay stack.

## 2. What RetroArch is (and why it matters)

RetroArch is an open-source multi-emulator frontend; it loads "cores", and **FBNeo is a core** — the same emulator engine, native on macOS/Windows/Linux, no Wine/Flatpak.

Its netplay (from official docs):

- TCP, server-authoritative. The server can be a player **or a non-playing spectator**.
- Up to 16 players and **many spectators**; "spectators send no input data" — they receive the stream and render locally. This is input-relay, **not video**.
- Replay/rollback: clients rewind/replay on delayed input. Requires identical core + content CRC + deterministic core.
- Per-packet cost ~28 bytes/frame (~1.7 KB/s per player); the server duplicates to spectators → 2 spectators ≈ ~5 KB/s total. Video would be ~1000× more.
- Spectator CPU cost is on the spectator's own machine (one extra emulator instance), not the host.
- Lobby is optional; manual "connect to host IP" with NAT traversal off is exactly right over Tailscale.

Tradeoff: FightCade uses dedicated UDP/GGPO and is generally regarded as better for fighting-game feel; RetroArch is TCP/replay (head-of-line blocking) but wins on spectating, cross-platform simplicity, and openness.

## 3. Decisions

- **App name: `cabinet`.** Git repository to be initialized under this name, **private** initially.
- **Stack: Tauri v2 + Vite.** Rust backend for process/socket/tailnet work; the UI is a Vite-built web frontend (HTML/CSS/JS) rendered in the OS webview. Node/npm is used only for the frontend build, not the backend.
- **Rust toolchain via `rustup`** (not Homebrew's keg-only `rustup`, which conflicts with the `rust` formula). See §6 Prerequisites.
- **Architecture: coordination layer, not a FightCade clone.** Reuse `quark:direct` and the existing scripts; the app orchestrates.
- **Do not clone/mod FightCade.** Server, helper, and spectator relay are closed; `github.com/Fightcade` has no public repos.
- **RetroArch is a co-equal candidate, not a consolation fallback.** Its spectator support is input-based and free.
- **Phase 0 gate = one-day RetroArch spectator spike.** If netplay feel is acceptable at <100 ms, make RetroArch primary and skip `ggponet` RE. If not, attempt the shim (hard time-box), then fall back to deferring spectate.
- **Keep the FightCade install** for 2-player play regardless; if the shim fails we simply lose true spectate on that path.
- **Room/KotH authority: host-authoritative, any of the four machines may host.** No always-on node.
- **Two distinct roles:** room/KotH authority (any machine) vs. match/stream source (necessarily one of the two competitors, since `quark:direct` is strictly peer-to-peer).
- **Discovery: peer-scan, no registry.** Each app reads peers from `tailscale status --json` and probes a fixed UDP discovery port; hosts answer with room info. The room list is empty when nobody hosts.
- **All four machines run the same build.** If a shim is used, all four carry the shimmed emulator (any of them may be the serving player).
- **Spectator acceptance bar: 2 concurrent spectators** (4-person group: 2 play, 2 watch).
- **Match definition: best-of-N.** Winner comes from the overlay `winner.txt` if it proves to work in direct mode; otherwise emulator-exit + a manual "I won" button, with disputes resolved by the room host.
- **Score tracking: per-player wins/losses/draws**, persisted by the room host (see §5).
- **Development harness:** abstract "launch an emulator instance" so dev can use isolated/native instances instead of three Wine instances sharing one `WINEPREFIX`.
- **Shim constraints (if used):** pin FightCade 2.1.45, disable auto-update, run Linux from a writable copy of `fbneo/`, and re-sign the macOS `.app` after replacing `ggponet.dll`.
- **Docs:** this file is the spec; `01`–`03` are amended for the direct-path finding and kept as history; `AGENTS.md` records conventions and setup.
- **Repository visibility:** private until the publication checklist in §8 is satisfied.

## 4. Architecture

```
cabinet
├─ frontend/           -> Vite web UI (lobby, ladder, scoreboard, spectator list)
└─ src-tauri/          -> Rust backend
   ├─ peer registry    -> `tailscale status --json` (stable 100.x / MagicDNS)
   ├─ room discovery   -> fixed UDP port probe across tailnet peers
   ├─ ROM index        -> scan the configured emulator's ROM dir
   ├─ session launcher -> port of scripts/fcade-lan-{macos,linux,windows}
   ├─ match supervisor -> watches process exit + fbneo/fightcade/*.txt
   ├─ room authority   -> host-authoritative queue + KotH ladder (P2P, no infra)
   └─ score ledger     -> wins/losses/draws per player (room-host persisted)
```

Launcher abstraction covers: macOS Wine (`wine32on64` + `.wine32`), Linux Flatpak (`com.fightcade.Fightcade`) or native, Windows native, and a native/isolated mode for development.

## 5. King of the Hill and score tracking

State machine held by the room host:

- Room state: `champion`, `challenger`, `queue[]`, `currentMatch`.
- On match end: detect winner (overlay file / exit / manual), mutate the ladder (winner stays), promote the next challenger, launch the next `quark:direct` pair.
- Roles per match: assign P1/P2 (side 0 / side 1).

Per-player ledger:

- Fields: `wins`, `losses`, `draws`, `games_played`, optional `current_streak` / `best_streak`.
- Storage: room-host local store (JSON or SQLite) so scores survive sessions and can be shown in the lobby.
- Result sources, in order of preference:
  1. `fbneo/fightcade/winner.txt` (+ `p1score.txt`/`p2score.txt`) when `bVidSaveOverlayFiles 1` — **must be verified in direct mode** (see Open Questions).
  2. Emulator exit + per-player confirm.
  3. Manual dispute resolution by the room host.
- Draw handling: a draw is recorded when neither player is a winner (double KO / timeout with equal rounds). Exact signal to be determined in the spike; if no signal exists, offer a "Draw" button alongside "I won".

## 6. Phased plan

| Phase | Work | Gate |
|---|---|---|
| 0 | RetroArch spectator spike: host + 1 client + 2 spectators over Tailscale; measure feel and bandwidth | Feel acceptable at <100 ms? |
| 0b | If not: FightCade `ggponet.dll` shim spike (static RE of `quark:stream` arg semantics, then a minimal shim) | Hard time-box; if the `quark:stream` middle token is a server session id, stop |
| 1 | Tauri launcher: peer registry, ROM index, side/role, spawn/teardown, Wine/Flatpak/native | Replaces scripts with equivalent behavior |
| 2 | Room + KotH + score ledger; auto-launch next pair; result detection | 4-person session end-to-end |
| 3 | Spectate integration via the Phase 0 outcome; else documented as deferred | 2 concurrent spectators |
| 4 | Docs + packaging | Reproducible on all four machines |

### Prerequisites (do once per machine)

Rust (via the official installer, not Homebrew):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
cargo install tauri-cli --version '^2'
```

Node/npm is needed for the Vite frontend only. Tauri's `create-tauri-app` (or `cargo tauri init`) wires the two together.

Per-OS system dependencies (not Rust crates; Cargo cannot fetch these):

| OS | Required |
|---|---|
| macOS (this machine) | Xcode Command Line Tools — already installed at `/Library/Developer/CommandLineTools` |
| Windows (native friend) | MSVC C++ Build Tools + WebView2 runtime (ships with Win10/11); NSIS/WiX only for installers |
| Linux (CachyOS friend) | `libwebkit2gtk-4.1-dev`, `build-essential`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` |

Cross-compiling Tauri across OSes is painful; each of the four machines builds its own binary. Test an early build on the CachyOS machine first — WebKitGTK is the least predictable webview.

## 7. Open questions

- Does `bVidSaveOverlayFiles 1` actually write `winner.txt`/scores in **direct** mode, or does it need FightCade match metadata? (Decides automatic KotH and auto-scoring.)
- `quark:stream` argument semantics: is the middle token a host IP or a server session id?
- Is RetroArch FBNeo netplay feel acceptable for fighting games at <100 ms?
- Multi-spectator fan-out ceiling and whether each spectator needs its own inbound UDP port / firewall rule.
- What exactly constitutes a "draw" signal from the emulator, if any.
- macOS: signature/quarantine handling after replacing `ggponet.dll`; behavior when FightCade auto-updates.
- RetroArch path: ROM/core parity and content-CRC matching across the four machines, and whether FBNeo core serialization is enabled.

## 8. Risks and tradeoffs

- A `ggponet.dll` shim is tied to FightCade 2.1.45 and will break on update; must pin and disable auto-update.
- Modifying a signed macOS `.app` requires re-signing; the Linux Flatpak `/app` is read-only (needs a writable copy).
- RetroArch's TCP/replay netplay may feel worse than FightCade's UDP/GGPO for fighting games.
- The 8 GB M1 dev harness (Chrome + editor + multiple emulator instances) may swap; use native/isolated instances.
- Room-host authority means the ladder pauses if the host leaves; acceptable per "anybody can host".
- Tailscale ACLs + per-OS firewall rules are prerequisites for discovery/spectating.
- Tailscale Personal free tier allows up to 6 users (unlimited devices) — the 4-person group is within limits.

### Publication checklist (repo is private for now)

Before making `cabinet` public, scrub the following from **all files and from git history** (deleting them in a later commit is not enough — use `git filter-repo` or BFG before any public push):

- Tailscale addresses/hostnames: `100.64.0.10`, `100.64.0.11`, `100.64.0.12`, `mac-host`, `cachyos-host`
- Public IPs: `203.0.113.10`, `203.0.113.11`
- LAN IPs and router: `192.168.1.100`, `192.168.1.101`, `192.168.1.1`
- Account handles: `me@`, `friend@`
- ROM short names, if considered sensitive

Preventive measures for the first commit: add `*.local` and `.env*` to `.gitignore`, and prefer placeholders such as `100.x.x.x` in documentation going forward.

## 9. Reference links

- Community direct-connect launcher: <https://github.com/nitsuboy/fightcade-fbneo-lan> (MIT)
- Upstream GGPO SDK (spectator API reference): <https://github.com/pond3r/ggpo>
- RetroArch netplay (user): <https://www.retroarch.com/netplay.php>
- RetroArch netplay protocol (developer): <https://docs.libretro.com/development/retroarch/netplay/>
- Native macOS client (evaluate later): <https://github.com/Jayian1890/Macade>
