# Design: cabinet — Tailnet FightCade lobby app (launcher + KotH + spectating)

Status: Phase 1 (macOS launcher + nethealth) implemented as a Tauri v2 app; KotH/scoring, room discovery, and spectating remain. See the progress note in §6.
Project name: **cabinet**. Git repository: **private** for now (see §8 Publication checklist).
Supersedes the "chosen route" framing in [`03-implementation-plan.md`](03-implementation-plan.md); the troubleshooting record (`01`–`03`) stays as history.

## TL;DR

- We already solved the hard network problem: `quark:direct` connects FBNeo peer-to-peer over Tailscale, bypassing FightCade matchmaking and the ISP CGNAT. Four friends have been playing this way all afternoon with a **direct, <100 ms** tailnet path.
- The proposed app is a **coordination layer**, not a FightCade clone. FightCade's server, `fcade` helper, and spectator relay are all closed; there is nothing to fork.
- The one genuinely hard feature is **input-relay spectating** (watch without video bandwidth). FightCade's spectator is server-brokered over a closed TCP protocol and is unreachable from `quark:direct`. **RetroArch netplay** provides the same low-bandwidth, input-based spectating natively, cross-platform, with no Wine or patched binaries.
- Decision: run a **one-day RetroArch spectator spike first**. If its netplay feels acceptable at <100 ms, make RetroArch the primary path and skip the risky `ggponet.dll` reverse-engineering entirely. Otherwise fall back to a time-boxed FightCade shim, then to "defer spectate".
- Features in scope: cross-platform Tauri lobby, **king of the hill**, room discovery over the tailnet, **per-player win/loss/draw tracking**, and **connection-health display (per-peer ping, direct-vs-relay)**.
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
- **Stack: Tauri v2 + Vite.** Rust backend for process/socket/tailnet work; the UI is a Vite-built web frontend rendered in the OS webview. Frontend stack is **React + TypeScript + Tailwind CSS v4 + shadcn/ui** (vanilla HTML/CSS/JS was the initial sketch; React + shadcn was chosen for a cleaner lobby UI). Node/npm is used only for the frontend build and the Tauri CLI, not the backend.
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
- **Match orchestration: authority-pull.** The host publishes `RoomState.currentMatch` naming both players (as stable node ids + assigned sides); each client launches its own emulator when it sees itself named, and stops/advances on host state. The host never pushes launch commands, so a missed message cannot strand a client mid-game. See the room protocol in §5.
- **Player identity: stable `node_id` + user-set `handle`.** The ladder is keyed by the machine's Tailscale **node id** (the key of the `Peer` map in `tailscale status --json`, and `Self.ID`), which is stable across IP/hostname changes; the display name is a user-set `handle` (falling back to the tailnet hostname). This replaces the local ledger's IP key when in a room.
- **Transport: UDP discovery + TCP control, newline-delimited JSON, versioned.** Discovery is a fixed UDP port (probe → room-info reply); control is a fixed TCP port (client connects, `hello`, then state/result messages). Both ports are configurable to dodge conflicts. See §5.
- **A drawn game is replayed**: no ladder change, same two players. This is deliberate — the overlay has no confirmed draw signal, so inventing a ledger outcome would be worse than replaying.
- **A room secret guards joins.** The TCP `hello` must present the room's join secret (shown to the host); this answers the tailnet's default `accept` posture without requiring ACLs. See §5.
- **Connection health is a first-class lobby feature.** Tailnet-level signals first (`tailscale status --json` + `tailscale ping`: RTT, direct-vs-relay, tx/rx) — no emulator cooperation needed. Fine-grained GGPO stats (ping, queue lengths, frames-behind) only later, via the shim path if ever. Complementary: recommend `bShowFPS 2` (ping+jitter on-screen overlay) for direct matches.
- **Gated proposals: emotes, opt-in voice, and input recording / match history.** Lobby/match reactions, mic/audio (default off, explicit per-session opt-in), and recording match inputs into a lobby match history for replays are **not** in scope until a **/grill-me session** stress-tests each. Voice especially must answer "why not Discord?"; recording/replay must answer "why not FightCade's existing `quark:replay`/`.fr` files?" and settle determinism/storage/consent before it enters the roadmap.
- **Development harness:** abstract "launch an emulator instance" so dev can use isolated/native instances instead of three Wine instances sharing one `WINEPREFIX`. **Implemented:** a `launch_dev_pair` command starts both sides (P1 local 7001, P2 local 7000) on `127.0.0.1`, plus a developer-loopback option for a single instance. Two Wine instances in the shared FightCade prefix were verified to coexist and bind their ports correctly, so no prefix isolation is needed for pairs.
- **Shim constraints (if used):** pin FightCade 2.1.45, disable auto-update, run Linux from a writable copy of `fbneo/`, and re-sign the macOS `.app` after replacing `ggponet.dll`.
- **Docs:** this file is the spec; `01`–`03` are amended for the direct-path finding and kept as history; `AGENTS.md` records conventions and setup.
- **Repository visibility:** private until the publication checklist in §8 is satisfied.

