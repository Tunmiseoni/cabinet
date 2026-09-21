# Design: The Cabinet — Tailnet lobby (rooms, sets, spectating rotation)

Status: **design agreed in a `/grill-me` session (2026-09-21); the lobby core has started.** This document
supersedes the removed rooms/KotH subsystem ([`04-design.md`](04-design.md) §5) for the *replacement*
lobby. It is a design, not a spec: [`04-design.md`](04-design.md) remains the top-level spec and will
be updated as the pieces land. The **build order is gated on a hardware spike**, specified separately
in [`10-lobby-spike.md`](10-lobby-spike.md) — read that before writing code. **Implemented
2026-09-21:** the RetroArch command-socket plumbing (`providers/retroarch/command.rs`, per-instance
command ports, `network_cmd_enable`), per-ROM RAM result detection (`lobby/results.rs`, the
`sfiii3nr1` address table and round-outcome watcher), the room model + HTTP beacon (`lobby/room.rs`,
`lobby/beacon.rs`), hosting, the seat/role split, the client join flow (`lobby_*` commands), and
`LobbyCard.tsx`. **Not implemented:** sets/rotation, history/scores, `RoomView.tsx`.

**The lobby is the preferred home-screen path.** `LobbyCard.tsx` hosts/joins rooms and is rendered
first; the direct **Launch match** card is hidden unless the `developerMode` setting is on (and
developer mode is session-scoped — forced off at every app start), and the
`launch_match`/`launch_dev_pair` commands are kept only for that card and loopback diagnostics —
`lobby_start`/`lobby_join` call the shared `launch_match_inner` instead. While a session runs,
`LobbyCard` shows a **Stop match** control so joiners (who have no Launch card) can stop; it calls
`lobbyStop` with a `stopMatch` fallback. The managed FBNeo core is stored under its canonical name
`fbneo_libretro.<ext>` under `<app_data_dir>/cores/<platform>/`; an earlier release-asset-named copy
is still found and migrated on startup (see [`04-design.md`](04-design.md) §3).

Motivation: the current flow makes the user pick a **role** (P1 / P2 / Spectator) against a peer by
hand, and there is no way to *find* a session. The lobby reframes this around a **room**: create one,
let everyone on the tailnet see it, and join — playing or spectating as capacity allows, with an
automatic winner-stays rotation driven by game state.

> Scope reminder: this is a **4-person, invite-only tailnet**. Every decision below is calibrated to
> that, not to a public matchmaking service.

---

## 1. Why this is not the thing we deleted

On **2026-09-21** the host-authoritative room stack was removed outright (`d65d994`): `player.rs`,
`room.rs`, `discovery.rs`, `control.rs`, `service.rs`, `scores.rs`, `results.rs`, plus the
`RoomCard`/`RoomsCard`/`LifetimeCard` frontend cards. [`04-design.md`](04-design.md) §5 records the
reasons and keeps the old protocol as history. The grill's accepted **root cause**:

1. **Result detection** came from FightCade-side overlay files (`winner.txt`, `p1score.txt`,
   `p2score.txt`), a provider-specific, fragile signal — and FightCade is the path being retired.
2. **The shared ledger was host-authoritative** — one machine held the truth, nobody trusted it, and
   it could not be independently verified.
3. **Room liveness ran over a bespoke TCP control protocol** (`47811`) with ping/pong heartbeats;
   when that channel dropped, the room appeared to die. (Network flakiness suspected; unproven.)

The replacement removes all three by construction:

| v1 failure | v2 answer |
|---|---|
| Overlay result detection (FightCade-only) | **Per-ROM RAM watcher** read from each machine's *own* emulator over RetroArch's command socket; the game is deterministic, so both players observe the same result. |
| Untrusted shared ledger | **Decentralized**: every machine keeps its own ledger from its own observation (no central store, no host truth). |
| Bespoke TCP control channel + heartbeat | **No cross-network control protocol at all.** The only cross-network traffic is RetroArch netplay itself; control is a local UDP command socket on each machine. A "room dropping" as a class of bug cannot recur the same way. Discovery is a single, read-only, independently testable beacon with a manual-address fallback. |

If the spike ([`10-lobby-spike.md`](10-lobby-spike.md)) shows the root cause was actually *discovery*,
the beacon gets re-designed before the lobby core is built — that is why the spike comes first.

## 2. Scope

