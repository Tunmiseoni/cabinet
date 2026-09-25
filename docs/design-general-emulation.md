# Design: The Cabinet — general multi-game session coordinator

Status: **design agreed in a `/grill-me` session on 2026-09-25; not implemented.**
This document re-scopes The Cabinet from a **Tailnet FBNeo / FightCade netplay launcher** into a
**modular session coordinator for arbitrary games** played by a small friend group across macOS,
Linux, and Windows. It supersedes the *scope* framing of [`04-design.md`](04-design.md) and is the
prerequisite for the implementation work tracked separately in
[`12-fightcade-ripout.md`](12-fightcade-ripout.md) (the first, mechanical step).

> This doc is the record of the grilling session and the resulting design. `04-design.md` remains
> the historical spec for the FightCade-launcher era; `01`–`03` remain the original troubleshooting
> record. The repository is public — this file must keep using the sanctioned placeholders from
> `04-design.md` §8 and never contain real addresses, handles, ROMs, or machine paths with personal
> data.

## 1. Why this exists (the shift)

The app was built around one game, one emulator, one network, and one transport: FBNeo under
FightCade, launched with `quark:direct`, over Tailscale, with RetroArch netplay as the spectator
path. The new goal is to play **different ROMs of different games with the same friends**, where the
game itself — not the app — determines how a shared session is run.

That changes what the app is:

| | Before (FightCade launcher) | After (session coordinator) |
|---|---|---|
| The game | Street Fighter III (`sfiii3nr1`) | arbitrary games |
| The emulator | FBNeo under FightCade's Wine/Flatpak | per-game, per-OS runners (RetroArch, Crossover, Proton, native, …) |
| The network | Tailscale, assumed | transport-agnostic; Tailscale is *one* reachability option |
| The transport | `quark:direct` (and RetroArch netplay) | one of three drivers, chosen per game |
| The config | hardcoded SF3 logic | declarative per-game profiles |
| The app's job | launch + connect + spectate | vary per game, from "monitor the network" to "set up the whole emulation stack" |

Two consequences drive everything below:

1. **Netplay and streaming are co-equal session architectures, not primary/fallback.** Ultimate
   Ninja Storm under Crossover (macOS) and Proton (Linux) is the same Windows build on both
   machines, so a host-stream is the only sane shared-session model — it has no deterministic
   netplay to lean on.
2. **Modularity must be data, not code.** The friend group is tiny, the repo is public, and custom
   profiles are shared by hand. Executable plug-ins would add ABI churn, multi-OS distribution, and
   untrusted-code loading for no benefit.

## 2. Grilling record (2026-09-25)

The session proceeded in rounds. This is the decision trail, including the reversals.

### Round 1 — re-scope and rip out FightCade

| Q | Decision |
|---|---|
| Target scope | **(d): arbitrary games**, per-OS/per-game emulation paths; not a fixed title or system set. |
| Session architecture | Netplay and external streaming (Parsec/Sunshine) are different architectures; **the game determines which**. |
| Lobby survival | The SF3 set/rotation/result code is irreducibly SF3-specific; keep it *as SF3-specific*, do not generalize it. |
| FightCade | **Full rip-out** — provider, launcher adapters, `fightcade_dir`, and FightCade ROM defaults all go; ROMs move to app config. |

### Round 2 — close the taxonomy

| Q | Decision |
|---|---|
| Session modes | A **small closed set** of architectures the app knows how to run; per-game emulation detail is a *property* of a mode, not a new mode. |
| App behaviour | **Varies per game**, from "ensure the game networks over the tailnet" to "set up the full emulation stack" (the SF3 case). |
| Profile form | **Declarative data (JSON). No code plug-ins.** |
| Core vs variable | The invariant-core / per-profile-variable split (below) approved **as a starting point**. |

### Round 3 — architecture and streamer

