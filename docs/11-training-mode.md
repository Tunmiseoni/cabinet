# Training mode (research)

Status: **research writeup only — no code, no commitment.** This document records how Fightcade's
own training mode works and how the two community Lua training projects work, plus the constraints
that would gate any The Cabinet "Practice" feature. Nothing here is scheduled; per the repo
convention (`04-design.md` §3/§6) a training feature would need a **/grill-me** session before it
enters the roadmap. It is deliberately separate from [`09-lobby.md`](09-lobby.md) (online rooms) and
[`10-lobby-spike.md`](10-lobby-spike.md) (lobby hardware gate).

Purpose: the question "can we add a training mode?" was asked during the lobby spike. The short
answer is **yes, and cheaply — but only on the FightCade/FBNeo path, only offline, and only if the
script licensing is settled.** The mechanism already exists inside the emulator the app launches;
this doc captures it so the decision is not re-researched later.

> Working assumption: "Fight Kid" = **FightCade 2**. The primary ROM is `sfiii3nr1` (Street Fighter
> III: 3rd Strike), the same one used everywhere else in this repo.

---

## 1. What "training mode" means here

Training mode is a **local, single-machine practice mode**: one human, a scripted dummy, and
quality-of-life tools (health/meter refill, input recording, hitbox display). It is **not** netplay
and **not** a lobby feature.

That matters because the two implementations below work by **injecting inputs and writing emulator
RAM every frame**. Running either during a netplay session would desync or corrupt state, so training
is a separate offline launch, never a role in a direct/quark match.

## 2. How Fightcade does it

Fightcade does **not** implement training natively. It bundles and launches a community Lua script.

| Fact | Evidence |
|---|---|
| Fightcade v2.1.22 added a `/**training` chat command that launches **peon2/fbneo-training-mode** on supported games | Fightcade v2.1.22 release post (Patreon) |
| The installed emulator has a `quark:training` mode | `strings /Applications/FightCade2.app/Contents/MacOS/emulator/fbneo/fcadefbneo.exe` → `quark:training`, format `quark:training,%[^,],%[^,],%d,%d` |
| The bundled script path is hardcoded | same binary → `fbneo-training-mode/fbneo-training-mode.lua` |

So "Fightcade's own training mode" = the client spawning the emulator in `quark:training` mode, which
loads a bundled copy of the peon2 script. The `quark:training` argument fields are **not fully
characterised** here (the regex has four fields after the mode: two strings and two ints); it needs a
spike to confirm before it could be reproduced externally. It is likely a local/offline mode (no peer
endpoint appears in the format, unlike `quark:direct`).

## 3. The community training scripts