**In scope (v1):** room creation/teardown, tailnet room discovery (+ manual join), join-with-parity
gate, live play/spectate, first-to-N sets with automatic winner-stays rotation, automatic per-round
result detection, local lifetime scores, local match history.

**Deferred / out of scope:** replays (native `.replay` — see §7.11; gated on its own spike),
host migration (parked, §7.8), voice/emotes, FightCade-path lobby support, an idle "waiting room"
(§7.2), any central/shared state.

## 3. Key technical findings

These are established from RetroArch's source (version 1.22.2 line as frozen in
[`07-retroarch-spike.md`](07-retroarch-spike.md) §2) and are the basis for the design.

**Live role switching is real — no relaunch.**
`netplay_toggle_play_spectate()` (`network/netplay/netplay_frontend.c:8164`) switches play↔spectate
on a running instance:

- **player → spectator** (client): immediate. It clears the client's player mask and devices,
  sets `self_mode = SPECTATING`, and announces (`:8169`–`:8189`).
- **spectator → player** (client): sends `netplay_cmd_mode(PLAYING)` and waits for the host
  (`:8196`). The host's `netplay_handle_play_spectate` (`:4968`) **auto-grants if a player device
  slot is free** (`NETPLAY_CMD_PLAY`, `:5040`+); it refuses only with
  `NETPLAY_CMD_MODE_REFUSED_REASON_NOT_AVAILABLE`.
- **host → spectator**: supported in protocol — `netplay_handle_play_spectate`'s server branch sets
  `self_mode = SPECTATING` and announces to all (`:5029`+); the server can be a non-playing relay.

The trigger is `RARCH_NETPLAY_CTL_GAME_WATCH` (`:10170`).

**The control plane already exists.** RetroArch's **network command interface** (UDP, default
`55355`; `network_cmd_enable` default `false`, `network_cmd_port` default `55355`) accepts, among
others (`command.h:498`–`563`):

| Command | Use |
|---|---|
| `NETPLAY_GAME_WATCH` | Toggle play/spectate on the target instance. |
| `READ_CORE_RAM <addr> <len>` | Read system RAM — the automatic result signal (§7.6). **Note:** the older memory-map command `READ_CORE_MEMORY` does **not** work with FBNeo (`no memory map defined`); use `READ_CORE_RAM` (512 KB, address base `0x02000000`). |
| `GET_STATUS` | Instance/core/content status. |
| `RECORD_REPLAY` / `HALT_REPLAY` / `PLAY_REPLAY_SLOT` / `SEEK_REPLAY` | Native replay (deferred, §7.11). |

Enabling it is a per-role appendconfig change; the socket is local-only and unauthenticated.

**There is no generic match/winner signal.** RetroArch and FBNeo expose nothing that says "the set
ended" or "who won." The only *existing* automatic source was FightCade's overlay files, which is
the path being retired. Automatic detection therefore means **per-ROM work** (§7.6).

**Host-as-spectator is untested.** [`07-retroarch-spike.md`](07-retroarch-spike.md) §9 / **F18**
recorded it as *not exercised* and, on 2026-09-21, a **non-goal** ("the host also plays (P1)"). The
rotation model *requires* it, so it is now **in scope** and is the spike's first item — which means
F18 is explicitly **reversed** by this design.

**Replay recording during netplay is unverified.** The input "movie" subsystem (`input/bsv/bsvmovie.c`)
has no obvious netplay guard, but whether `.replay` recording works mid-netplay and captures *both*
players is unknown; it is deferred behind a spike (§7.11).

## 4. The room

**A room *is* a running RetroArch host instance.** RetroArch netplay only accepts clients while a
host has the core + ROM loaded and is listening, so there is no such thing as a metadata-only room
without inventing a signaling service (which is the v1 shape we are deliberately not rebuilding).

- Creating a room **launches** the host instance. The host picks its own seat when creating the
  room: **`Player 1` (default) / `Player 2` / `Spectate (table)`** (`hostSeat` on `lobby_start`). A
  seated host plays; the **Spectate** choice is the non-playing spectator server (the "table") for
  rotation, so the host machine is not forced onto a controller when it is only arbitrating.
  (Corrected 2026-09-21: the built version originally *always* hosted as a non-playing spectator,
  which left a two-person session with a single player — the host had no way to take a seat because
  the live toggle below is not built yet.)
