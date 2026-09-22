# Tailnet lobby spike (pre-lobby hardware validation)

Status: **local build gate run 2026-09-21 — S1–S5 PASS on one macOS machine over loopback; S6/S7
pending (online).** The **sets/rotation layer was validated live on one machine (loopback) on
2026-09-22** (see §5/§6 L10–L17), and a **RAM round-phase map** now gates result counting (L12).
The CPU-match discriminator (L15) and the `RESET` boundary (L17) remain open. This is the gate for [`09-lobby.md`](09-lobby.md) — the lobby design. The gate is
split: **S1–S5 are the build gate** (one machine, loopback — they are protocol/RAM questions, not
network ones) and **S6–S7 are the release gate** (need all peers online / all three OSes). No lobby
product code until the build gate passes; the lobby must not ship until the release gate passes (S7
has a manual-join fallback, so it may lag).

Purpose: de-risk the four unknowns the lobby depends on, on real hardware, before building. Each is a
fact we could not settle from source alone, and getting any of them wrong invalidates a design
decision, not just an implementation detail.

| # | Unknown | Why it blocks the design |
|---|---|---|
| S1 | Host-as-spectator (a non-playing server) works | The whole rotation needs the host to step out of play; the previous spike declared it a **non-goal** and never tested it (07-F18). |
| S2/S3 | Live play↔spectate switching works, both directions | Replaces "relaunch on every rotation." If it fails, the rotation design changes entirely. |
| S4 | The host can itself toggle out of play | The host is one of the players; if it can't rotate, "loser becomes spectator" is impossible for the host. |
| S5 | We can read a decisive round result from RAM | There is no generic result signal; automatic detection is the *only* accepted result source. |
| S6 | Netplay stays connected under the target load | v1's room "dropped" (cause unnamed — suspected network); this is the regression proof. |
| S7 | The discovery beacon is reachable across all three OSes | Discovery "not working well" is not root-caused; if it was firewall/socket, the beacon must be redesigned. |
| S8 | `.replay` recording works during netplay | Replays are deferred behind this; **optional** for the lobby gate. |
| S9 | A game-boundary `RESET` returns both peers to character select | Character select is native winner-locked; the lobby needs a synced boundary it can trigger without relaunching. |

This spike folds in **07-F11** (2 concurrent spectators over the tailnet), which was the last unmet
Phase 3 acceptance criterion.

> Not to be confused with [`07-retroarch-spike.md`](07-retroarch-spike.md) (Phase 0) or the redesign's
> **R0 window-topology spike** ([`06-redesign.md`](06-redesign.md) §3.5).

---

## 1. Exit criteria

| # | Criterion | How it is judged |
|---|---|---|
| S1 | Host-as-spectator serves two playing clients | Host launched `-H` + `netplay_start_as_spectator = "true"`; clients join as **player 1/2**; the host holds **no player slot** (sends no input); both clients play; no desync over a full set |
| S2 | Client player→spectator live | `NETPLAY_GAME_WATCH` to the client's command port frees its slot (host log), the instance keeps rendering, and sends no input |
| S3 | Client spectator→player live | `NETPLAY_GAME_WATCH` to a spectator's command port; host auto-grants the free slot (`NETPLAY_CMD_MODE`); the instance sends input and stays synced |
| S4 | Host player→spectator live | Host leaves play; two clients (one toggled in from spectator) play on; host remains the server; no desync |
| S5 | Per-ROM round result read from RAM | A decisive round is detectable from known `sfiii3nr1` addresses; **both peers read the same value** |
| S6 | Stability soak | 2 players + 2 spectators over the tailnet, **≥ 20 min**, with zero unintended disconnects, CRC failures, or timeouts |
| S7 | Beacon reachable on all three OSes | A listener on the planned beacon port answers a query from every other machine; firewall rules documented |
| S8 | (Optional) Replay during netplay | A netplay match recorded host-side replays deterministically; both players' inputs present |
| S9 | Game-boundary `RESET` | The host's `RESET` resets both peers in sync to a fresh select screen (a client `RESET` is refused); record `RESET` vs `LOAD_STATE` |

**Build gate (local): S1–S5.** **Release gate (online): S6–S7.** S8 and S9 can be deferred (replays
phase / sets+rotation) without blocking the lobby core. Note S1's original "host outbound ≈ 0 B/s" wording was wrong — a
non-playing host is still a relay and forwards both clients' traffic; the real signal is that it holds
no player slot.

## 1a. Confirmed on the local run (2026-09-21)

| Item | Result |
|---|---|
| S1 | **PASS** — host spec log: `... has joined as player 1` / `player 2`; host held no slot; no desync |
| S2 | **PASS** — host log `Player ... has left the game`; client stayed connected and kept rendering |
| S3 | **PASS** — host auto-granted (`... has joined as player 1`); client kept its connection |
| S4 | **PASS** — host stayed the server while spectating; a spectator took the freed slot; clients played on |
| S5 | **PASS (method changed)** — see the addresses below |