Both are **FBNeo Lua scripts**, run by the emulator's Lua engine (`Game → Lua Scripting → New Lua
Script Window → run`). Neither is a Fightcade feature; Fightcade just bundles one of them.

### 3.1 `peon2/fbneo-training-mode` (multi-game framework)

- **Shape:** one main script (`fbneo-training-mode.lua`) plus a per-game file
  (`games/<game>/<game>.lua`) that supplies memory addresses and HUD config. Adding a game means
  adding one file, which is why it covers so many titles (Street Fighter III: 3rd Strike among
  them).
- **Features:** health/meter refill (instant or gradual), infinite timer, stun control, damage/
  combo HUD, savestates, per-game HUD element placement, stun/colour config tables.
- **How it reads state:** `rb`/`wb`/`ww` (read/write byte/word) against ROM-specific addresses;
  a `Run()` function is called once per frame; `OnSaveStateLoad()` re-applies constants.
- **The 3rd Strike file** (`games/sfiii3/sfiii3.lua`) uses **parent-`sfiii3` offsets** that do
  **not** apply to `sfiii3nr1`:
  - `p1health = 0x2068D0B`, `p2health = 0x20691A3` (read `00` on our ROM)
  - `p1direction = 0x2068C76`, `p2direction = 0x2068C77`
  - `timer = 0x2011377` (`timemax = 0x63` = 99), forced via `clockControl()`
  - This is the exact +3-offset mismatch already recorded as **L7** in
    [`10-lobby-spike.md`](10-lobby-spike.md) §6 (their health `0x02068D0B`/`0x020691A3` vs our
    `0x068D08`/`0x0691A0`). **Never blind-copy parent-ROM offsets.**

### 3.2 `Grouflon/3rd_training_lua` (3rd Strike, deep)

- **Shape:** SF3-specific and much richer than peon2. Latest documented version **v0.10
  (2022-05-29)**; targets 3rd Strike (Japan 990512) on Fightcade FBNeo (its README says Fightcade
  v2.0.91 / "last tested" v2.0.97.44).
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
  also be passed on the emulator command line — see §4.

## 4. The launch hook (the useful finding)

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

If it holds, The Cabinet can start training directly — the same way it already starts a direct
match — without the Fightcade client. The launcher already appends args after the quark string
(`launcher/macos.rs:66` → `["fcadefbneo.exe", quark, "-w"]`; the Linux/Windows layouts are the
same), so a training spec is a matter of replacing the quark string and adding the script path.

## 5. Comparison

| | Fightcade bundled (peon2) | 3rd_training_lua |
|---|---|---|
| Games | many (framework) | 3rd Strike only |
| Input recording / replay slots | basic | 8 slots, random/ordered/repeat, save/load |
| Dummy counter-attack | yes | yes, frame-1, per situation |
| Hit/hurt/throwbox display | no | yes |
| Input history / frame advantage | partial | yes |
| Memory addresses for `sfiii3nr1` | **parent `sfiii3` — wrong for our ROM** | 3rd Strike-specific, but `sfiii3nr1` vs 990512 must still be verified |
| License | unverified | **none declared (see §6)** |
| Maintained | yes | last release 2022 |

## 6. Constraints and gates

1. **Offline only.** Both scripts inject input and write RAM every frame; they cannot run during
   netplay without desync. Training is its own local launch (no peer, no port).
2. **FightCade/FBNeo path only.** These scripts use the **FBNeo Lua engine**. The RetroArch FBNeo
   *core* has no equivalent Lua scripting environment, so a RetroArch training mode is not the same
   feature and is out of scope here.
3. **Per-ROM, per-revision addresses.** Every function depends on hardcoded memory addresses. A
   core/ROM change silently breaks it (the same version-sensitivity risk as the lobby's RAM result
   detection, `04-design.md` §8). The `sfiii3nr1` ↔ parent-`sfiii3` offset gap is already documented
   (L7); do not assume the script's ROM names match ours.
4. **Licensing is the hard gate for a public repo.** The repo is **public** and MIT
   (see `AGENTS.md`). `Grouflon/3rd_training_lua` reports **no license** (`license: null` via the
   GitHub API → default all-rights-reserved), and `peon2/fbneo-training-mode`'s license was not
   verifiable at research time. **Neither script can be committed, bundled, or redistributed** in
   this repo without the author's permission. The peon2 copy that ships inside a FightCade install
   is Fightcade's business, not ours. A Cabinet feature could only (a) load a script the user
   installed themselves, and/or (b) link to the upstream project, never vendor it.
5. **Scope.** A "Practice" launch is a new product surface (launch mode, no peer, script discovery/
   path, failure UX). Per the repo convention it needs a /grill-me gate before implementation.

## 7. If it were built (sketch only)

Not a commitment — just the smallest shape that fits the current architecture:

- A **Practice** launch option that spawns the FightCade FBNeo binary with no `quark:direct` peer
  and either `quark:training` or `<rom> --lua <script>` (per §4's confirmed token order).
- **Bring-your-own script:** the app points at a user-supplied Lua path (Settings), never a bundled
  one, keeping the license question off the repo.
- The existing session capture/logging, ROM index, and Cabinet-mode windowing apply unchanged; the
  result-watcher/lobby code is irrelevant (offline).
- Explicitly **out of scope:** RetroArch-path training, bundled scripts, and any training feature
  that touches a netplay session.

## 8. Open questions / derived tasks

| # | Question | Why it blocks |
|---|---|---|
| T1 | Exact `quark:training` argument semantics (4 fields) on the 2.1.45 binary | Needed only if we reproduce Fightcade's own mode rather than the `.lua` path |
| T2 | Confirm `<rom> --lua <path>` actually loads both the ROM and the script on 2.1.45 | It is the proposed launch mechanism; a spike settles it |
| T3 | Does `3rd_training_lua` v0.10 work unmodified on `sfiii3nr1` (vs Japan 990512), and with a TorrentZipped ROM? | Determines whether our ROM can use the rich script at all |
| T4 | Verify the peon2 `sfiii3.lua` health/timer offsets against `sfiii3nr1` (L7) | If we ever point at peon2's script, its parent-ROM offsets are wrong as-is |
| T5 | License + written permission status of both scripts | The only hard blocker for anything that ships |
| T6 | Where a user-installed script lives and how the app finds it | Product/UX decision for a future grill |

## 9. References

- Fightcade's bundled training: `peon2/fbneo-training-mode` — <https://github.com/peon2/fbneo-training-mode>
  (3rd Strike file: `games/sfiii3/sfiii3.lua`).
- Rich 3rd Strike training: `Grouflon/3rd_training_lua` — <https://github.com/Grouflon/3rd_training_lua>.
- FBNeo command-line Lua loading: `finalburnneo/FBNeo`, `src/burner/win32/main.cpp:1265`.
- Memory map cross-check and the L7 offset caveat: [`10-lobby-spike.md`](10-lobby-spike.md) §1a/§6.
- FightCade dependency/provider model: [`04-design.md`](04-design.md) §1, [`06-redesign.md`](06-redesign.md) §2.
- Launchers that would carry the training args: [`launcher/macos.rs`](../src-tauri/src/launcher/macos.rs),
  [`launcher/linux/layouts.rs`](../src-tauri/src/launcher/linux/layouts.rs),
  [`launcher/windows.rs`](../src-tauri/src/launcher/windows.rs).