- "Waiting in the lobby" = the host emulator sits at attract / character-select until a client joins.
- The host chooses the **ROM** and the **first-to-N** when creating the room. Both are fixed for the
  room's lifetime.
- The room dies when the host emulator dies (§7.8). There is no persistence and no idle state.
- The host **may toggle between play and spectator later** (it is just another instance using the
  same control path) — this is what makes winner-stays rotation possible when the host is one of the
  two players. The switch is the B4 `NETPLAY_GAME_WATCH` control; until it lands, the seat is chosen
  at creation and cannot change.

Consequences accepted: a joiner must already own a **matching ROM + core**, so the join flow runs the
existing parity gate *before* spawning the joiner's emulator (§5), not after.

## 5. Discovery and joining

**Discovery = one read-only beacon, plus manual entry.** While hosting, the app serves a beacon that
any tailnet peer can query. It is *read-only advertisement*; it carries no control authority and no
secrets.

- **Beacon contents (proposed):** `roomId`, `hostNodeId`, `hostHandle`, `rom`, `firstTo`, `phase`
  (`waiting` | `playing`), `players` (slots in use), `spectators` (count), `revision`.
- **Query path:** the app already enumerates online tailnet peers (`tailscale status --json`); it
  probes the beacon port on each online peer only. Non-hosts answer nothing.
- **Transport: HTTP `GET /room` (decided 2026-09-21).** While hosting, the app serves JSON on the
  configured beacon port (default `47812`, `lobbyBeaconPort`) via a small server (`tiny_http`, the
  app's first server dependency); peers probe with the existing `ureq` client. It is easy to test
  with `curl` and to reason about; the cost is a listening socket and per-OS firewall surface (spike
  S7). **Implemented:** `lobby/beacon.rs` (serve + query), `commands/lobby.rs`
  (`lobby_start`/`lobby_stop`/`lobby_status`/`lobby_query`/`lobby_discover`).
- **Manual fallback is mandatory in v1:** a host can paste/hand out `host-ip:netplay-port`, and a
  joiner can enter it directly. A broken or blocked beacon must never prevent play.

**Join flow:** query beacon → show room (host handle, ROM, phase, occupancy) → joiner confirms →
**parity gate** (frontend version, core revision, content CRC — the gate already exists in
`providers/retroarch/parity.rs`) using the beacon's ROM identity → spawn the joiner's instance as a
client/spectator (`-C <host>`). If parity fails, the joiner is told before anything launches.

**Seat vs. role (resolved 2026-09-21).** The room host assigns each connecting client a player slot,
but `Role` coupled connection direction with the player slot: `P1` was the `-H` server binding
`input_player1_*`, `P2` a `-C` client binding `input_player2_*`. A client seated as player 1 had no
role that both connected as a client and bound player 1 (the normal case with a host-as-spectator,
where the first joiner becomes player 1). Fixed by separating *seat* from *role*: the launch request
carries an explicit `player_slot` (`1`/`2`, or none), the seat chooses the `input_player{1|2}_*`
binds, and `Role` still chooses the connect direction (`-H`/`-C`/spectator). The `lobby_join` command
derives the seat from the room's advertised occupancy (first player -> 1, second -> 2, full ->
spectate) unless the joiner picks one explicitly.

**v1's second protocol is dropped.** There is no custom TCP control channel. Discovery is one
beacon; control is RetroArch's own command socket (§7.5).

## 6. Identity

- **Key:** the Tailscale **node id** (the `Peer` map key in `tailscale status --json` / `Self.ID`) —
  stable across IP, DNS-name, and hostname changes.