**S5 result source — the design's assumption was wrong.** FBNeo does **not** expose a libretro memory
map, so `READ_CORE_MEMORY` returns `READ_CORE_RAM 0 -1 no memory map defined`. The usable command is
`READ_CORE_RAM` (`retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM)`, **512 KB**, `0x00000`–`0x7FFFF`,
code-address base `0x02000000`). Also, there is **no dedicated per-fighter win counter** in that
512 KB: the byte that looked like one (`0x010D28`) increments once per completed round **regardless of
who won**, and resets each match — it is the round counter. The decisive result is read from health.

| Signal | RAM address (offset) | Behavior |
|---|---|---|
| P1 current health | `0x068D08` (mirror `0x068D0E`) | `0xA0` at round start; falls on damage; **saturates to `0xFF` on KO** |
| P2 current health | `0x0691A0` (mirror `0x0691A6`) | same |
| Rounds completed this match | `0x010D28` | `0,1,2…`, one per round, resets at match start |

Result detection: a round ends when a health byte reaches `0xFF` (KO — that side lost) or the round
counter advances; on a timer expiry with neither at `0xFF`, the higher health wins; both at `0xFF` is
a draw. Every peer reads the identical values (verified on host/client/spectator). The lobby derives
set scores from these events rather than reading a counter.

Independent public research agrees: the FBNeo training-mode `sfiii3.lua` and the MAME cheat data
expose **no** P1/P2 win byte and drive round logic from health + timer. Their health offsets are the
parent-`sfiii3` values and read `00` on `sfiii3nr1` (see task L7).

**Live-match signal (2026-09-22, same core).** The game exposes its own round phase, which the
watcher now gates on (`lobby/results.rs`), plus a character-select countdown:

| Signal | `READ_CORE_RAM` offset | Behavior |
|---|---|---|
| Round phase (`game_phase`) | `0x0154A4` | `0x01` character select / round intro; `0x02` round live; `0x06`–`0x09` round-end/continue sequence |
| Round counter | `0x010D28` | Completed rounds of the **current arcade match**; it does **not** move during versus matches (both versus round ends read `0`), so it is diagnostic, not a trigger (L18) |
| P2 control type | `0x069104` (P2 struct `+0x03`; P1 at `0x068C6C`) | `0x01` while the fighter is human-controlled, `0x00` when the game drives it (arcade/CPU, attract demo) |
| Continue countdown | `0x0154FC` | `0x32` (50) counting down to `0` on the post-match continue screen; whether character select shares it is unverified |
| Match context byte | `0x015572` | Changes only at match boundaries and differs per match (`0x14`/`0x00` in the two sampled 2P matches; `0x02`/`0x09`/`0x15`/`0x16`/`0x19`/`0x1A`/`0x1B` across arcade matches; `0x01` idle) — stage/opponent progression, **not** a player-vs-CPU flag (L15) |

Every KO edge sampled (8/8) landed at phase `0x06` with the previous sample at `0x02`, so the
watcher latches "live" and keeps it through the round-end sequence; testing `phase == 0x02` at the
edge itself would miss every KO.

## 2. Reference set and prerequisites

- **Frozen parity set:** [`07-retroarch-spike.md`](07-retroarch-spike.md) §2 (RetroArch 1.22.2; FBNeo
  core `GIT6bb3167` per OS with the recorded sha256s; `sfiii3nr1.zip` sha256/CRC; content CRC
  `0x46119843`). Do not let any machine auto-update its core during the spike.
- **Machines:** macOS (`mac-host`, `100.64.0.1`), CachyOS (`cachyos-host`, `100.64.0.2`), Windows
  (`windows-host`, `100.64.0.3`). All four peers (2 play, 2 watch) must be online for S6.
- **Ports:** netplay TCP **55435** (must match on all peers — 07-F20); command interface UDP
  **55355** default, **reassign per instance** on a machine running more than one emulator (the
  developer loopback pair) — see S2.

### Enabling the command interface

