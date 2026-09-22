# Training mode (research)

Status: **research writeup only — no code, no commitment.** This document records how Fightcade's
own training mode works, how the two community Lua training projects work, and — added 2026-09-22 —
how a **RetroArch FBNeo** practice mode could be built by The Cabinet itself. Nothing here is
scheduled; per the repo convention (`04-design.md` §3/§6) a training feature would need a
**/grill-me** session before it enters the roadmap. It is deliberately separate from
[`09-lobby.md`](09-lobby.md) (online rooms) and [`10-lobby-spike.md`](10-lobby-spike.md) (lobby
hardware gate).

Purpose: the question "can we add a training mode?" was asked during the lobby spike, and then
again on 2026-09-22 as **"a RetroArch instance of FBNeo with training mode, launched from The
Cabinet."** There are therefore **two distinct paths**, and the 2026-09-22 investigation changed the
answer to the RetroArch one:

- **Path A — FightCade standalone FBNeo + a community Lua script.** Deep (scripted dummy, hitboxes,
  input history), but **offline-only, FightCade-path-only, and licensing-gated** (neither script
  declares a license).
- **Path B — RetroArch FBNeo core, driven by The Cabinet.** Shallower (RAM-value aids, not scripted
  behaviour), but it fits the stack the app already ships, needs no third-party assets, and was
  previously written off here — incorrectly. The earlier text said "the RetroArch FBNeo core has no
  equivalent Lua scripting environment, so a RetroArch training mode is not the same feature and is
  out of scope." **The first half is confirmed; the "out of scope" conclusion is wrong.** RetroArch
  has no Lua, but it exposes a memory read/write command interface and replay controls that the app
  already knows how to speak, so the *front end* can implement the training aids the script would
  have implemented in-emulator.

> Working assumption: "Fight Kid" = **FightCade 2**. The primary ROM is `sfiii3nr1` (Street Fighter
> III: 3rd Strike), the same one used everywhere else in this repo.

---

## 1. What "training mode" means here

Training mode is a **local, single-machine practice mode**: one human, an opponent (scripted dummy,
idle opponent, or CPU), and quality-of-life tools (health/meter refill, infinite timer, input
recording, hitbox display). It is **not** netplay and **not** a lobby feature.

That distinction drives both paths:

- The community Lua implementations work by **injecting inputs and writing emulator RAM every
  frame**. Running either during a netplay session would desync or corrupt state, so training is a
  separate offline launch, never a role in a direct/quark match.