| Q | Decision |
|---|---|
| Driver set | **Three transport drivers, closed**: `native_online`, `netplay` (a *family* — a profile binds a specific emulator's netplay), `stream`. |
| Streamer | **Sunshine + Moonlight, managed.** Parsec only as an optional `stream/unmanaged` profile (coordination-only). |
| `native_online` scope | **v1 = direct peer IP only.** Broadcast-discovery and central-service interception are per-game research, not driver features. |

Rationale for Sunshine over Parsec: Parsec is closed and account-bound, its macOS hosting requires
manual permission grants that **cannot be scripted**, its CLI only *connects* (`parsecd peer_id=…`),
and it carries its own NAT traversal — which makes Tailscale, and therefore most of this app,
redundant. Sunshine is open, account-free, scriptable, and connects to an **IP**, which slots
directly onto a Tailscale `100.x` address.

### Round 4 — the profile model

| Q | Decision |
|---|---|
| Profile keying | **`gameId` with `variants[]`**; each variant binds its own driver + per-OS runner + content selector. Driver can differ per variant of the same game. |
| Step ownership | **Driver owns the ordered steps; the profile supplies parameters** and `assist` markers where the OS forces a human prompt. Rejected: a profile-authored general workflow engine. |
| Per-game logic | Referenced as **named built-in detectors/strategies by id + params** (e.g. `fbneo-ram-ko`). Detection only exists under the **netplay** driver. |
| Rotation | **Parked in v1** (`rotation: "none"`); re-introduced later as a *referenced strategy* (possibly several). |
| Profile tiers | **Bundled read-only presets + a user custom directory.** Custom shadows preset by `gameId`+variant; schema-versioned; invalid fails loudly. |

### Round 5 — drivers, hooks, rotation, layering

| Q | Decision | Note |
|---|---|---|
| One driver or several? | **Exactly one *transport* driver** per variant, **plus optional auxiliaries** (monitor, hooks). | The user initially revised "exactly one" toward "a subset"; reconciled as transport (one) + auxiliaries (many). |
| Scripting | **No arbitrary scripting in v1.** A fixed small event set may bind to **named local hooks**; presets carry none; shared profiles name hooks, never paths. | Parked branch. |
| Rotation in v1 | **Parked**, `rotation: "none"`. | The user asked it be implementable in different ways per profile later — confirmed as referenced strategies. |
| Preset/custom layering | **Custom shadows preset, preset is the fallback.** | |

### Facts established during the session

- Parsec macOS hosting exists (10.15+, Metal, 2019+ hardware) but needs unscriptable permission
  grants and reportedly degrades above 1080p on Mac hosts.
- The current `lobby/results.rs` reads RAM for **`sfiii3nr1` only** (`addresses_for("sf2ce") == None`).
- **Tailscale is unicast L3** — UDP broadcast/multicast LAN discovery does not traverse it, so games
  that auto-find LAN peers by broadcast are unsupported without extra tooling.
- "Emulators package their own online" (RetroArch netplay, Dolphin netplay, PPSSPP adhoc, melonDS
  local wireless) is the **same architecture** as netplay, just a different binding.

## 3. The design

### 3.1 Scope

- **Target:** arbitrary games, across macOS / Linux / Windows, for a small fixed friend group.
- **Each game declares** its session architecture, its per-OS runner, its content selector, and its
  optional per-game logic — as **data**.
- **The app core** is generic: peers, sessions, health, UI, config, logging, updates, diagnostics.
- **FightCade is gone** as a provider and a dependency. See `12-fightcade-ripout.md`.

### 3.2 The three transport drivers (closed set)

Every profile variant declares **exactly one** transport driver.

| Driver | Who runs the game | How peers sync | Requires | Bandwidth | App's job |
|---|---|---|---|---|---|
| `native_online` | Everyone | The game's own netcode (or a LAN/direct-IP mode) | The game's online works when run; peers can be routed together | game-dependent | network plumbing + launch |
| `netplay` | Everyone | Deterministic input relay (rollback) via a specific emulator | **Identical emulator + content**; deterministic core | ~1–2 KB/s per player | emulator + content orchestration |
| `stream` | Host only | Host renders; guests receive video + send input | Host runs game + streamer; upload bandwidth | ~5–50+ Mbps | host orchestration |

**Per-game decision rule** (encoded in the profile, not inferred by the app):

1. Game's own online (or LAN/direct-IP) works → `native_online`
2. An emulator offers deterministic netplay → `netplay`
3. Otherwise → `stream`

`netplay` is a **family**: the profile binds which emulator's netplay to invoke (RetroArch today;
Dolphin/PPSSPP/melonDS may be added as *bindings*, not new drivers).

`native_online` v1 covers only games that **accept a peer IP**. The profile supplies the tailnet IP;
the game connects to it. Not supported in v1 (parked, per-game research): games that broadcast-
discover LAN peers, and games reachable only through a central service (the
[`05-ggst-tailnet-enforcement.md`](05-ggst-tailnet-enforcement.md) territory).

### 3.3 Auxiliary concerns (optional, driver-independent)

Any variant may additionally enable:

