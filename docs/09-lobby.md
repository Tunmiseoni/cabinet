# Design: The Cabinet — Tailnet lobby (rooms, sets, spectating rotation)

Status: **design agreed in a `/grill-me` session (2026-09-21); nothing implemented.** This document
supersedes the removed rooms/KotH subsystem ([`04-design.md`](04-design.md) §5) for the *replacement*
lobby. It is a design, not a spec: [`04-design.md`](04-design.md) remains the top-level spec and will
be updated as the pieces land. The **build order is gated on a hardware spike**, specified separately
in [`10-lobby-spike.md`](10-lobby-spike.md) — read that before writing code.

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
| `READ_CORE_MEMORY <addr> <len>` | Read emulator RAM — the automatic result signal (§7.6). |
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

- Creating a room **launches** the host instance. By default the host runs as a **non-playing
  spectator server** (the "table"), so the host machine is not forced onto a controller and can
  rotate players freely.
- "Waiting in the lobby" = the host emulator sits at attract / character-select until a client joins.
- The host chooses the **ROM** and the **first-to-N** when creating the room. Both are fixed for the
  room's lifetime.
- The room dies when the host emulator dies (§7.8). There is no persistence and no idle state.
- The host may toggle *into* play later (it is just another instance using the same control path),
  and can toggle back out — this is what makes winner-stays rotation possible when the host is one of
  the two players.

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
- **Transport is an open question** (§9): a tiny `GET /room` (HTTP over the tailnet) is easier to
  test with `curl` and to reason about than v1's UDP probe datagram, but adds a listening socket and
  per-OS firewall surface. UDP request/response is lighter but harder to debug.
- **Manual fallback is mandatory in v1:** a host can paste/hand out `host-ip:netplay-port`, and a
  joiner can enter it directly. A broken or blocked beacon must never prevent play.

**Join flow:** query beacon → show room (host handle, ROM, phase, occupancy) → joiner confirms →
**parity gate** (frontend version, core revision, content CRC — the gate already exists in
`providers/retroarch/parity.rs`) using the beacon's ROM identity → spawn the joiner's instance as a
client/spectator (`-C <host>`). If parity fails, the joiner is told before anything launches.

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
| Server | The room's host instance | Default: non-playing. May toggle into/out of play. |
| Player | A client claiming a device slot | Two player slots (P1/P2-equivalent). |
| Spectator | A client with `netplay_start_as_spectator = "true"` | Sends no input; many allowed. |

### 7.2 Room lifecycle

1. Host creates a room: picks ROM + first-to-N; app launches the host instance as a non-playing
   server and starts the beacon.
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
- The **winner keeps their slot/side**; only the other slot changes hands. Sides are the two player
  device slots; the winner is not re-seated.
- A **draw** (double KO / timeout with equal rounds) is recorded but does **not** advance the set and
  does **not** rotate anyone; the same two players replay it.
- The set ends when one side reaches N; that transition triggers the rotation in §7.2 step 4.

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

- **Signal:** poll the relevant RAM address(es) with `READ_CORE_MEMORY <addr> <len>` and detect the
  game's round/win counters advancing. This is **the** automatic signal; there is no generic one.
- **Cost accepted:** it is **per-ROM reverse engineering**, version-sensitive to core/ROM updates.
  Start with **`sfiii3nr1`** (Street Fighter III: 3rd Strike — the ROM the group plays now).
- **Both players observe independently.** Because the game is deterministic and synced, both
  machines should compute the *same* result. If they disagree mid-session, that is a **desync alarm**
  — surface it, do not silently pick a winner.
- **The round/win address for `sfiii3nr1` is not yet located** — finding it is the spike's job
  ([`10-lobby-spike.md`](10-lobby-spike.md) S4). Until it exists, automatic detection does not exist,
  and the rotation cannot be considered desktop-complete.
- Detection feeds: the set counter, the rotation trigger (§7.4), lifetime scores (§7.9), and
  history (§7.10).
- **How the address is found (investigation sketch):** use FBNeo's memory map / cheat tables, or
  narrow a candidate address by watching round transitions (read a range, diff across a known round
  win). Record the address *and* the core revision it was found against.

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

## 8. Module map (proposed, not built)

Indicative only; names may change. It slots into the existing `src-tauri` layout without reviving
any deleted module.

```
src-tauri/src/
├─ lobby/
│  ├─ room.rs        -> room model (rom, firstTo, seats, phase, node ids)
│  ├─ beacon.rs      -> serve + query the read-only discovery beacon
│  ├─ control.rs     -> the RetroArch command socket (NETPLAY_GAME_WATCH, READ_CORE_MEMORY, …)
│  ├─ sets.rs        -> set/rotation state machine (first-to-N, FIFO)
│  └─ results.rs     -> per-ROM RAM watcher (address table + desync cross-check)
├─ scores.rs         -> local per-opponent ledger (observed locally)
├─ history.rs        -> local set/round index
└─ providers/retroarch -> gains command-socket plumbing (port allocation, overrides)

frontend/src/components/
├─ LobbyCard.tsx     -> create/join room, room list
└─ RoomView.tsx      -> seats, set score, rotation state
```

## 9. Open questions

- **Beacon transport and port:** HTTP `GET /room` vs UDP request/response; which port; bind address.
- **Parity timing:** run the parity gate before spawning the joiner (recommended) or after? (Before —
  fail fast, no window churn.)
- **Slot race:** exact ordering/acknowledgement between the loser's release and the challenger's
  claim (§7.4 step 1/2) — needs the spike's live-switch confirmation.
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

- **Phase 2** was the room/KotH/ledger stack, **removed 2026-09-21**. The lobby is its replacement,
  but starts fresh and decentralized.
- **Phase 3** ("spectate integration; gate: 2 concurrent spectators") is *code-complete* — the
  RetroArch provider already launches host/client/spectator with per-role configs and a parity gate.
  What remains is the **acceptance run** (2 concurrent spectators over the tailnet, still deferred as
  F11). The lobby's stability soak **absorbs** this, so the lobby subsumes the rest of Phase 3.
- **Phase 4** (docs + packaging) is unaffected and may ship before or after the lobby; the open
  question is the ordering (§9).

## 12. Build order

Gated on the spike ([`10-lobby-spike.md`](10-lobby-spike.md)) passing its exit criteria:

1. **Hardware spike** — host-as-spectator, live role switch both directions, RAM watcher on
   `sfiii3nr1`, connection stability, beacon reachability. (No product code until this passes.)
2. **Lobby core** — room lifecycle, beacon (+ manual join), parity pre-check, control-socket plumbing.
3. **Sets + rotation** — first-to-N, FIFO, automatic winner-stays using the RAM watcher.
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
