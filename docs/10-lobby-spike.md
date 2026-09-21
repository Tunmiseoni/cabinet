# Tailnet lobby spike (pre-lobby hardware validation)

Status: **not run.** This is the gate for [`09-lobby.md`](09-lobby.md) — the lobby design. **No lobby
product code is written until every required item below passes** (or its fallback is chosen).

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

This spike folds in **07-F11** (2 concurrent spectators over the tailnet), which was the last unmet
Phase 3 acceptance criterion.

> Not to be confused with [`07-retroarch-spike.md`](07-retroarch-spike.md) (Phase 0) or the redesign's
> **R0 window-topology spike** ([`06-redesign.md`](06-redesign.md) §3.5).

---

## 1. Exit criteria

| # | Criterion | How it is judged |
|---|---|---|
| S1 | Host-as-spectator serves two playing clients | Host launched `-H` + `netplay_start_as_spectator = "true"`; clients join as **player 1/2**; host outbound ≈ 0 B/s; both clients play; no desync over a full set |
| S2 | Client player→spectator live | `NETPLAY_GAME_WATCH` to the client's command port frees its slot (host log), the instance keeps rendering, and sends no input |
| S3 | Client spectator→player live | `NETPLAY_GAME_WATCH` to a spectator's command port; host auto-grants the free slot (`NETPLAY_CMD_MODE`); the instance sends input and stays synced |
| S4 | Host player→spectator live | Host leaves play; two clients (one toggled in from spectator) play on; host remains the server; no desync |
| S5 | Per-ROM round result read from RAM | A decisive round increments exactly one side at a known `sfiii3nr1` address; **both peers read the same value** |
| S6 | Stability soak | 2 players + 2 spectators over the tailnet, **≥ 20 min**, with zero unintended disconnects, CRC failures, or timeouts |
| S7 | Beacon reachable on all three OSes | A listener on the planned beacon port answers a query from every other machine; firewall rules documented |
| S8 | (Optional) Replay during netplay | A netplay match recorded host-side replays deterministically; both players' inputs present |

S1–S7 are the gate. S8 can be deferred to the replays phase without blocking the lobby.

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

## 4. Procedure / order

1. Freeze the reference set on all machines (07 §2); confirm content CRC `0x46119843`.
2. Bring up macOS host + Linux client on loopback/local first, then add the Windows machine.
3. Run **S1** before anything else — it is the load-bearing assumption.
4. Run **S2 → S3 → S4** as one live-switch sequence against the S1 topology.
5. Run **S5** alongside S1/S4 (round transitions occur during normal play).
6. Schedule the **S6** tailnet soak last, when all four peers are online; run **S7** opportunistically.
7. **S8** only if replays are being pulled forward.

## 5. Evidence log

| Item | Topology | Observed | Result |
|---|---|---|---|
| S1 | | | |
| S2 | | | |
| S3 | | | |
| S4 | | | |
| S5 | | address: | |
| S6 | | duration: | |
| S7 | | | |
| S8 | | | |

## 6. Derived tasks

Fill in as the spike runs; mirror the 07-F table style.

| # | Finding (evidence) | Task | Priority |
|---|---|---|---|
| L1 | — | — | — |

## 7. Decision matrix

| Outcome | Decision |
|---|---|
| S1–S7 all pass | Build the lobby per [`09-lobby.md`](09-lobby.md) in its stated order. |
| S1 fails | Rotation needs a playing host/table machine; revise §4/§7.4 of the design before building. |
| S2/S3 fail | Rotation requires relaunching instances; accept reconnect churn and revise §7.4. |
| S5 fails | No automatic result source exists; result detection must be re-decided (design blocker). |
| S6 fails | Diagnose the disconnect before building; discovery/netplay stability may be the real v1 root cause. |
| S7 fails | Ship manual join first; beacon is a follow-up investigation. |
| S8 fails | Replays remain deferred; the lobby is unaffected. |

## 8. Reference

- Design: [`09-lobby.md`](09-lobby.md).
- Prior spike and frozen set: [`07-retroarch-spike.md`](07-retroarch-spike.md) §2, §3, §9 (F11, F18).
- Upstream source references: [`09-lobby.md`](09-lobby.md) §13.