- The Path B design writes RAM from **outside** the emulator, so the same rule applies: it must
  never run during netplay (the app's own launch path enforces offline). RetroArch also explicitly
  refuses cheats while netplay is initialised.

## 2. Two paths at a glance

| | Path A — FightCade FBNeo + Lua | Path B — RetroArch FBNeo + Cabinet |
|---|---|---|
| What runs the emulator | `fcadefbneo.exe` (FightCade's standalone FBNeo, Wine on macOS) | `RetroArch` + `fbneo_libretro.<ext>` (the frozen core the app already ships) |
| What implements training | An in-emulator Lua script (`peon2` or `3rd_training_lua`) | The Cabinet's Rust backend, over RetroArch's command socket |
| Feature ceiling | Very high: scripted dummy counter/parry, hitboxes, input history, frame advantage | Low–medium: infinite time, health/meter refill, savestates, frame advance, replay review; **no** input injection, hitboxes, or dummy logic |
| Dummy opponent | Scripted (Lua input injection) | Idle P2 via a manual coin-in, or the CPU |
| Licensing | **Blocked** — both scripts are `license: null` (all rights reserved) | **Clear** — no third-party script or cheat data needed; the app writes RAM itself |
| Stack fit | New FightCade-path launch mode; does not use the frozen core | Reuses the existing RetroArch provider, core staging, command socket, per-ROM tables |
| Status | Researched, gated on permission + spikes | Researched 2026-09-22, feasible, gated on spikes + a /grill-me |

## 3. Path A — FightCade FBNeo with a Lua script

### 3.1 How Fightcade does it

Fightcade does **not** implement training natively. It bundles and launches a community Lua script.

| Fact | Evidence |
|---|---|
| Fightcade v2.1.22 added a `/**training` chat command that launches **peon2/fbneo-training-mode** on supported games | Fightcade v2.1.22 release post (Patreon) |
| The installed emulator has a `quark:training` mode | `strings /Applications/FightCade2.app/Contents/MacOS/emulator/fbneo/fcadefbneo.exe` → `quark:training`, format `quark:training,%[^,],%[^,],%d,%d` (re-confirmed 2026-09-22) |
| The bundled script path is hardcoded | same binary → `fbneo-training-mode/fbneo-training-mode.lua`; the repo also ships `fbneo-training-mode/` (a 2022-vintage copy) next to the binary |

So "Fightcade's own training mode" = the client spawning the emulator in `quark:training` mode, which
loads a bundled copy of the peon2 script. The `quark:training` argument fields are **not fully
characterised** here (the regex has four fields after the mode: two strings and two ints); it needs
a spike to confirm before it could be reproduced externally. It is likely a local/offline mode (no
peer endpoint appears in the format, unlike `quark:direct`).

### 3.2 The community training scripts

Both are **FBNeo Lua scripts**, run by the emulator's Lua engine (`Game → Lua Scripting → New Lua
Script Window → run`). Neither is a Fightcade feature; Fightcade just bundles one of them.

#### 3.2.1 `peon2/fbneo-training-mode` (multi-game framework)

- **Shape:** one main script (`fbneo-training-mode.lua`) plus a per-game file
  (`games/<game>/<game>.lua`) that supplies memory addresses and HUD config. Adding a game means
  adding one file, which is why it covers so many titles (Street Fighter III: 3rd Strike among
  them).
- **Features:** health/meter refill (instant or gradual), infinite timer, stun control, damage/
  combo HUD, savestates, per-game HUD element placement, stun/colour config tables, recording slots.
- **How it reads state:** `rb`/`wb`/`ww` (read/write byte/word) against ROM-specific addresses;
  a `Run()` function is called once per frame; `OnSaveStateLoad()` re-applies constants. Input
  recording/replay and the dummy use the emulator's injected `joypad.set()` / `input.get()` /
  `input` tables.
- **The 3rd Strike file** (`games/sfiii3/sfiii3.lua`) uses **parent-`sfiii3` offsets** that do
  **not** map 1:1 to `sfiii3nr1` — see §4.4 for the mapping the lobby work established.
- **Maintenance update (2026-09-22):** the repo is **actively maintained** (last push 2026-07-31,
  latest release `v0.26.03`, 80 stars). The copy bundled in FightCade is older (directory dated
  2022-12-24).

#### 3.2.2 `Grouflon/3rd_training_lua` (3rd Strike, deep)

- **Shape:** SF3-specific and much richer than peon2. Latest documented version **v0.10
  (2022-05-29)**; targets 3rd Strike (Japan 990512) on Fightcade FBNeo. The repo was last pushed
  **2024-06-13**.
- **Features:** dummy counter-attacks on frame 1 after hit/block/parry/wake-up, 8 recording slots
  with save/load and random/ordered/repeat replay, hit/hurt/throwbox display, input history for both
  players, frame-advantage and damage displays, character switch, dedicated **parry / red-parry**
  training, guard-jump practice replays. It also ships a separate `3rd_spectator.lua` for replay
  viewing that does not touch input.
- **How it works:** the same FBNeo Lua model — `memory.readbyte`/`writebyte`/`writeword` plus an
  injected `input` table (`"P1 Up"`, `"P1 Weak Punch"`, …), a per-frame `Run`, and a large
  per-character frame-data database used to *predict* hitboxes and decide when the dummy should
  block/parry/counter. It reaches well beyond peon2, but only for this one game family.
- **Note:** the demo in its README is triggered from `Game → Lua Scripting`, but (like peon2) it can
  also be passed on the emulator command line — see §3.3.

### 3.3 The launch hook

**FBNeo auto-loads any command-line argument ending in `.lua`.** In upstream FBNeo's Windows
burner, the arg parser calls `FBA_LoadLuaCode(...)` for any `*.lua` argument:

```
finalburnneo/FBNeo  src/burner/win32/main.cpp:1265
  } else if (_tcscmp(&szName[_tcslen(szName) - 4], _T(".lua")) == 0) {
      // Command: lua file
      FBA_LoadLuaCode(TCHARToANSI(szName, NULL, 0));
```

The community pattern (seen in shortcut guides) is:

```
fcadefbneo.exe <rom> --lua <path\to\3rd_training.lua>
```

`--lua` itself is not a recognised switch; the `.lua`-suffixed argument is what loads. **A spike must
confirm the exact token order on the FightCade 2.1.45 binary** (does the ROM still load when a
second path-like argument is present?), because it is load-bearing for any in-app launch.

If it holds, The Cabinet could start training directly on the FightCade path — the same way it
already starts a direct match. The launchers append args after the quark string
(`launcher/macos.rs:66` → `["fcadefbneo.exe", quark, "-w"]`; the Linux/Windows layouts are the
same), so a training spec is a matter of replacing the quark string and adding the script path.

## 4. Path B — RetroArch FBNeo core, driven by The Cabinet (2026-09-22)

### 4.1 The core cannot run the scripts (confirmed)

RetroArch has **no Lua scripting engine**, and the FBNeo core does not embed one, so neither peon2
nor `3rd_training_lua` can run under `fbneo_libretro`. RetroArch also has **no network command to
inject input** (the command map has no input-setting verb), so the scripts' two load-bearing tricks —
`joypad.set()` / `input` injection and the per-frame Lua `Run()` — have no front-end equivalent.

Evidence: RetroArch 1.22.2 (local install, `Version: 1.22.2 (Git 69a4f0ea)`), the libretro FBNeo
docs, and upstream issue/PR discussion (see §9).

### 4.2 What the core does expose

The same local RetroArch build (and the `fbneo_libretro.info` metadata) confirm the pieces needed:

| Capability | Evidence |
|---|---|
| **Arbitrary RAM writes over the command socket** | Binary strings: `WRITE_CORE_RAM`, `WRITE_CORE_MEMORY` (plus the reads the app already uses). `docs.libretro` "Network Control Interface". |
| **Memory descriptors** | `fbneo_libretro.info` → `memory_descriptors = "true"`, `cheats = "true"` |
| **Deterministic savestates** | `fbneo_libretro.info` → `savestate = "true"`, `savestate_features = "deterministic"` (the app's frozen core `.info` already declares this) |
| **Input replay controls** | Binary strings: `RECORD_REPLAY`, `PLAY_REPLAY`, `PLAY_REPLAY_SLOT`, `HALT_REPLAY`, `SAVE_REPLAY_CHECKPOINT`, `SEEK_REPLAY`, `REPLAY_SLOT_PLUS/MINUS` |
| **Cheats (optional)** | Native FBNeo cheats via core options; RetroArch `.cht` via the cheat engine. **Unlicensed data** (see §6.4) — Path B does not need them. |

Two write verbs exist and must be told apart by spike:

- `WRITE_CORE_RAM <addr> <bytes…>` uses **RetroAchievements addresses** (`rcheevos_patch_address`) —
  the same space the app's `READ_CORE_RAM` already uses successfully.
- `WRITE_CORE_MEMORY <addr> <bytes…>` uses **system addresses** via the core's memory descriptors
  (`RETRO_ENVIRONMENT_SET_MEMORY_MAPS`) and is the upstream-preferred verb.

Upstream notes `READ_CORE_RAM`/`WRITE_CORE_RAM` can be brittle when a game has no achievements
loaded; the app's lobby reads have worked, but which verb reaches the validated `sfiii3nr1`
addresses must be confirmed live (spike R1).

### 4.3 What The Cabinet already has

Path B reuses a surprising amount of shipped code:

- **Command socket plumbing** — `providers/retroarch/command.rs` frames commands and parses replies,
  with a per-role command port (`network_cmd_enable`, `network_cmd_port` in the session overrides).
- **Core staging** — `providers/retroarch/core.rs` (`ensure_core_visible`, frozen SHA-256, download).
- **A per-ROM RAM table and poller** — `lobby/results.rs` (`RomAddresses`, `read_snapshot`,
  `RoundWatcher`) reads six bytes per poll on `sfiii3nr1`; a training poller is the same shape.
- **Per-session overrides** — `providers/retroarch/spec.rs` writes `netplay-<role>.cfg` and a
  generated core-options file (the SOCD pin), and already handles input binds and hotkey collisions.
- **Session launch/state** — `session.rs` (`Plan`, `MatchState`, `launch`/`launch_many`/`stop`).

The gap is small: the RetroArch launch always enables netplay (`-H`/`-C` in `spec.rs::launch_args`),
and there is no `practice` capability or training loop.

### 4.4 Training aids via the command socket (proposed)

`sfiii3nr1` addresses already validated live on the frozen core (from the lobby, `10-lobby-spike.md`
§6):

| Purpose | Address | Values |
|---|---|---|
| P1 / P2 current health | `0x068D08` / `0x0691A0` (mirrors `+0x06`) | `0xA0` full; falls on damage; saturates `0xFF` on KO |
| Round phase (`game_phase`) | `0x0154A4` | `0x01` select/intro, `0x02` live, `0x06`–`0x09` round-end |
| P1 / P2 control type | `0x068C6C` / `0x069104` | `0x01` human, `0x00` CPU |
| Round counter | `0x010D28` | diagnostic only (does not advance in versus) |
| Continue countdown | `0x0154FC` | `0x32` counting down |

Candidate addresses **derived, not yet validated** — the peon2 3rd-Strike file is for the parent
`sfiii3`, and the lobby established the mapping `sfiii3nr1 = parent − 0x2000003` (health
`0x2068D0B` → `0x068D08`). The `−0x2000003` is a 0x2000000 address-bank prefix plus the **+3
revision shift**, and whether the revision shift applies to non-struct globals (the timer) is
exactly what must be checked:

| Purpose | peon2 parent (`sfiii3.lua`) | candidate `sfiii3nr1` | Note |
|---|---|---|---|
| P1 / P2 meter | `0x20695BE` / `0x20695EB` | `0x0695BB` / `0x0695E8` | struct, so the −3 likely applies |
| P1 / P2 max meter | `0x20286AD` / `0x20286E1` | `0x0286AA` / `0x0286DE` | read the value; do not assume |
| P1 / P2 combo counter | `0x20696C5` / `0x206961D` | `0x0696C2` / `0x06961A` | struct |
| P1 / P2 facing | `0x2068C76` / `0x2068C77` | `0x068C73` / `0x068C74` | struct |
| Timer | `0x2011377` (external MAME/L7: `0x02011377`) | `0x011374` **or** `0x011377` | global — the −3 may or may not apply; the L7 note suggested `0x011377`. Validate both. |
| Match state | (external L7: `0x020154A6`) | `0x0154A6` | near the validated phase byte; validate |

With those, the front end can reproduce peon2's *core* aids without Lua:

- **Infinite timer** — write `0x63` (99) to the timer byte each poll while a round is live.
- **Health refill** — write `0xA0` to P1 and/or P2 health, per-player toggle, instant or gradual
  (gradual = step one point per poll until full).
- **Meter refill** — same, at the meter address, up to the max-meter value read at startup.
- **Optional: round/health HUD** — the app could surface P1/P2 health and timer in its own window
  (the values it is already reading), which is the closest thing to peon2's damage HUD.

**Write policy (important):**

- Only write while `phase == 0x02` and only during practice (never netplay; a host-side write would
  desync clients).
- Never write over a KO (`0xFF`); skip a health that is already `0xFF`, and skip values outside
  `0 < h < max`, so round-end and continue screens are left alone.
- Writes are idempotent holds, so a slightly different cadence than the game's frame rate is fine:
  a hit that lands is restored on the next poll (≈16–33 ms of flicker), and the value converges
  rather than drifting. Cadence and per-game interaction are spike items (R2/R3).
- `WRITE_CORE_RAM` disables RetroAchievements hardcore mode if active — irrelevant for an offline
  practice launch with no achievements.

### 4.5 Native practice tools (available without new emulator code)

- **Savestates** — the core is deterministic; the app already points `savestate_directory` at a
  session dir and only neutralizes colliding hotkeys, so save/load slots work today. A practice
  overrides file can pin `input_save_state`/`input_load_state`/`input_frame_advance` to known keys.
- **Frame advance, slow motion, rewind** — RetroArch front-end features (`FRAMEADVANCE`, slow-motion
  toggles); usable as-is, or driven from the app via `command.rs`.
- **Input replay recording/review** — `RECORD_REPLAY` / `HALT_REPLAY` / `PLAY_REPLAY_SLOT` let the
  app start and stop a `.replay` for the session. Note this records and replays *all* players'
  inputs — it plays the game back, it does **not** mix live input with a recording, so it is a
  review/reference tool, not a training dummy.

### 4.6 The dummy problem

There is no way to inject P2 input from outside RetroArch, so a *scripted* dummy (block/parry/
counter) is out of reach on Path B. The practical substitute uses the game's own flow, discovered
during the lobby spike (`10-lobby-spike.md` §6 L12/L15/L16/R5):

- In a single offline instance, an arcade board accepts a **P2 coin-in** mid-game; the game flips P2
  to human-controlled and plays out the round, then returns to character select for a versus match.
- So a practice config that binds `input_player2_select` (coin) and `input_player2_start` to a
  couple of keys lets the player **drop in a stationary training dummy with one key press** — the
  same "idle opponent" baseline a training mode gives before you configure any dummy behaviour.
- The alternative — two local netplay instances (Dev-pair style) with an idle P2 — works for an idle
  dummy, but **cannot be combined with the RAM aids**: the app's writes would desync the peers.
  Single instance + manual coin-in is the only safe home for Path B's aids.

### 4.7 What is still impossible on RetroArch

Scripted dummy counter-attacks, parry/red-parry drills, hit/hurt/throwbox display, input history,
frame-advantage/damage prediction, and any form of live input injection. Those require the in-
emulator Lua path (Path A) and are the reason Path A is not simply deleted.

## 5. Comparison

| | Fightcade bundled (peon2, Path A) | `3rd_training_lua` (Path A) | Cabinet practice (Path B) |
|---|---|---|---|
| Games | many (framework) | 3rd Strike only | `sfiii3nr1` first, extensible |
| Infinite time / health / meter | yes | yes | **proposed** (RAM writes) |
| Input recording / replay slots | basic | 8 slots, random/ordered/repeat | RetroArch `.replay` review (not live-mixable) |
| Scripted dummy counter-attack | yes | yes, frame-1, per situation | **no** (idle P2 via coin-in only) |
| Hit/hurt/throwbox display | no | yes | **no** |
| Input history / frame advantage | partial | yes | **no** |
| Savestates / frame advance / slow-mo | yes (Lua) | yes (Lua) | yes (RetroArch native) |
| Memory addresses for `sfiii3nr1` | parent `sfiii3` — needs the −0x2000003 mapping | 990512-specific, unverified for `sfiii3nr1` | validated where noted, derived candidates otherwise |
| License | `license: null` (2026-09-22) | `license: null` (2026-09-22) | **none needed** (app writes RAM itself) |
| Runs on the shipped frozen core | no | no | **yes** |

## 6. Constraints and gates

1. **Offline only.** Every mechanism here injects input and/or writes RAM; it cannot run during a
   netplay session. Training is its own local launch (no peer, no port). RetroArch also disables
   cheats while netplay is initialised.
2. **Path A is FightCade/FBNeo-only; Path B is RetroArch-only.** The Lua engine exists only in the
   standalone burner, and RetroArch has neither Lua nor input injection. They are different features
   with different ceilings; don't conflate them.
3. **Per-ROM, per-revision addresses.** Every function depends on hardcoded memory addresses; a
   core/ROM change silently breaks it (the same version-sensitivity risk as the lobby's RAM result
   detection, `04-design.md` §8). The `sfiii3nr1` ↔ parent-`sfiii3` gap is documented above — never
   blind-copy parent offsets.
4. **Licensing gates Path A, not Path B.** The repo is **public** and MIT. `Grouflon/3rd_training_lua`
   and `peon2/fbneo-training-mode` both report `license: null` (re-checked 2026-09-22 via the GitHub
   API → default all-rights-reserved), so **neither script can be committed, bundled, or
   redistributed** here without the author's permission. The same applies to the cheat-data repos
   (`finalburnneo/FBNeo-cheats` is also `license: null`), which is why Path B implements the aids
   rather than shipping cheat files. A Path A feature could only (a) load a script the user installed
   themselves, and/or (b) link to the upstream project, never vendor it.
5. **Scope.** A "Practice" launch is a new product surface (offline launch mode, training toggles,
   script/ROM discovery, failure UX). Per the repo convention it needs a /grill-me gate before
   implementation.

## 7. Proposed implementation (Path B — RetroArch Practice)

Not a commitment — the smallest shape that fits the current architecture. Decisions to settle in the
grill: the feature set, whether Practice is a normal-UI surface or developer-mode first, and whether
the address table starts `sfiii3nr1`-only.

**Backend**

- `providers/mod.rs`: add `practice: bool` to `Capabilities` (RetroArch `true`, FightCade `false`).
  Front-end `ProviderCapabilities` mirrors it (frontend/src/lib/types.ts).
- `providers/retroarch/spec.rs`: a practice variant of `launch_args` that omits `-H`/`-C`, `--port`,
  and `--nick`, and a practice overrides writer (no netplay keys; keep `network_cmd_enable` and the
  command port; keep the core-options/`savestate_directory` handling; pin practice hotkeys; bind
  `input_player1_*` plus `input_player2_select`/`start` for the dummy coin-in).
- `providers/retroarch/command.rs`: add `write_core_ram`/`write_core_memory` framing (pick the verb
  per spike R1) and keep them off the netplay path.
- New `training` module: a `TrainingConfig` (per-player health/meter toggles, infinite timer, refill
  speed) and a poller thread that, on a fixed interval, reads the phase/health/meter and writes the
  enabled holds under the §4.4 policy. Model the per-ROM table on `lobby/results.rs::addresses_for`
  and peon2's `games/<game>/<game>.lua` layout (without vendoring the file).
- `constants.rs`: a training poll interval (e.g. 16–33 ms) and any defaults.
- `commands`: a `launch_practice(rom)` command (reusing `session::launch` with a single practice
  `Plan`), plus `practice_status`/`stop`; the toggles could be a `practice_set(config)` command or a
  field on the status. Decide whether practice reuses `Role::P1` or gets its own role/paths.

**Front end**

- A `PracticeCard.tsx` (or a Practice panel inside `LaunchCard.tsx`) with the ROM picker and the
  toggles, wired through `lib/api.ts` and `lib/types.ts`. `MatchView` likely needs a practice
  variant (the existing view assumes a netplay slot).

**Per-OS**

- macOS/Linux/Windows all use the same RetroArch provider and frozen core; no per-OS launcher work
  beyond the existing core staging. The macOS practice overrides should keep the MoltenVK pin that
  the netplay path already applies (`spec.rs`).

**Out of scope**

- Path A's Lua/script features (scripted dummy, hitboxes, frame data), bundled scripts, cheat-data
  files, and any training that touches a netplay session.

## 8. Open questions / derived tasks

Path A (FightCade + Lua), unchanged except where noted:

| # | Question | Why it blocks |
|---|---|---|
| T1 | Exact `quark:training` argument semantics (4 fields) on the 2.1.45 binary | Needed only if we reproduce Fightcade's own mode rather than the `.lua` path |
| T2 | Confirm `<rom> --lua <path>` actually loads both the ROM and the script on 2.1.45 | It is the proposed Path A launch mechanism; a spike settles it |
| T3 | Does `3rd_training_lua` v0.10 work unmodified on `sfiii3nr1` (vs Japan 990512), and with a TorrentZipped ROM? | Determines whether our ROM can use the rich script at all |
| T4 | Verify the peon2 `sfiii3.lua` offsets against `sfiii3nr1` (the −0x2000003 mapping) | Path A's peon2 copy has parent-ROM offsets; Path B already uses the mapping |
| T5 | License + written permission status of both scripts (still `license: null`, 2026-09-22) | The only hard blocker for anything Path A ships |
| T6 | Where a user-installed script lives and how the app finds it | Product/UX decision for a future grill |

Path B (RetroArch practice), new 2026-09-22:

| # | Question | Why it blocks |
|---|---|---|
| R1 | Do `WRITE_CORE_RAM` and/or `WRITE_CORE_MEMORY` actually land on the frozen FBNeo core in a **standalone, no-achievements** launch, at the validated `sfiii3nr1` health addresses (read back after write)? | If neither writes, Path B's aids are dead and only the native tools remain |
| R2 | Validate the derived addresses: timer (`0x011374` vs `0x011377`), meter (`0x0695BB`/`0x0695E8`), max meter (`0x0286AA`/`0x0286DE`), combo counters, match state (`0x0154A6`) | The health/meter/timer aids are only as good as these |
| R3 | Write policy under real play: does holding health/meter/timer confuse round/KO/continue logic? Is skipping `0xFF` and gating on `phase == 0x02` sufficient? | Prevents the aid from breaking the match |
| R4 | Write cadence vs. flicker and CPU cost (e.g. 30–60 Hz UDP writes) | UX quality; whether the front end can hold values smoothly |
| R5 | Practice launch plumbing: does a netplay-less spec boot the game and open the command socket correctly? Does the `input_libretro_device_p*` setting need changing offline? | It is the launch mechanism |
| R6 | Practice hotkeys and savestate behaviour with the existing preset/collision logic; and replay commands (`RECORD_REPLAY`/`PLAY_REPLAY_SLOT`) usable from the socket | The native-tools half of the feature set |
| R7 | Dummy coin-in: which P2 binds, and does the one-time coin reliably open versus with an idle P2 in a single offline instance? | The only dummy option on Path B |

## 9. References

- Fightcade's bundled training: `peon2/fbneo-training-mode` — <https://github.com/peon2/fbneo-training-mode>
  (3rd Strike file: `games/sfiii3/sfiii3.lua`; repo `license: null`).
- Rich 3rd Strike training: `Grouflon/3rd_training_lua` — <https://github.com/Grouflon/3rd_training_lua>
  (repo `license: null`).
- Cheat data (unlicensed, not used): `finalburnneo/FBNeo-cheats` — <https://github.com/finalburnneo/FBNeo-cheats>,
  `libretro/libretro-database` `cht/`.
- RetroArch network control interface (read/write memory, replay commands):
  <https://docs.libretro.com/development/retroarch/network-control-interface/>.
- `READ_CORE_RAM`/`WRITE_CORE_RAM` vs `READ_CORE_MEMORY`/`WRITE_CORE_MEMORY` (achievement vs system
  addresses; brittleness with no achievements loaded): libretro/RetroArch#16392.
- FBNeo libretro core metadata (`cheats`, `memory_descriptors`, deterministic savestates):
  `fbneo_libretro.info`; local RetroArch 1.22.2 (`Version: 1.22.2 (Git 69a4f0ea)`) binary strings
  for the command verbs.
- FBNeo command-line Lua loading: `finalburnneo/FBNeo`, `src/burner/win32/main.cpp:1265`.
- Memory map, the −0x2000003 mapping, and the L7 offset caveat:
  [`10-lobby-spike.md`](10-lobby-spike.md) §1a/§6; validated addresses in
  [`lobby/results.rs`](../src-tauri/src/lobby/results.rs).
- RetroArch provider / command socket / core staging:
  [`providers/retroarch/`](../src-tauri/src/providers/retroarch/) (`command.rs`, `core.rs`, `spec.rs`).
- Launchers that would carry Path A's training args: [`launcher/macos.rs`](../src-tauri/src/launcher/macos.rs),
  [`launcher/linux/layouts.rs`](../src-tauri/src/launcher/linux/layouts.rs),
  [`launcher/windows.rs`](../src-tauri/src/launcher/windows.rs).