## 4. Architecture

```
cabinet
├─ frontend/           -> Vite + React + Tailwind v4 + shadcn/ui (lobby, room/KotH, ladder, scoreboard, spectator list)
└─ src-tauri/          -> Rust backend
   ├─ peer registry    -> `tailscale status --json` (stable 100.x / MagicDNS / node id)
   ├─ nethealth        -> per-peer RTT/path polling (`tailscale status --json` + `tailscale ping`); lobby badges + pre-match gate
   ├─ identity         -> stable node id + user handle (`player`)
   ├─ room discovery   -> fixed UDP port probe across tailnet peers (`discovery`)
   ├─ room control     -> newline-delimited JSON over a fixed TCP port (`control`)
   ├─ ROM index        -> scan the configured emulator's ROM dir
   ├─ session launcher -> port of scripts/fcade-lan-{macos,linux,windows}
   ├─ match supervisor -> watches process exit + fbneo/fightcade/*.txt
   ├─ room authority   -> host-authoritative queue + KotH ladder (`room`, P2P, no infra)
   └─ score ledger     -> wins/losses/draws per player (room-host persisted)
```

Gated components (no build order until grilled, see Decisions): `replay recorder` (capture per-match input streams + core/ROM identity; likely reuses the emulator's native replay format rather than a custom one) and `match history` (lobby index of past matches with playback, backed by the recorder).

Launcher abstraction covers: macOS Wine (`wine32on64` + `.wine32`), Linux Flatpak (`com.fightcade.Fightcade`) or native, Windows native, and a developer loopback mode. Phase 1 implements the macOS Wine adapter and the loopback mode; the **Linux adapter is implemented** (Flatpak-preferred, native/Wine fallback, `launcher/linux.rs`); the Windows adapter is pending. The Rust modules map onto the diagram as `tailscale` (peer registry + nethealth), `roms` (ROM index), `launcher` (session launcher abstraction), `session` (match supervisor), `results` (overlay/result watcher), `scores` (local per-opponent lifetime ledger), and `config`/`commands` (settings + IPC). Phase 2 adds `player` (identity), `discovery`, `control`, and `room` (KotH state machine + host-authoritative shared ledger). The current `scores` ledger is **local and per-opponent** (this machine's W/L vs each tailnet IP, counted per game from overlay score increments); the host-authoritative shared ladder in §5 is Phase 2.

Connection health (`nethealth`) polls `tailscale status --json` for per-peer `Online`, `CurAddr`/`Relay`, `Active`, `TxBytes`/`RxBytes`, and runs `tailscale ping --c N <peer>` for RTT plus the `via direct …` vs `via DERP(…)` path. The lobby shows one badge per peer (RTT in ms + path); launching warns when the path is relayed or RTT is above threshold. In-match the wrapper re-pings periodically — coarse by design, since per-frame GGPO stats live inside the closed emulator (visible on-screen via `bShowFPS 2`, ping+jitter). **Implemented (2026-09-19):** a session health thread pings the match peer every 4 s and the launcher card shows the live RTT/path badge. Fine-grained GGPO stats (queue lengths, frames-behind) arrive only with the shim path, if ever; on the RetroArch path the tailnet-level signals are the wrapper's display.

Gated extras (no build order until grilled): `reactions` (lobby/match emotes rendered by the wrapper overlay — never injected into the emulator), `voice` (opt-in mic/audio, default off, per-session consent with a visible live indicator), and `input recording / match history` (record each match's inputs — not video — keyed to core + ROM identity so the lobby can list and replay past matches; lean on the emulator's existing replay encoding where possible). None of these block Phases 1–3.

## 5. King of the Hill and score tracking

State machine held by the room host:

- Room state: `champion`, `challenger`, `queue[]`, `currentMatch`.
- On match end: detect winner (overlay file / exit / manual), mutate the ladder (winner stays), promote the next challenger, launch the next `quark:direct` pair.
- Roles per match: assign P1/P2 (side 0 / side 1).

Per-player ledger:

- Fields: `wins`, `losses`, `draws`, `games_played`, optional `current_streak` / `best_streak`.
- Storage: room-host local store (JSON or SQLite) so scores survive sessions and can be shown in the lobby.
- **Implemented (local, 2026-09-19):** `scores.rs` persists a per-opponent ledger to `<app_config_dir>/scores.json`. Because direct mode bypasses `fcade`, there are no real handles — the opponent key is the launch **peer IP**, and the local player's identity is the launch **side** (`plans[].config.side`). A game is counted from an overlay score increment: the session diffs `p1score`/`p2score`, and a single-game delta (`+1` on exactly one side) is attributed as win/loss to the local side. Counter resets and over-large deltas are ignored (no double counting). Dev-pair (loopback) sessions are excluded. The shared host-authoritative ladder across all four players is still Phase 2. **Verified live (2026-09-19, real direct match):** a decisive game incremented the ledger — `scores.json` recorded `1W/1L` for the peer over two games, and the lobby's lifetime card updated in place without a restart.
- Result sources, in order of preference:
  1. `fbneo/fightcade/winner.txt` (+ `p1score.txt`/`p2score.txt`) when `bVidSaveOverlayFiles 1` — **verified in direct mode (2026-09-19)**. Details in §7.
  2. Emulator exit + per-player confirm.
  3. Manual dispute resolution by the room host.
- Draw handling: a drawn game (double KO / timeout with equal rounds) **does not mutate the ladder — the same two players replay it**. The overlay has no confirmed draw signal, so rather than fabricate a ledger outcome the host simply re-runs the match. A manual "Draw / replay" button lets a player request it when the overlay is ambiguous.

### Room protocol (Phase 2)

Agreed wire format, implemented incrementally in Phase 2 (2.1 state machine → 2.2 discovery → 2.3 control → 2.4 orchestration → 2.5 shared ledger → 2.6 mid-match re-ping).

**Identity.** A player is keyed by the Tailscale **node id** (the `Peer` map key in `tailscale status --json`, and `Self.ID`) — stable across IP/`DNSName`/hostname changes. The display name is a user-set **handle** (defaults to the tailnet hostname). The room state carries `node_id`, `handle`, and `ip` for display/launch purposes.

**Discovery (UDP, default `47810`, configurable).** Each app probes every online tailnet peer's discovery port with a small JSON datagram and waits briefly for replies:

```
-> {"magic":"cabinet/1","kind":"probe"}
<- {"magic":"cabinet/1","kind":"room","roomId":"...","host":"<handle>","rom":"sfiii3nr1",
    "phase":"lobby|playing","champion":"<handle>","queue":3,"players":2,"secretRequired":true}
```

A host listens on the port; a non-host replies nothing. The lobby renders one row per room; an empty list means nobody is hosting. Discovery reveals no secrets.

**Control (TCP, default `47811`, configurable).** Newline-delimited JSON, one message per line. The host listens; clients connect and drive:

```
client -> {"v":1,"kind":"hello","roomId":"...","nodeId":"...","handle":"...","secret":"...","want":"play|spectate"}
host   -> {"v":1,"kind":"welcome","playerId":"...","state":{...RoomState}}
client -> {"v":1,"kind":"enqueue"}          // join the KotH queue
client -> {"v":1,"kind":"leave"}
client -> {"v":1,"kind":"result","matchId":"...","outcome":"win|loss","confidence":"overlay|manual"}
host   -> {"v":1,"kind":"state","state":{...RoomState}}     // on every change
host   -> {"v":1,"kind":"ping"} / client -> {"v":1,"kind":"pong"}
```

The client may open this connection only if it presents the room's `secret`; a wrong/missing secret is rejected with an `error` message. Because tailnet peers can otherwise reach the port, the secret is the join gate (documented; not a substitute for ACLs).

**RoomState (host-authoritative, versioned).**

```
{ "roomId", "host": {nodeId,handle,ip}, "rom", "revision":N,
  "phase": "lobby|playing",
  "champion": {nodeId,handle} | null,
  "challenger": {nodeId,handle} | null,
  "queue": [{nodeId,handle}],
  "currentMatch": { "matchId", "p1":{nodeId,handle,ip,side}, "p2":{...}, "startedAtMs" } | null,
  "ledger": { "<nodeId>": {handle,wins,losses,draws,games} } }
```

**Orchestration (authority-pull).** On every state message the client compares `currentMatch` against its own `nodeId`:
- named in `currentMatch` → launch `quark:direct` locally with the assigned side and the opponent's `ip`;
- was in a match, now not → stop its instance;
- not named → stay idle.

The host advances the ladder only on a result, so a client that misses a state message re-syncs from the next `state` and still launches/stops correctly.

**Result flow.** The client watches its **local** overlay (existing `results.rs` watcher) and knows its own side. On a decisive score increment it sends `result` with `matchId` (to reject stale reports) and `confidence=overlay`; the host records W/L and advances. If a match ends with no overlay signal, the client offers the existing manual "I won" button (`confidence=manual`); the host resolves disputes. On a `draw`/replay the host re-runs the same pairing. **Implemented (2026-09-19):** the room poller runs a `ScoreCounter` over the overlay dir for the local side and auto-sends `confidence=overlay` results; the room card's "I won"/"I lost" buttons remain as the manual fallback.

**Shared ledger.** The host persists `RoomState.ledger` (per `node_id`) to `<app_config_dir>/room-ledger.json` so scores survive sessions; clients render the host's ledger as read-only (and the host re-loads it when hosting). The existing local per-opponent ledger remains the fallback when not in a room. When a room session is active, the local `quark` launches are host-driven and **the local ledger is bypassed** (room launches pass `track_scores=false`) to avoid double counting. **Implemented (2026-09-19).**

## 6. Phased plan

| Phase | Work | Gate |
|---|---|---|
| 0 | RetroArch spectator spike: host + 1 client + 2 spectators over Tailscale; measure feel and bandwidth | Feel acceptable at <100 ms? |
| 0b | If not: FightCade `ggponet.dll` shim spike (static RE of `quark:stream` arg semantics, then a minimal shim) | Hard time-box; if the `quark:stream` middle token is a server session id, stop |
| 1 | Tauri launcher: peer registry, ROM index, side/role, spawn/teardown, Wine/Flatpak/native; **lobby connection health** (per-peer ping + direct/relay badge, pre-match warn on relay/high RTT) | Replaces scripts with equivalent behavior; unhealthy peer flagged before launch |
| 2 | Room + KotH + score ledger; auto-launch next pair; result detection; periodic re-ping of match peers during the session. Sub-steps: **2.1** `player`/`room` state machine (pure, tested) → **2.2** UDP discovery → **2.3** TCP control → **2.4** authority-pull orchestration → **2.5** host-persisted shared ledger → **2.6** mid-match re-ping → **2.7** 4-person test (2.1–2.6 done) | 4-person session end-to-end |
| 3 | Spectate integration via the Phase 0 outcome; else documented as deferred; show spectator link quality with the same nethealth signals | 2 concurrent spectators |
| 4 | Docs + packaging, including a convenient cross-OS delivery/update path (see §7) | Reproducible on all four machines |
| Gated | Emotes + opt-in voice + input recording / match history for replays | Admitted to the roadmap only after a /grill-me session |

**Progress note.** Phase 1 is implemented (macOS-first). The app currently provides: peer registry and per-peer connection health (`tailscale status --json` + `tailscale ping`, direct/DERP/RTT badges, relay/high-RTT warnings), ROM index, config/settings overrides, the macOS Wine launcher with spawn/stop/exit detection, a developer loopback pair, a **match-result watcher** that polls the emulator's `fbneo/fightcade/` overlay files (winner/scores/characters) during and after a session, and a **local per-opponent lifetime score ledger** with streak tracking and a lobby card. Verification: Rust unit tests for parsers/ports/config, the overlay reader, and the ledger/counter, plus opt-in live smoke tests (status, ping, ROM scan, real emulator launch and loopback pair); the shell launchers remain the reference. The lifetime-score ledger was **verified live** on 2026-09-19 (a real direct match wrote two games to `scores.json` and updated the lobby card). Phase 2's wire protocol is designed in §5 (2.0), and its backend core is implemented and unit-tested: **2.1** identity + KotH state machine (`player`, `room`), **2.2** UDP discovery (`discovery`, `list_rooms`), **2.3** TCP control channel (`control`, join secret, state broadcast), **2.4** room service + orchestration (`service`): host/join/leave, enqueue, manual result reporting, a discovery-backed Rooms UI and a room ladder/scoreboard card, and authority-pull auto-launch/stop of the local emulator when `currentMatch` names this machine. Remaining: **2.7** 4-person test. **2.5** is implemented (2026-09-19): the host persists the shared ledger to `<app_config_dir>/room-ledger.json`, and the room poller auto-reports results from the local overlay (`confidence=overlay`) with the manual buttons as fallback; room launches bypass the local ledger to avoid double counting. **2.6** is implemented (2026-09-19): a session health thread re-pings the match peer every 4 s and attaches the RTT/path (`peer_health`) to `MatchState`, shown as a badge on the launcher card. Phase 0/0b, 3, and 4 are not yet done; the **Linux launcher adapter is implemented** (`launcher/linux.rs`: Flatpak-preferred, native/Wine fallback), and the Windows adapter is stubbed by platform gating (non-Linux/macOS builds return an error until implemented).

### Prerequisites (do once per machine)

Rust (via the official installer, not Homebrew):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

The Tauri CLI is **not** installed globally via `cargo install`; it ships as a prebuilt npm package and is a devDependency in the root `package.json` (`@tauri-apps/cli`). The frontend lives in `frontend/` with its own `package.json`; `src-tauri/` is the backend.

```sh
npm install                      # root: Tauri CLI
npm install --prefix frontend    # frontend: React/Vite/Tailwind/shadcn
npm run tauri dev                # dev: Vite on :1420 + Rust backend
npm run tauri build              # production bundle
```

Per-OS system dependencies (not Rust crates; Cargo cannot fetch these):

| OS | Required |
|---|---|
| macOS (this machine) | Xcode Command Line Tools — already installed at `/Library/Developer/CommandLineTools` |
| Windows (native friend) | MSVC C++ Build Tools + WebView2 runtime (ships with Win10/11); NSIS/WiX only for installers |
| Linux (CachyOS friend) | `libwebkit2gtk-4.1-dev`, `build-essential`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` |

Cross-compiling Tauri across OSes is painful; each of the four machines builds its own binary. Test an early build on the CachyOS machine first — WebKitGTK is the least predictable webview.

## 7. Open questions

- ~~Does `bVidSaveOverlayFiles 1` actually write `winner.txt`/scores in **direct** mode, or does it need FightCade match metadata?~~ **Answered (verified 2026-09-19): it writes them, no FightCade metadata needed.**
  - With `bVidSaveOverlayFiles 1` set in `fbneo/config/fcadefbneo.ini`, a `quark:direct` session (local loopback pair) wrote the full overlay set to `fbneo/fightcade/`: `game.txt` (ROM short name), `gametype.txt`, `vs.txt`, `started.inf` (`1` while running), `p1name.txt`/`p2name.txt`, `p1rank.txt`/`p2rank.txt`, `p1character.txt`/`p2character.txt`, `p1score.txt`/`p2score.txt`, and `winner.txt`.
  - `p1name`/`p2name` are placeholders (`Player1`/`Player2`) and rank is `?`, because `fcade` (which injects real handles) is bypassed in direct mode — so results must be mapped by **name**, not a side id.
  - `winner.txt` holds the winner's **name** (no trailing newline); score files are ASCII integers. A decisive local session produced `winner.txt`=`Player2`, `p2score.txt`=`1`, `p1score.txt`=`0`.
  - **Live update confirmed (2026-09-19, real 2-player direct match, Ken vs Akuma):** the overlay updates **while the match runs** — `winner.txt` and the score files changed repeatedly mid-session (e.g. `Player2`/`0–1` → `Player2`/`0–2` → `Player1`/`1–2`) with no emulator exit. Scores count **games within the session**, not just a final result; `winner.txt` is the winner of the most recent game.
  - Still open: the exact **draw** signal (a level score with no `winner.txt` was only observed synthetically), and whether a best-of-N series reset clears the counters. The app polls the overlay dir every 300 ms and re-reads after exit, so both live and on-exit writes are covered.
- `quark:stream` argument semantics: is the middle token a host IP or a server session id?
- Is RetroArch FBNeo netplay feel acceptable for fighting games at <100 ms?
- Multi-spectator fan-out ceiling and whether each spectator needs its own inbound UDP port / firewall rule.
- What exactly constitutes a "draw" signal from the emulator, if any.
- Room protocol: does the macOS/Windows firewall prompt on first UDP/TCP bind, and can the bind be limited to the tailnet interface (`utun`) rather than all interfaces? Can more than one room coexist on the tailnet (multiple hosts answering discovery)? Does the join secret need a per-room rotation after each session?
- Connection-health UX: poll cadence for `tailscale status --json` / `tailscale ping` (lobby idle vs. in-match), RTT warn threshold (group plays direct at <100 ms — warn above ~150 ms?), and Windows/macOS CLI output parity for parsing.
- Can the wrapper detect mid-match degradation, or only pre-match? (GGPO stats need the shim; tailnet re-ping mid-match is coarse.)
- Emotes/voice gate: what survives /grill-me? Open: emote surface (lobby-only vs. in-match overlay), voice transport (WebRTC over tailnet vs. "just use Discord"), push-to-talk vs. open mic, per-session consent UX.
- Input recording / match history gate: what survives /grill-me? Open: reuse emulator replay files (FightCade `.fr` / `quark:replay`, RetroArch replay) vs. capture raw inputs; where recordings live and who holds them (room host vs. per-player); determinism/version pinning (core + ROM CRC) so replays stay playable; storage growth and retention; recording consent and visibility (does the lobby show a "recording" indicator?).
- macOS: signature/quarantine handling after replacing `ggponet.dll`; behavior when FightCade auto-updates.
- **Getting builds onto the friend machines is too manual.** Today each machine clones/copies the repo, installs OS deps, and runs a native build (`scripts/setup-linux.sh`), and there is no way to push updates iteratively — a code change means re-copying and rebuilding by hand, and stale package DBs / missing toolchains derailed the first Linux attempt. We need a more convenient way to get the code running and keep it updated on all four systems (options to evaluate: a prebuilt artifact per OS in a private release, `tauri-plugin-updater` with a signed update feed, a git pull + one-command rebuild script, or shipping via a package manager). This blocks regular cross-machine testing and should be resolved before Phase 2.7/4. Deferred for now.
- RetroArch path: ROM/core parity and content-CRC matching across the four machines, and whether FBNeo core serialization is enabled.

## 8. Risks and tradeoffs

- A `ggponet.dll` shim is tied to FightCade 2.1.45 and will break on update; must pin and disable auto-update.
- Modifying a signed macOS `.app` requires re-signing; the Linux Flatpak `/app` is read-only (needs a writable copy).
- RetroArch's TCP/replay netplay may feel worse than FightCade's UDP/GGPO for fighting games.
- The 8 GB M1 dev harness (Chrome + editor + multiple emulator instances) may swap; use native/isolated instances.
- Room-host authority means the ladder pauses if the host leaves; acceptable per "anybody can host".
- Tailscale ACLs + per-OS firewall rules are prerequisites for discovery/spectating.
- `tailscale status --json` format is version-dependent (Tailscale warns it may change between releases); pin a minimum Tailscale version and parse defensively.
- Voice is a subsystem, not a feature (device selection, echo, consent UX); mic defaults off with a visible live indicator is mandatory. Scope risk stays until the /grill-me gate is passed.
- Input recordings are only replayable against the exact core + ROM build; a core/ROM update silently invalidates history. Storage and retention are unbounded unless capped, and recordings capture player behavior (consent/visibility needed). Scope risk stays until the /grill-me gate is passed.
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