- **Display:** a **handle**, shown to prospective joiners ("people that want to join should see the
  handle"). Defaults through the existing chain (`retroarchNickname → handle → tailnet self
  hostname → OS hostname → "player"`).
- The FIFO queue is a list of node ids; the UI renders handles.
- **Privacy constraint:** this repository is public. Nothing observed at runtime (real handles,
  tailnet names/IPs) may be persisted into the repo. All docs/code use the sanctioned placeholders
  (`100.64.0.1`–`.3`, `mac-host`, `cachyos-host`, `windows-host`, `player-one`, …).

## 7. Design by area

### 7.1 Roles

The lobby does not remove roles; it hides them. Instances still launch as host / client / spectator,
and the role is the launch argument — but the UI problem changes from "choose your role" to "create
or join a room, and the room assigns you a seat." Three seat types remain:

| Seat | Instance | Notes |
|---|---|---|
| Server | The room's host instance | Plays a seat by default; `Spectate (table)` keeps it non-playing. May toggle into/out of play (later, §7.4). |
| Player | A client claiming a device slot | Two player slots (P1/P2-equivalent). Host and joiners each hold one. |
| Spectator | A client with `netplay_start_as_spectator = "true"` | Sends no input; many allowed. |

### 7.2 Room lifecycle

1. Host creates a room: picks ROM + first-to-N + its own seat (default Player 1); the app launches
   the host instance seated (or as a non-playing server for `Spectate`) and starts the beacon. The
   beacon's `players` counts the host's seat, so the first joiner is seated in the free slot.
2. Joiners connect (client or spectator). First two *players* are seated; everyone else spectates.
3. The room is `waiting` until two players are seated, then `playing`.
4. On set end (automatic detection, §7.6): winner stays **and keeps their slot/side**; the loser
   toggles to spectator; the **longest-waiting spectator** toggles into the vacated player slot.
   Both toggles are driven by the app over the command socket.
5. With **no waiting spectators**, the two players keep playing indefinitely — no rotation. The set
   counter still runs, but nothing changes hands until a spectator wants in.
6. Host app closes / host emulator exits → room ends; all clients disconnect and return to idle.

**No idle waiting room.** A room exists only while its host instance runs.

### 7.3 Sets

- A **set is first-to-N wins**, N configured at room creation (**default N = 2**). The open question
  of "matches per set" is resolved to *first-to-N* rather than *best-of* or *fixed N games*.
- **A "win" is a whole game/match, not a single round** (settled 2026-09-21). The RAM watcher emits
  per-round outcomes; round wins accumulate to the game, and the game winner is the first side to
  **2 round wins** (SF3's own best-of-3), after which the game's round counter resets for the next
  game. The set counts *game* wins, and rotation happens at a game boundary — never mid-game. The
  earlier §7.6 wording ("counting these outcomes") meant round outcomes feeding the game score, not
  rounds counting directly toward the set.
- The **winner keeps their slot/side**; only the other slot changes hands. Sides are the two player
  device slots; the winner is not re-seated.
- A **draw** (double KO / timeout with equal health) is recorded but does **not** count as a round win
  for either side, so it does **not** advance the game or the set and does **not** rotate anyone; the
  same two players replay it.
- The set ends when one side reaches N game wins; that transition triggers the rotation in §7.2
  step 4.

### 7.4 Live role switching (the rotation mechanism)

Driven from the app via `NETPLAY_GAME_WATCH` on the relevant instance's command socket. Ordering on
set end, to avoid a slot race:

1. Loser toggles **player → spectator** (immediate, releases the device slot). Wait for the slot to
   be free/announced.
2. Longest-waiting spectator toggles **spectator → player**; the host auto-grants the now-free slot.
3. Winner does nothing.

A would-be challenger must **already be connected as a spectator** — no fresh join during the
transition (avoid the flaky mid-join window). This is the FIFO contract.

### 7.5 Control plane

- Enable RetroArch's network command interface in the per-role appendconfig (`network_cmd_enable`).
- **Bind to loopback and assign a distinct `network_cmd_port` per instance.** The default `55355`
  collides if two instances run on one machine (the developer loopback pair), so the app must
  allocate a port per running instance and remember it.
- All app→instance commands (`NETPLAY_GAME_WATCH`, `READ_CORE_MEMORY`, later `RECORD_REPLAY`/…) go
  through this socket. No OS-level key injection (which breaks on Wayland, and needs extra grants).
- Accepted risk: the socket is unauthenticated. It is loopback-only and exists only while an
  instance runs. Revisit if a machine is ever multi-user.

### 7.6 Automatic result detection (per-ROM RAM watcher)

- **Signal:** poll RAM with `READ_CORE_RAM <addr> <len>` and read the per-round state. This is **the**
  automatic signal; there is no generic one.
- **Located on `sfiii3nr1` (spike 2026-09-21, core `GIT6bb3167`):** there is **no per-fighter win
  counter** in the exposed 512 KB. The decisive result is read from health, with a round counter as
  the transition trigger:

  | Signal | `READ_CORE_RAM` offset | Behavior |
  |---|---|---|
  | P1 current health | `0x068D08` (mirror `0x068D0E`) | `0xA0` at round start; falls on damage; **saturates to `0xFF` on KO** |
  | P2 current health | `0x0691A0` (mirror `0x0691A6`) | same |
  | Rounds completed this match | `0x010D28` | `0,1,2…`, increments once per round **regardless of who won**; resets at match start |

  A round is decisive when one health byte reaches `0xFF` (that side was KO'd and lost); on a timer
  expiry with neither at `0xFF`, the higher health wins; both at `0xFF` is a draw. Round outcomes
  accumulate into the game score (first to 2 round wins; §7.3), and the game's round counter reset
  marks the next game — the lobby does **not** read a counter as a win count. (An earlier read
  mistook `0x010D28` for a win counter; it increments once per round for either winner and resets
  each game.)
- **Cost accepted:** it is **per-ROM reverse engineering**, version-sensitive to core/ROM updates.
  Start with **`sfiii3nr1`** (the ROM the group plays now).
- **Both players observe independently.** Because the game is deterministic and synced, both
  machines compute the *same* result (verified: host/client/spectator read identical values). If they
  disagree mid-session, that is a **desync alarm** — surface it, do not silently pick a winner.
- **Recording:** store the addresses **and** the core revision they were validated against; re-run the
  spike on any core/ROM change.
- Detection feeds: the set counter, the rotation trigger (§7.4), lifetime scores (§7.9), and
  history (§7.10).
- **Read cost:** the command socket services ~1 command/frame (~60/s), so read only these windows; a
  full 512 KB sweep takes ~34 s.

### 7.7 Discovery beacon

See §5. Read-only; no authority; manual fallback required. Firewall behavior on each OS is a spike
item (S7) because v1's discovery "not working well" is not root-caused and may have been firewall or
socket related.

### 7.8 Host failure and lifecycle

- The host app owns the host emulator process, so **host exits → room ends**; every client and
  spectator disconnects and returns to idle. They may join a new room.
- **No host migration.** RetroArch has no migration primitive; building one would resurrect the
  host-authoritative state machine this design exists to avoid. *Parked, explicitly out of scope.*
- A host that wants to stop playing toggles to spectator (§7.4) but must keep the app (and thus the
  server) running.
- Open product question (§9): a designated always-on "table" machine for the group so sessions
  don't depend on one person's laptop staying awake.

### 7.9 Lifetime scores

- **Local, per-machine ledger**, keyed by opponent **node id**, derived from *that machine's* own RAM
  watcher. There is **no shared/host-authoritative ledger** — this is the direct fix for the v1
  trust failure.
- Fields (proposed): `wins`, `losses`, `draws`, `games`; draws are recorded but do not advance a set.
  Streaks/head-to-head views are optional and not required for v1.
- Storage: `app_config_dir` (JSON). Never committed; never in the repo.
- Because every machine computes independently, two machines' ledgers can be cross-checked; a
  mismatch is a bug or a desync, not an authority dispute.

### 7.10 Match history

- **Local, read-only index** of completed **sets**, one per machine.
- Recorded: timestamp (UTC), ROM, both node ids + handles (display), final set score, winner.
- **Per-round records** are captured as well, indexed at the **set** level (a set entry owns its
  rounds).
- Storage: `app_config_dir` (JSON); never committed. History is a log; it is distinct from scores
  (aggregate) and replays (playback).

### 7.11 Replays (deferred)

- RetroArch **has** its own replay format (the input "movie", `.replay`), which answers "why not the
  emulator's own format?" — one already exists.
- Intended mechanism: start/stop recording over the command socket (`RECORD_REPLAY`/`HALT_REPLAY`);
  playback via `PLAY_REPLAY_SLOT`/`SEEK_REPLAY`. Replays are tiny (input-only, ~1.2 KB/s).
- **Unverified:** whether recording works *during netplay* and captures **both** players. Gated
  behind a spike (S8). **Deferred** until that spike passes — not in the lobby's first version.
- If it works: store per-session locally with the **core revision + ROM sha** needed to play back;
  show a visible recording indicator with an explicit opt-in (inputs capture player behavior).
- Accepted risk: a core/ROM update silently invalidates old replays; the stored identity lets
  playback refuse clearly rather than desync.

## 8. Module map (partially built)

Indicative only; names may change. It slots into the existing `src-tauri` layout without reviving
any deleted module. **Built 2026-09-21:** the command socket lives in
`providers/retroarch/command.rs` (with per-instance port allocation in the provider), and the
per-ROM RAM watcher is `lobby/results.rs`. The items marked *proposed* are not built.

```
src-tauri/src/
├─ lobby/
│  ├─ room.rs        -> room model (rom, firstTo, seats, phase, node ids)   [built]
│  ├─ beacon.rs      -> serve + query the read-only discovery beacon         [built]
│  ├─ service.rs     -> the hosting room + beacon lifecycle                  [built]
│  ├─ control.rs     -> the RetroArch command socket (NETPLAY_GAME_WATCH, READ_CORE_RAM, …) [built in providers/retroarch/command.rs]
│  ├─ sets.rs        -> set/rotation state machine (first-to-N, FIFO)         [proposed]
│  └─ results.rs     -> per-ROM RAM watcher (address table + desync cross-check) [built]
├─ commands/lobby.rs -> lobby_start/stop/status/query/discover/join             [built]
├─ scores.rs         -> local per-opponent ledger (observed locally)          [proposed]
├─ history.rs        -> local set/round index                                 [proposed]
└─ providers/retroarch -> command-socket plumbing (port allocation, overrides) [built]

frontend/src/components/
├─ LobbyCard.tsx     -> host/join room, room list, manual address, stop       [built; preferred home screen]
└─ RoomView.tsx      -> seats, set score, rotation state                      [proposed]
```

## 9. Open questions

- **Beacon transport and port:** ~~HTTP `GET /room` vs UDP request/response; which port; bind address.~~
  **Resolved 2026-09-21:** HTTP `GET /room`, server `tiny_http`, default port `47812`, bound to all
  interfaces; queried with `ureq`. Cross-OS reachability (firewall) remains spike S7.
- **Client seat vs. input binds:** ~~the host assigns slots at connect time, but `Role` couples
  direction and slot — a client seated as player 1 cannot bind player 1 today.~~ **Resolved
  2026-09-21:** seat is split from role via `player_slot` on the launch request (§5); the
  `lobby_join` command assigns it from room occupancy. The host updates the beacon's occupancy from
  its netplay observer so joiners can see free seats (spectators are not counted yet).
- **Host seat:** ~~does hosting always mean spectating?~~ **Resolved 2026-09-21:** `lobby_start`
  takes a `hostSeat` (`1`/`2`, or none for the table mode), defaulting to `1`. The room advertises
  its `hostSeat` and the host's held seat is included in the advertised `players`, so a joiner is
  seated in the free slot (host on 1 → joiner takes 2, and vice versa). Manual join-by-address has
  no beacon, so a manual joiner defaults to seat 1 and can collide with a host playing seat 1 — use
  the beacon path, or the host's `Spectate`/seat-2 choice.
- **Parity timing:** run the parity gate before spawning the joiner (recommended) or after? (Before —
  fail fast, no window churn.)
- **Slot race:** exact ordering/acknowledgement between the loser's release and the challenger's
  claim (§7.4 step 1/2) — **confirmed 2026-09-21**: `NETPLAY_GAME_WATCH` frees the slot immediately
  and the host auto-grants the next claim, so the release-then-claim order in §7.4 works.
- **Two spectators wanting the same slot:** strictly FIFO, or a "next up" prompt? (FIFO recommended.)
- **Spectator leaving mid-set:** does the queue shift, and does the loser still rotate out?
- **Group's ROM set:** v1's watcher targets `sfiii3nr1`; the design is per-ROM, so what is the real
  target list, and is per-ROM RE acceptable for each?
- **Always-on "table" machine:** do we designate one, or accept host-laptop dependency?
- **Does the lobby ship before Phase 4 (packaging)?** It absorbs the remaining Phase 3 acceptance
  (F11, 2 concurrent spectators on the tailnet) — see §11.
- **Setting surfaces:** new settings proposed — beacon port, command-port base, default first-to-N.
  Names/TBD.

## 10. Risks and tradeoffs

- **Per-ROM RAM detection** is per-game and version-sensitive; a core/ROM update can silently break
  it. Mitigation: store the core revision with the address; re-validate on version change.
- **Host-as-spectator** is the load-bearing unknown; the whole rotation depends on it and it was a
  declared non-goal in the previous spike (F18). It is now spike item S1.
- **Unauthenticated local command socket** is the control plane (accepted; loopback-only).
- **Abandoning FightCade** removes the only existing automatic result source. That is the price of
  automatic detection on RetroArch; it is knowingly paid.
- **No host migration** means a single point of failure per session (accepted; a designated table
  machine is the mitigation if it becomes annoying).
- **Beacon adds a listening socket** and per-OS firewall surface (spike item S7).
- **Replays are non-portable** across core/ROM updates and capture player behavior (consent/UI).
- **Privacy:** real handles/tailnet names must never be written into this public repo.
- **Root cause of v1 is inferred**, not proven. If the spike shows discovery was the true failure,
  the beacon is redesigned before any lobby core work.

## 11. Relationship to the roadmap

- **Build gate cleared locally (2026-09-21):** spike items S1–S5 pass on one machine (see
  [`10-lobby-spike.md`](10-lobby-spike.md) §1a); the **release gate** S6 (tailnet soak) and S7
  (beacon) remain, with manual join as S7's fallback.
- **Phase 2** was the room/KotH/ledger stack, **removed 2026-09-21**. The lobby is its replacement,
  but starts fresh and decentralized.
- **Phase 3** ("spectate integration; gate: 2 concurrent spectators") is *code-complete* — the
  RetroArch provider already launches host/client/spectator with per-role configs and a parity gate.
  What remains is the **acceptance run** (2 concurrent spectators over the tailnet, still deferred as
  F11). The lobby's stability soak **absorbs** this, so the lobby subsumes the rest of Phase 3.
- **Phase 4** (docs + packaging) is unaffected and may ship before or after the lobby; the open
  question is the ordering (§9).

## 12. Build order

Gated on the spike ([`10-lobby-spike.md`](10-lobby-spike.md)): the **build gate S1–S5 passed locally
on 2026-09-21** (host-as-spectator, live switching both directions, health/round RAM detection). The
**release gate S6–S7** (tailnet soak, beacon reachability) is still pending; S7 has a manual-join
fallback. Steps 2–4 build order:

1. **Hardware spike** — S1–S5 **done locally**; S6–S7 pending online. (Build gate cleared; release
   gate open.)
2. **Lobby core** — room lifecycle, beacon (+ manual join), parity pre-check, control-socket plumbing.
   **Done 2026-09-21:** control-socket plumbing (`providers/retroarch/command.rs`); the room model
   (`lobby/room.rs`), the HTTP beacon (`lobby/beacon.rs`), the host launch path (seated by default,
   `Spectate` for the non-playing server), the `lobby_*` commands, and the **client join flow** (`lobby_join`, with the seat/bind split
   of §5 and host occupancy advertised from the netplay observer). `LobbyCard.tsx` hosts/joins rooms
   and joins by address. `RoomView.tsx` (seats/set score) still belongs to step 3.
3. **Sets + rotation** — first-to-N, FIFO, automatic winner-stays using the RAM watcher. **Result
   detection started 2026-09-21:** `lobby/results.rs` reads the `sfiii3nr1` health/round addresses and
   classifies rounds; the first-to-N set machine and rotation remain.
4. **History + scores** — local, per-machine, from own observation.
5. **Replays** — only after the `.replay`-during-netplay spike (S8) passes.

## 13. Reference links (upstream source, as read 2026-09-21)

- Live role switch: `network/netplay/netplay_frontend.c` — `netplay_toggle_play_spectate` (`:8164`),
  `netplay_handle_play_spectate` (`:4968`), `RARCH_NETPLAY_CTL_GAME_WATCH` (`:10170`).
- Command interface: `command.h` `map[]` (`:498`–`563`) — `NETPLAY_GAME_WATCH`, `READ_CORE_MEMORY`,
  `GET_STATUS`, `RECORD_REPLAY`, `PLAY_REPLAY_SLOT`, `SEEK_REPLAY`.
- Replay subsystem: `input/bsv/bsvmovie.c`, `tasks/task_movie.c`.
- Defaults: `config.def.h` — `network_cmd_enable=false`, `network_cmd_port=55355`,
  `netplay_start_as_spectator=false`, `netplay_max_connections=3`.
- Local: [`04-design.md`](04-design.md) §5 (what was removed), [`07-retroarch-spike.md`](07-retroarch-spike.md)
  §2/§9 (frozen set, F18), [`06-redesign.md`](06-redesign.md) §2 (FightCade dependency).
