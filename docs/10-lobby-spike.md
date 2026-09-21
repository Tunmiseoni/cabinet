# Tailnet lobby spike (pre-lobby hardware validation)

Status: **local build gate run 2026-09-21 — S1–S5 PASS on one macOS machine over loopback; S6/S7
pending (online).** This is the gate for [`09-lobby.md`](09-lobby.md) — the lobby design. The gate is
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
| S6 | | duration: | pending |
| S7 | | | pending |
| S8 | | | not run |
| S9 | | | not run |

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
| L10 | The sets + FIFO rotation layer advertises a rotation instruction the host derives from its netplay observation, but two inputs are unverified: whether the host's log actually carries a joining **spectator's** nick (`connections`, from `Got connection from: "nick"`), and whether a host that toggles to spectator clears its own `self_player`. Without the first, the queue is empty and rotation never fires; without the second, the host reconciler cannot step the host out. | On the next 2-player + 1-spectator tailnet run, watch the host's captured netplay log: confirm spectator nicks appear and that the queue populates; toggle the host play↔spectate and confirm `self_player` clears. If spectator nicks are absent, have joiners include their nick in the beacon query and register it host-side. | Medium (blocks reliable automatic rotation only; manual play is unaffected). |

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
| S9 fails | Character select keeps SF3's native winner-lock; the boundary just waits for both peers to coin+start (or use `LOAD_STATE`), and the app does not force a reset. |

## 8. Reference

- Design: [`09-lobby.md`](09-lobby.md).
- Prior spike and frozen set: [`07-retroarch-spike.md`](07-retroarch-spike.md) §2, §3, §9 (F11, F18).
- Upstream source references: [`09-lobby.md`](09-lobby.md) §13.