Append to the relevant per-role config (or use the harness's generated config):

```ini
network_cmd_enable = "true"
network_cmd_port   = "55355"   # distinct per running instance on one host
```

Send a command from the shell (UDP; the reply returns to the source address):

```sh
printf 'NETPLAY_GAME_WATCH\n'                 | nc -u -w1 127.0.0.1 55355
printf 'READ_CORE_MEMORY 0x000000 4\n'        | nc -u -w1 127.0.0.1 55355
printf 'GET_STATUS\n'                         | nc -u -w1 127.0.0.1 55355
```

Candidate commands and their upstream definitions are in [`09-lobby.md`](09-lobby.md) §3 and §13.

## 3. Test items

### S1 — Host-as-spectator (non-playing server)

1. Launch the host instance with `-H` **and** `netplay_start_as_spectator = "true"`.
2. Connect two clients as players (no spectator flag).
3. Confirm host logs both joins as players (not `Connected to`/spectator), host outbound ≈ 0 B/s.
4. Play a full first-to-2 set; confirm no desync and no CRC timeout lines.

**Pass:** two clients play a complete set with a non-playing server relaying. **Fail fallback:** if
RetroArch cannot serve two players from a non-playing host, the "table" must be a playing host, and
the rotation model must be revisited (`09-lobby.md` §9).

### S2 — Client player→spectator live

1. With S1's topology running, send `NETPLAY_GAME_WATCH` to a **playing client's** command port.
2. Observe: the host logs the player leaving its slot; the client stays connected and renders; its
   outbound bytes drop to ≈ 0.

**Pass:** slot freed, instance keeps watching, no disconnect.

### S3 — Client spectator→player live

1. Send `NETPLAY_GAME_WATCH` to a **spectating client's** command port (with a free slot available).
2. Observe: host auto-grants (`NETPLAY_CMD_MODE`); the instance starts sending input; no desync.

**Pass:** seat claimed live. **Fail fallback:** if the host refuses or the instance desyncs, the
rotation must relaunch the challenger's instance instead (accept the reconnect cost).

### S4 — Host player→spectator live

1. Start a host that is **playing** (P1). Send `NETPLAY_GAME_WATCH` to the host's command port.
2. A spectator toggles in (S3). Confirm the host remains the server while spectating and the two
   clients play on.

**Pass:** host can rotate out without dropping the room. This is the item that reverses 07-F18.

### S5 — RAM watcher on `sfiii3nr1`

1. Stand up a host (S1 topology) and reach a known round transition.
2. Locate the round/win counter address: read candidate ranges with `READ_CORE_MEMORY`, or use
   FBNeo's memory map / cheat tables; diff values across a decisive round versus a non-decisive one.
3. Assert: a decisive round increments **exactly one** side; a draw increments neither.
4. Read the same address on a **second peer** and confirm identical values at the same point.

**Pass:** a stable address whose single-side increment marks a game win, readable identically on both
peers. **Record:** the address **and** the core revision it was validated against. **Fail fallback:**
if no stable address is found for `sfiii3nr1`, automatic detection cannot ship and the result source
must be re-decided (`09-lobby.md` §7.6) — this is the biggest open risk in the design.

### S6 — Stability soak (folds in 07-F11)

1. macOS/Linux/Windows: host (non-playing per S1, or playing if S1 failed) + 1 client + **2
   concurrent spectators**, over the tailnet.
2. Run **≥ 20 minutes**; capture per-machine logs and timestamps.
3. Watch for: `A netplay client has disconnected`, `Netplay disconnected`, CRC failures, timeout
   lines, and the v1 symptom (a session that appears to die with no local cause).

**Pass:** all four stay connected for the full soak with no unintended disconnect; spectators claim
no player slot. This is the acceptance the previous spike left open (07-F11) and the regression test
for the deleted room's drop behavior.

### S7 — Beacon reachability

1. Stand up a trivial listener on the planned beacon port on each OS (placeholder for the real
   beacon).
2. From every other machine, query it; record latency and failures.
3. On failure, add/diagnose the per-OS firewall rule; document the required rules.

**Pass:** the beacon answers from all three OSes with a documented firewall story, **or** the manual
host-address fallback is declared mandatory (per `09-lobby.md` §5). **Fail fallback:** if beacon
discovery is unreliable, ship manual join first and treat discovery as a later investigation.

### S8 — (Optional) Replay during netplay

1. With a netplay session running, send `RECORD_REPLAY` to one instance's command port.
2. Play a set, then `HALT_REPLAY`; locate the `.replay` file.
3. Play it back (`PLAY_REPLAY_SLOT`/`SEEK_REPLAY`) on a matching core+ROM; confirm deterministic
   reproduction and that **both** players' inputs are captured.

**Pass:** recording during netplay works and captures both inputs. Otherwise replays stay out of
scope until the format/approach is re-decided.

### S9 — Game-boundary `RESET` for character select

1. With a host + one client in netplay (S1/S2), send `RESET` to the **host** instance's command
   socket and confirm via `GET_STATUS`/logs that it resets and that the client follows in sync
   (`NETPLAY_CMD_RESET`); confirm a `RESET` sent to the **client** is refused (NAK).
2. Repeat at a real game boundary driven by the RAM watcher, then coin+start into a fresh character
   select on both peers.
3. If the reset flow feels clunky, probe the fallback: `LOAD_STATE` (netplay-synced
   `RARCH_NETPLAY_CTL_LOAD_SAVESTATE`) to a per-ROM "fresh match" savestate and compare.

**Pass:** the host can return both peers to a fresh select screen at a game boundary without
relaunching. **Record:** which mechanism (`RESET` vs `LOAD_STATE`) and the core revision.

**Live finding 2026-09-22 (R2):** `RESET` to the host's command socket is a no-op on the local build
— three attempts produced no `[Core] Reset.` log and no RAM change on any of the four in-sync peers,
while `NETPLAY_GAME_WATCH` toggled a client cleanly. See L17; S9 stays open.

**Live finding 2026-09-22 (R4):** `RESET` is a no-op on a **standalone** instance too (the new
`retroarch-spike.sh solo` role — no netplay; no log line, `phase` unchanged), so netplay is not the
cause. The state path works (`SAVE_STATE` wrote a 10 MB state; `LOAD_STATE_SLOT 0` loaded it back),
but a state load during netplay **drops the clients** — the client logged
`[ERROR] [Netplay] Netplay state load out of order!` and disconnected (libretro/RetroArch#7767).
S9's pass condition (return both peers to a fresh select without relaunching) is therefore
unreachable with this build's command interface; the boundary must come from a relaunch or the
game-native coin/continue flow.

**Resolved 2026-09-22 (R5):** the game-native route works and needs no reset. A mid-arcade challenger
coin-in flips P2 to human (`0x069104` → `0x01`), the in-progress CPU round plays out, and character
select + the versus match follow (19:34:51 → 19:35:40 → 19:35:45), both windows playable. S9 passes
through the game's own flow; the remaining app-side work is the "waiting for coin" prompt for the
**queue head** (L16).

## 4. Procedure / order

1. Freeze the reference set on all machines (07 §2); confirm content CRC `0x46119843`.
2. Bring up macOS host + Linux client on loopback/local first, then add the Windows machine.
3. Run **S1** before anything else — it is the load-bearing assumption.
4. Run **S2 → S3 → S4** as one live-switch sequence against the S1 topology.
5. Run **S5** alongside S1/S4 (round transitions occur during normal play).
6. Schedule the **S6** tailnet soak last, when all four peers are online; run **S7** opportunistically.
7. **S8** only if replays are being pulled forward; **S9** when sets + rotation are built.

## 5. Evidence log

| Item | Topology | Observed | Result |
|---|---|---|---|
| S1 | 1 machine, loopback: `hostspec` + 2 clients | Host logged both joins as player 1/2; host held no slot; no desync | **PASS** |
| S2 | S1 topology | `NETPLAY_GAME_WATCH` → host `Player ... has left the game`; client stayed connected | **PASS** |
| S3 | S1 topology | `NETPLAY_GAME_WATCH` → host `... has joined as player 1` (auto-grant); no disconnect | **PASS** |
| S4 | 1 machine: playing host + client + spectator | Host spectated, stayed server; spectator took player 1; clients played on | **PASS** |
| S5 | S1 topology + a full first-to-2 set | Health `0x068D08`/`0x0691A0`; loser → `0xFF`; round counter `0x010D28`; identical on all peers | **PASS (method changed)** |
| R1 | 1 machine, loopback: app host (P1) + `spike-p2` + `spike-spec`/`spike-spec2` | Queue built from the host's netplay log (`Got connection from: "spike-spec"` → `queue: ["spike-spec"]`); a real set end advertised `rotation {loserSlot: 1, incoming: "spike-spec"}`; the host stepped itself out (`<host> steps out`, `self_player` cleared) and the freed slot was auto-granted to the incoming spectator; rounds rolled up to games/set correctly. R1b: stepping the host out preserved the waiting queue after the L11 fix (`["spike-spec", "spike-spec2"]` before and after), and stepping it back in restored its seat. **Not proven here:** the promoted player could not play — run 1's scripted spectator had no player-1 binds (harness fixed 2026-09-22), run 2 then lost rounds entirely when `0x010D28` stayed at `0` (L13), and the match boundary is not coin-aware (L12). **Superseded by R6:** a promoted player took the freed seat and coined in, once the L19 sequencing fix was applied. | **PASS (rotation mechanism only)** |
| R2 | 1 machine, loopback: app host (P1) + `spike-p2` + 2 spectators; ~10 min at 1 s RAM sampling | Phase map validated: all 8 KO edges at phase `0x06` (prev `0x02`); select countdown at `0x0154FC`; `0x015572` changes only at match boundaries. The runner counted the CPU/arcade rounds after the 2P match ended and announced `set won in slot 1; rotation 31` at 16:07:59Z from them. `RESET` to the host: no `[Core] Reset.` log and no RAM change on any of the four in-sync peers, while `NETPLAY_GAME_WATCH` toggled a client cleanly | **PASS (phase map) / FAIL (CPU gate, `RESET`)** |
| R3 | 1 machine, loopback: app host (P1) + `spike-p2`; labelled 2P → arcade → attract demo | P2 struct `+0x03` (`0x069104`) read `0x01` through the whole 2P match, flipped `0x00` at the match end (18:22:35) and stayed `0x00` through both arcade matches; in the attract demo both players read `0x00` (P1 flipped at game over, 18:29:04). With the phase-only gate the app counted 5 arcade rounds (bogus) | **PASS (discriminator)** |
| R4 | 1 machine, loopback: app host (P1) + `spike-p2`; scripted 2P match (KO then timeout) → arcade (Ibuki, Yun) → idle | **Control gate works:** the run's only outcome was the 2P KO (17:50:48Z); the two arcade matches, the boot attract demo (18:50:01, `phase 0x02`, both control bytes `0x00`) and the post-loss state produced **zero** outcomes. **New finding (L18):** the round-2 timeout (18:52:29, `phase 0x02 → 0x06`, `p1 A0` / `p2 77`, `rounds` still `0`) produced **no** outcome — the counter never moves in versus play, so the counter-based timeout was dead. | **PASS (control gate) / FAIL (timeout → fixed)** |
| R5 | 1 machine, loopback: app-shaped host (P1, launched from `netplay-p1.cfg`) + `spike-p2` client | **The challenger coin-in is the boundary.** P2's coin (`0x069104` → `0x01`) at 19:26:35 started a versus match from the title; after that match ended (19:29:34) the game dropped P2 back to CPU and the same coin re-opened select → versus. The decisive run: **mid-arcade** (`mode 0x16`, `phase 0x02`, P1 at 64 health) P2's coin at 19:34:51 flipped `p2_ctrl → 0x01`, the in-progress CPU round played out, select at 19:35:40, versus round live at 19:35:45 — **no reset, no relaunch, no savestate**, both windows playable (round 2 at 19:37). | **PASS (boundary solved by the game)** |
| R6 | 1 machine, loopback: app host (P1) + `spike-p2` (P2) + `spike-spec` (queue head); a real first-to-2 set won by P1 | **Three-person rotation run.** Set won 19:49:56 → `rotation 9 {loserSlot: 2, incoming: "spike-spec"}`. The rotation's two toggles raced (L19): the incoming's seat request beat the loser's step-out, so it was granted **device 2** ("You have joined as player 3"), not seat 2 — the seat stayed empty (`seats: ["player-one", null]`, `p2_ctrl 0x00` for ~9 min) and the room showed both players in the queue. Re-sent **after** the seat freed, the same toggle took seat 2 instantly (`You have joined as player 2`), and the promoted player's coin at 19:59:07 flipped `p2_ctrl → 0x01`, round intro 19:59:53, versus round live 19:59:58 — the challenger flow with a **promoted** player, not a fresh joiner. **Queue order (L20):** after the rotation the queue read `["spike-p2", "spike-spec"]` — the rotated-out loser at the head, so the next rotation would have handed the seat straight back to the loser. | **PASS (seat handover + promoted-player coin) / FAIL (toggle race → fixed, queue order → fixed)** |
| S6 | | duration: | pending |
| S7 | | | pending |
| S8 | | | not run |
| S9 | R2 topology (host + client + 2 spectators, all in sync) | `RESET` to the host three times: no `[Core] Reset.` log and no RAM change on any peer; `NETPLAY_GAME_WATCH` toggled a client cleanly in the same session. Standalone `RESET` is a no-op too (R4); `LOAD_STATE_SLOT` disconnects netplay clients (R4) | **FAIL (no-op) — superseded by R5: the boundary is the incoming player's own coin-in** |

## 6. Derived tasks

| # | Finding (evidence) | Task | Priority |
|---|---|---|---|
| L1 | `READ_CORE_MEMORY` returns `no memory map defined` on FBNeo; `READ_CORE_RAM` works (512 KB). | Implement result reads with `READ_CORE_RAM`; correct the command in `09-lobby.md` §3/§7.6. | High |
| L2 | No per-fighter win counter exists in the exposed RAM; `0x010D28` is the round counter. | Derive set scores from health + round transitions, not a counter. Update the design. | High |
| L3 | The command socket services ~1 command/frame (~60/s); a full 512 KB read takes ~34 s. | Read only the needed windows; for bulk analysis use `SAVE_STATE` (RAM lives at state offset `0x214`). | Medium |
| L4 | Live role switch toggles with a single `NETPLAY_GAME_WATCH`; the host auto-grants a free slot. | Implement rotation as toggles; verify the slot is free before promoting a spectator. | High |
| L5 | The copied RetroArch config had **all `input_player2_*` bindings `nul`**, so the P2/client instance had no keys. | Ensure the launch path binds P2 input (the app must not inherit an unbound P2). | Medium — **Superseded 2026-09-21 (L8):** binding `input_player2_*` was the wrong fix for the wrong reason; netplay samples `input_player1_*`. |
| L8 | **Live 2026-09-21:** after the host-seat change, a host seated at **P1** had controls but a **joining client seated at P2** did not — its config bound only `input_player2_*`. Root cause: `get_self_input_state` (`network/netplay/netplay_frontend.c`) captures a participant's local input from the **first local device of the matching type** (local device 0 -> `input_player1_*`), not from the player slot it was assigned; the slot is chosen by `netplay_request_device_pN`. | Bind the keyboard preset to `input_player1_*` for **any** seated instance (plus an `input_player2_*` duplicate for seat 2, as a hedge); set `netplay_request_device_p1`/`p2` from the host's seat so "play as P2" is real; leave clients unset so RetroArch auto-assigns the free slot. | High — **Fixed 2026-09-21 (v0.1.10).** |
| L6 | Host-as-spectator and live switching all work, and the loser's health saturates to `0xFF`. | Record `0x068D08`, `0x0691A0`, `0x010D28` + core `GIT6bb3167` in the design; re-validate on core/ROM change. | Medium |
| L7 | External research (FBNeo training-mode `sfiii3.lua`, MAME cheats) independently agrees there is **no** P1/P2 win byte and that result logic uses health + timer. It also lists timer `0x02011377`, match state `0x020154A6`, and player structs `0x02068C6C`/`0x02069104` (0x498 apart — matches ours). Its health `0x02068D0B`/`0x020691A3` read `00` on our ROM (ours read `A0`); they are parent-`sfiii3` offsets, **+3** from `sfiii3nr1`. | Validate `0x011377` (timer) and `0x0154A6` (match state) on `sfiii3nr1` in a live round — the timer may be a cleaner round-transition trigger than `0x010D28`. Never blind-copy parent-ROM offsets. | Medium |
| L9 | **Live 2026-09-21 (Linux friend, `misc/logs-linux-21-09-2026`):** a perfectly good frozen FBNeo core launched and then netplay was refused with `[ERROR] [Netplay] Core does not support netplay.` Root cause is **not** the core build or RetroArch's version: `init_netplay` (`network/netplay/netplay_frontend.c`) consults `core_info_current_supports_netplay()`, and RetroArch resolves a loaded core's capability metadata by **basename** (`path_basename_nocompression(core_path)`) against its own **scanned** core list (`core_info_find_internal`, `core_info.c`). Our managed core lives under the app data dir, outside that list, and the friend had no other FBNeo installed, so `core_info_list_get_info` left a **`memset` zeroed** entry (`savestate_support_level = 0`) and `core_info_load` returned false without repairing it → refused. The macOS peer only passed by luck: it happens to have `~/Library/Application Support/RetroArch/cores/fbneo_libretro.dylib` (no `.info`), and a scanned core with a missing `.info` defaults to `SAVESTATE_DETERMINISTIC` (`core_info.c` ~2128). This holds on 1.14.0 *and* 1.22.2 alike; `core_info_savestate_bypass` (a newer setting) does not exist on 1.14.0. | Guarantee a `fbneo_libretro.<ext>` exists in the directory RetroArch scans (`libretro_directory`, else the per-OS default) with a deterministic `.info` beside it — install the frozen core on launch preflight when the basename is missing, never clobbering an existing file — and keep loading our own core for parity. Surface `Core does not support netplay` (and the platform-dependent variant) in the tracker. | High — **Fixed 2026-09-21** (`core::ensure_core_visible`, launch preflight). |
| L10 | **Resolved 2026-09-22 (loopback, R1).** Both inputs verified in one run: the host's log carries a joining **spectator's** nick (`Got connection from: "spike-spec"`), so the beacon queue populated (`queue: ["spike-spec"]`); and a host that toggles to spectator clears its own `self_player` (beacon `seats[0]` → `null`). A real set end drove the full rotation: `set won in slot 2; rotation 12 -> Some("spike-spec")` → `<host> steps out` (the host's own reconciler toggled it) → the freed slot was auto-granted (`spike-spec has joined as player 1`). | No further action — validated locally; the online run (S6/S7) should re-confirm on the tailnet. | Closed. |
| L11 | **Live 2026-09-22 (loopback, R1):** the host's step-out wiped its own waiting queue. `NetplayTracker::feed` treated `You have left the game` as a session end (`connections.clear()` + `set_connection(Disconnected)`), but RetroArch emits that line when an instance leaves its **player slot** (play→spectate) — exactly the rotation's loser step-out. Beacon evidence: queue `["spike-spec"]` → `[]` at the same tick while the spectator was still connected; with two waiters the second would never rotate in, quietly breaking repeated rotation. | Clear only `self_player` on `You have left the game`; keep `connections` and the connection state, and reserve the full clear for `Netplay disconnected`. Regression test `netplay::tests::leaving_the_game_keeps_the_waiting_spectators`. Re-verified live: host step-out kept queue `["spike-spec", "spike-spec2"]`, and step-in restored its seat. | High — **Fixed 2026-09-22.** |
| L12 | **Live-observed 2026-09-22 (loopback):** the watcher is not **coin/match-phase aware**. SF3 requires a coin for **every** match, and coining in is what opens character select and starts it; the winner is then auto-locked (winner-lock). Observed: after a win, P1's character was auto-selected and could not be re-chosen; a player promoted into a vacated slot held the seat but could not coin in/start the next match; and with only one seated player the watcher still emitted outcomes from a non-match state (run 1: three outcomes between the set end and the promoted player joining). The app reads only health + `0x010D28`, so it can neither prompt for a coin nor tell a live round from a continue/select/attract screen. | Model the match phase and the coin step: gate outcome counting on a live-match signal (validate `0x0154A6` match state / `0x011377` timer per L7), surface an explicit "waiting for coin" state between matches and after a rotation, and force the boundary with the S9 `RESET`/`LOAD_STATE`. **Phase gate landed 2026-09-22 (R2):** `RoundWatcher` latches on `phase == 0x02` and clears on the next select/intro, so a KO edge that was never live cannot decide a round, while edges seen in the round-end sequence still count (all 8 sampled KO edges were `0x06` after `0x02`). The seat-count guard that stood in for the match signal was removed once the P2 control byte landed (L15). The forced boundary turned out to be unnecessary (R5: the challenger coin-in **is** the boundary); the "waiting for coin" state/prompt remains open. | High — counting is phase- and control-gated; the prompt half remains. |
| L13 | **Live-observed 2026-09-22 (loopback, R2):** result detection stalls when `0x010D28` does not advance. `RoundWatcher` only emits when `rounds` increases, so a round whose counter byte stays at `0` is dropped **even though health saturated to `0xFF`**. Evidence: 13:52–13:57, P2 KO'd six-plus times with `rounds: 0` on every poll and **zero** outcome lines; the set froze at 0–1, so no set end and no rotation — the player's wins were invisible to the app. | **Fixed 2026-09-22 (partial):** emit on the non-KO → `0xFF` edge (immediate, deduplicated, counter no longer required); regression test `a_ko_counts_even_when_the_round_counter_stays_at_zero`. Follow-up found at once: with immediate emission the lone host's **attract/demo** KOs are counted (beacon showed `p1Games: 1` before any joiner), so the set runner now only observes while two players are seated and resets the watcher otherwise. The proper gate is the in-game match state/timer — re-validate `0x011377`/`0x0154A6` (L7). **Superseded 2026-09-22 (R2):** the in-game phase is the gate (L12); `0x010D28` is no longer required for KOs and the seat guard was later removed too (L15). The counter is diagnostic only now — it never moved the timeout path either (L18). | High — **Fixed 2026-09-22** (phase-gated). |
| L14 | **Live-observed 2026-09-22 (loopback, R3):** the rotated-out **loser was dropped from the waiting queue**. When a peer leaves its player slot (the loser's own rotation step-out) the host logs `Player <nick> has left the game`, and `NetplayTracker` removed it from `connections`; `observe_netplay` rebuilds the queue from `connections`, so the loser could never return and the FIFO only ever held peers that had never played. Evidence: after rotation 21 the beacon showed `queue: []` while the rotated-out spectator was still connected. | Keep the connection on slot-leave (remove only the player, FIFO order preserved so the loser re-queues at the back); reserve `remove_connection` for `"<nick>" has disconnected`. Regression tests `a_player_leaving_its_slot_stays_a_waiting_connection` / `quoted_disconnect_removes_the_player`. | High — **Fixed 2026-09-22.** |
| L15 | **Live-observed 2026-09-22 (loopback, R2):** CPU/arcade matches are counted as set rounds. The 2P match ended at 16:05:12Z (KO; the match-context byte `0x015572` left `0x14` for `0x02` at 16:05:24Z), the loser did not coin, and the winner fought the CPU on. The runner kept emitting outcomes (16:06:48Z, 16:07:59Z) and announced **`set won in slot 1; rotation 31` at 16:07:59Z from those CPU rounds**, then kept counting the following arcade rounds (16:09:44Z–16:13:11Z). The round phase cannot separate them — arcade rounds are live rounds too. | **Resolved 2026-09-22 (R3).** The P2 player struct carries the game's own control type at struct `+0x03` (`0x069104`): `0x01` while P2 is human, `0x00` when the game drives it. Evidence: `0x01` through the whole 2P match, flipped `0x00` at the match end (18:22:35), stayed `0x00` through both arcade matches; in the attract demo **both** fighters read `0x00` (P1 flipped at game over, 18:29:04). The watcher now arms only when the round is live **and** P2 reads `0x01`; regression tests `a_cpu_driven_p2_is_not_a_result`, `a_cpu_timeout_is_not_a_result`, `the_arcade_round_after_a_2p_match_is_not_a_result`. The seat-count guard was removed — the game's own signal supersedes it. | High — **Fixed 2026-09-22.** |
| L16 | **Live-observed 2026-09-22 (loopback, R2):** the rotation fires against a game that is no longer at a match boundary. The set end is computed from the last counted round (here CPU rounds, L15) while the game is in the post-match/continue state — the winner is auto-locked in character select and nothing starts until a coin (L12). After rotation 31 the game sat at `phase 0x09`, `p1_health 0xFF` from 16:14Z until the session was killed at 16:54Z, with no new match: the seat rotated, but play could not resume. | **Solved by the game 2026-09-22 (R5):** the boundary is the incoming player's own coin-in — mid-arcade, the coin flips P2 to human (`0x069104` → `0x01`), the in-progress CPU round plays out, then character select and the versus match follow, with no reset. The app half remains: surface the "waiting for coin" state and prompt the **queue head** to coin in, so the seat handover (not whoever presses start first) decides who plays. | High — **boundary solved; the prompt/UX half is open.** |
| L17 | **Live-observed 2026-09-22 (loopback, R2):** `RESET` over the network command interface is a **no-op** on this build (RetroArch 1.22.2 / FBNeo `GIT6bb3167`). Sent to the host's command socket three times: no `[Core] Reset.` line in the emulator log and no RAM change (`phase`/health/round counter) on any of the four in-sync instances, while `GET_STATUS` answers and `NETPLAY_GAME_WATCH` toggles a client cleanly in the same session. The flag never reached `retroarch.c`'s `CMD_EVENT_RESET` (which does `core_reset()` + `netplay_driver_ctl(RARCH_NETPLAY_CTL_RESET)` and logs); `PAUSE_TOGGLE`/`FAST_FORWARD` also showed no effect, so the state-command path itself is suspect. **Tested further 2026-09-22 (R4, standalone + netplay):** `RESET` is a no-op on a **standalone** instance too (new `solo` spike role, no netplay, window unfocused: no `[Core] Reset.` log, `phase` unchanged), so netplay is not the cause. The state-command path itself works — `SAVE_STATE` wrote a 10,295,424-byte state and `LOAD_STATE_SLOT 0` (action command) loaded it back (both logged under `[State]`) — but loading a state **during netplay drops the clients**: the host's load reached the client as `[ERROR] [Netplay] Netplay state load out of order!` then `[Netplay] Netplay disconnected` (known upstream: libretro/RetroArch#7767). | **Resolved 2026-09-22 (R5):** option (b) works — a mid-arcade **challenger coin-in** flips P2 to human and the game returns to character select for the versus match (coin 19:34:51 → select 19:35:40 → versus live 19:35:45), no reset, relaunch, or savestate. S9's pass condition is met by the game's own flow; the app only has to prompt the queue head (L16). | Closed — **the challenger coin-in is the boundary.** |
| L18 | **Live-observed 2026-09-22 (loopback, R4):** the round counter `0x010D28` does not advance during versus matches, so the counter-based timeout never fires. Across the whole 2P match the counter read `0` at both round ends (KO 18:50:48, timeout 18:52:29) and only moved once the game was in an arcade match (18:53:51, `0x00 → 0x01`), by which point P2 read CPU and the watcher was disarmed; the timeout round was silently dropped. | **Fixed 2026-09-22 (R4):** the timeout now hangs off the phase transition — a live round (`0x02`) entering the round-end sequence (`0x06`–`0x09`) without a KO byte has run out of time, decided by the health at the transition. A per-round `decided` flag keeps a lagging KO byte (or a KO seen while the phase was still live) from deciding the same round twice. Regression tests: `the_recorded_versus_timeout_counts` (replays the recorded samples), `a_lagging_ko_byte_after_a_timeout_decision_is_not_counted_twice`, `a_ko_seen_while_still_live_does_not_double_count_at_the_transition`. The counter is now diagnostic only. | High — **Fixed 2026-09-22.** |
| L19 | **Live-observed 2026-09-22 (loopback, R6):** the rotation's two `NETPLAY_GAME_WATCH` toggles are independent, and the incoming's seat request races the loser's step-out. RetroArch grants a request the next free **input device**, not the losing seat: with the host on device 0 and the loser still holding device 1, the incoming was granted **device 2** — announced as "player 3", invisible in a two-seat room, and the game's P2 stayed CPU (`p2_ctrl 0x00`). Re-sent after the seat freed, the same toggle took seat 2 ("You have joined as player 2"). Two app clients have the same race: their beacon polls are independent (0–1.5 s apart), so either request can land first. | `reconcile_rotation` now waits until the room reports the losing seat **free** before the incoming toggles in (retrying each poll, logging once, and leaving the rotation unmarked until it acts). Regression tests: `the_queue_head_waits_while_the_losing_seat_is_occupied`, `the_queue_head_steps_in_after_the_loser_steps_out`. **Open tail:** if the loser never steps out (its app died while its instance stays connected) the incoming waits indefinitely; the room/UI should surface a stuck rotation. | High — **Fixed 2026-09-22.** |
| L20 | **Live-observed 2026-09-22 (loopback, R6):** the waiting queue was rebuilt in **connection** order, so a player who stepped out of a seat re-entered at the *front*. After the R6 rotation the queue read `["spike-p2", "spike-spec"]` — the loser `spike-p2` ahead of the spectator who had been waiting all along — so the next set end would have rotated the loser straight back in and the "3rd person" would never get the seat. (The code comment claimed the loser belongs at the back; the implementation did not do it.) | The netplay tracker keeps a `waiting` list ordered by when each nickname started waiting: joiners enter at the back, a player who steps out re-enters at the back, and a real disconnect leaves both lists. `observe_netplay` builds the queue from it. Regression tests: `a_player_leaving_its_slot_stays_a_waiting_connection` (tracker) and `a_rotated_out_loser_queues_behind_the_spectators_already_waiting` (room). | High — **Fixed 2026-09-22.** |

## 7. Decision matrix

| Outcome | Decision |
|---|---|
| S1–S5 pass (build gate) | **Met 2026-09-21** — the lobby core may be built per [`09-lobby.md`](09-lobby.md); actual result detection uses `READ_CORE_RAM` health, not the counter the design assumed. |
| S6–S7 pass (release gate) | Ship the lobby; S7 failing only forces manual-join first. |
| S1 fails | Rotation needs a playing host/table machine; revise §4/§7.4 of the design before building. |
| S2/S3 fail | Rotation requires relaunching instances; accept reconnect churn and revise §7.4. |
| S5 fails | No automatic result source exists; result detection must be re-decided (design blocker). **Not hit** — health is a valid source. |
| S6 fails | Diagnose the disconnect before building; discovery/netplay stability may be the real v1 root cause. |
| S7 fails | Ship manual join first; beacon is a follow-up investigation. |
| S8 fails | Replays remain deferred; the lobby is unaffected. |
| S9 fails | **Finding 2026-09-22 (L17): the `RESET` path is a no-op, so this is the live branch unless `LOAD_STATE` works.** Character select keeps SF3's native winner-lock; the boundary waits for both peers to coin+start (or uses `LOAD_STATE`), and the app does not force a reset. |

## 8. Reference

- Design: [`09-lobby.md`](09-lobby.md).
- Prior spike and frozen set: [`07-retroarch-spike.md`](07-retroarch-spike.md) §2, §3, §9 (F11, F18).
- Upstream source references: [`09-lobby.md`](09-lobby.md) §13.