- **monitor** — watch a network interface / session health (the existing netplay log observer and
  `tailscale ping` health thread are the current examples).
- **hooks** — bind a **fixed, small event set** to **named local hook scripts** (see §3.5). Events:
  session start, session stop, health crossed threshold, process exit. Hooks are referenced by
  *name*, resolved from the user's local hooks directory; never by path; never embedded in a
  profile; **bundled presets carry none**.

A "monitor-only" game is therefore `native_online` + `launch: none` + `monitor`/`hooks` — **not** a
fourth driver.

### 3.4 The profile

Profiles are **declarative JSON**, validated against a versioned schema. They live in two tiers
(§3.6). Illustrative shape:

```jsonc
{
  "schema": 1,
  "gameId": "sfiii3nr1",
  "title": "Street Fighter III: 3rd Strike",
  "variants": [
    {
      "id": "arcade-fbneo",
      "label": "Arcade (FBNeo, netplay)",
      "driver": "netplay",
      "managed": true,
      "runner": {
        "macos":   { "kind": "retroarch", "core": "fbneo_libretro.dylib", "args": [] },
        "linux":   { "kind": "retroarch", "core": "fbneo_libretro.so",    "args": [] },
        "windows": { "kind": "retroarch", "core": "fbneo_libretro.dll",   "args": [] }
      },
      "content":   { "match": "*.zip", "parity": "required" },
      "players":   { "seats": 2, "rotation": "none" },
      "detector":  { "id": "fbneo-ram-ko", "params": { /* per-ROM addresses */ } },
      "hooks":     {}
    }
  ]
}
```

A streaming variant of a different game, with a different driver and per-OS runner, is the same
schema:

```jsonc
{
  "schema": 1,
  "gameId": "naruto-uns4",
  "title": "Naruto Shippuden: Ultimate Ninja Storm 4",
  "variants": [
    {
      "id": "pc-crossproton",
      "label": "PC (CrossOver/Proton, stream)",
      "driver": "stream",
      "managed": true,
      "streamer": "sunshine",
      "runner": {
        "macos":   { "kind": "crossover", "args": ["..."] },
        "linux":   { "kind": "proton",    "args": ["..."] },
        "windows": { "kind": "native",    "args": ["..."] }
      },
      "content":   { "parity": "none" },
      "players":   { "seats": 4 }
    }
  ]
}
```

Key rules:

- **The driver owns the ordered steps.** The profile supplies *parameters*, not a bespoke step order.
  `netplay` always resolves content → parity-checks → launches host/join → observes. `native_online`
  always resolves the peer IP → launches → monitors. `stream` always launches the game + streamer →
  pairs → hands over control. Variation is parameters, not sequencing.
- **`assist`** marks a step the OS forces a human to complete (e.g. macOS permission prompts for a
  streamer). Where a step can be automated, it is.
- **`managed`** distinguishes "the app launches and connects" from "the app prepares/deep-links and
  the user completes it" (the Parsec-unmanaged case).
- **Content selector**: how to find the game's content, and whether **parity is required**
  (`required` for netplay — identical bytes; `none` for stream/native_online).
- **Per-game logic** is only referenced by **id + params** to built-in detectors/strategies
  (`detector.id`, `players.rotation`). `fbneo-ram-ko` reads the RetroArch command socket, so it is
  meaningful **only under `netplay`**; under `stream`/`native_online` it must be absent.
- **`rotation: "none"`** is the only value in v1. Winner-stays/loser-out (the current `sets.rs`
  behaviour) is a *parked* strategy to be re-introduced later, selectable per profile/game — the
  user expects multiple rotation strategies over time.

### 3.5 Hooks (v1 boundary)

- A **fixed event set** only: session start, session stop, health crossed threshold, process exit.
- A hook is a **name**; the app resolves it inside the local hooks directory. Profiles never carry a
  path or a script body.
- **Bundled presets carry no hooks.** A shared custom profile can at most name a hook the recipient
  already has locally.
- Arbitrary event→script automation is a **parked branch**, not v1.

### 3.6 Profile tiers and trust

| Tier | Location | Writable | Hooks | Precedence |
|---|---|---|---|---|
| Bundled presets | shipped with the app | no | never | fallback |
| Custom | user profile directory | yes | names only | **shadows** a preset with the same `gameId`+variant |

- Both tiers validate against the same `schema` version.
- An **invalid custom profile fails loudly**; it never silently falls back to a preset, so a typo
  cannot launch the wrong thing.
- No executable content is ever loaded from a profile.
- v1 authoring is **editing JSON on disk**; profiles are shared **by hand**. In-app editing and
  sharing are parked branches.

### 3.7 Reachability layer

The reachability/peer layer is **transport-agnostic**. Tailscale is one concrete provider of
"a stable peer id + a reachable address + RTT/path health"; a `stream` variant that uses Parsec may
have no tailnet at all, in which case reachability collapses to "the host tells everyone the peer
id." The core must not assume Tailscale, FBNeo, netplay, two seats, or a winner.

### 3.8 Invariant core vs per-game variable (the "constants")

**Core (invariant across all games):**

1. Peer identity + reachability/health.
2. Session intent + room — who hosts, which game, who is in, join/invite, one session per machine.
3. **Session-plan resolution** — profile + roles + local machine → concrete actions. This is the
   mode/driver engine and the heart of the design.
4. Lifecycle + health where the app owns the process.
5. Config, logging, diagnostics, updater, UI shell.

**Per-game profile (variable):**

1. Runner recipe **per OS** (emulator + compatibility layer + args/cwd/env).
2. Content identity + location + parity requirement.
3. Transport driver + managed/unmanaged + ports.
4. Player count / seats / control mapping.
5. Result/scoring detection.
6. Rotation/set rules.

## 4. Worked examples

| Game | Driver | Why | App's job |
|---|---|---|---|
| SF3 arcade (FBNeo) | `netplay` | Deterministic FBNeo netplay exists | resolve ROM → parity gate → launch host/join → (later) KO detector |
| Ultimate Ninja Storm 4 | `stream` | No deterministic netplay; same Windows build under CrossOver/Proton | launch game + Sunshine, pair Moonlight guests, hand over control |
| A game with good LAN/direct-IP online | `native_online` | Its own netcode is fine | supply the tailnet IP, launch, monitor health |
| A game that only needs traffic watched + hooks | `native_online`, `launch: none` | Nothing to launch | monitor the interface, fire named hooks |

## 5. Risks and tradeoffs (accepted)

- **Streaming** makes the host a bandwidth/GPU single point of failure and adds encode + round-trip
  latency. Accepted because some games have no other shared-session form.
- **Parsec-on-macOS cannot be automated**; that profile is coordination-only, and the app's value
  there shrinks toward "a Discord post with a link." Sunshine+Moonlight is the mitigation.
- **Tailscale does not carry broadcast/multicast**, so discovery-based `native_online` games are
  unsupported in v1.
- **RAM result detection is bound to netplay**; it is meaningless under `stream`/`native_online`.
- **"Any game" is aspirational**: each new game can be its own research item.
- **The current code is netplay- and SF3-shaped**; the modular core will be partly a rewrite, not a
  refactor. The FightCade rip-out is deliberately the *first*, low-risk step.

## 6. Branches deliberately left open

1. Scripting / arbitrary event→script automation (parked).
2. `native_online` broadcast discovery and central-service interception (per-game research).
3. Rotation port and multiple rotation strategies (parked).
4. In-app profile authoring / profile sharing (v1 = JSON on disk, shared by hand).
5. **Seat/control mapping — per-game or per-runner?** (raised, never answered; will shape the profile
   schema).
6. Content identity/parity for non-RetroArch emulators, and Sunshine detection/configuration per OS.
7. The implementation plan beyond the FightCade rip-out (drivers, profiles, core rewrite).

## 7. Relationship to the existing code and docs

**Survives and is generalizable (keep):**
- Tailscale reachability/health (`tailscale/`, `nethealth`), the peer registry, the UI shell,
  config/logging/diagnostics/updater.
- The RetroArch provider plumbing (`providers/retroarch/{core,spec,command,parity,hotkeys}`), the
  frozen-core download, the SOCD preset, the command socket, the netplay log observer.
- The lobby's *shell* (rooms, beacon, join/host, seats) as the basis for session intent.

**Becomes per-game data (extract from code):**
- SF3 result detection (`lobby/results.rs` `sfiii3nr1` addresses) and set/rotation (`lobby/sets.rs`).

**Goes (FightCade):**
- `providers/fightcade.rs`, the `launcher/` tree, `Config.fightcade_dir` + `Config.provider`, the
  FightCade ROM defaults, and the FightCade reference scripts. Tracked in
  [`12-fightcade-ripout.md`](12-fightcade-ripout.md).

**Docs touched:** `04-design.md` (scope superseded), `README.md`, `AGENTS.md`, `docs/README.md`, and
the FightCade reference scripts.
