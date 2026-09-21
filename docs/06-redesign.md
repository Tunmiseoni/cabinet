# Redesign pass: Cabinet mode + a streamlined single-system feel

Status: **R0 (window-topology spike) partially done; R2 (Cabinet mode, macOS-first) implemented;
R1 (the redesign grill) not yet held.** The feature is off by default (`cabinetMode` in Settings) and
does not block Phases 1–4. This document captures (a) the two questions raised after the `v0.1.3`
release — whether the emulator can open **inside** The Cabinet, and whether FightCade needs to stay
installed at all — and (b) the wider UX redesign this repo needs. The remaining feature work
(R3/R4) is **gated on a `/grill-me` session** (see §6), in keeping with the repo's convention for
gated proposals in [`04-design.md`](04-design.md) §3/§6. Note: the rooms/lobbies/KotH subsystem and
score tracking were **removed 2026-09-21** ([`04-design.md`](04-design.md) §5), so the redesign no
longer has a lobby/rooms/ladder to fold in.

Companion to [`04-design.md`](04-design.md), which remains the spec for everything already built.
This file is the spec for the redesign and Cabinet mode until it is folded back in.

## 1. Motivation

Today The Cabinet is a coordination layer that *spawns a separate emulator window*. The app and
the game sit side by side: the user launches a match, then alt-tabs to a Wine window to play, and
the match HUD lives in a different window. It works, but it does not feel like "a system" —
it feels like a launcher next to an emulator.

Two asks:

1. **Open the emulator within The Cabinet**, so a match is one window: a cabinet bezel/HUD framing
   the game, with connection health around it.
2. **Does FightCade even need to be installed?** (Answer in §2: yes, as the provider — the client
   is never launched.)

The redesign pass is deliberately broader than Cabinet mode. Cabinet mode is the forcing function
for a single-window information architecture; the grill (§6) decides how far the redesign goes.

## 2. Does FightCade need to be installed? Yes — as the provider, never the client

The Cabinet **already** bypasses the FightCade client: it spawns the emulator directly with
`quark:direct`, and never opens the Electron lobby or the `fcade` helper. But the emulator binary,
Wine, the ROM directory, the FBNeo config, and the overlay output all still come **from the
FightCade installation**:

| What the launch needs | Provided by | Reference |
|---|---|---|
| `fcadefbneo.exe` — the only build with `quark:direct` (upstream FBNeo lacks it) | FightCade install | `launcher/macos.rs`, `launcher/linux.rs`, `launcher/windows.rs` |
| `wine32on64` + `.wine32` prefix (macOS) | `FightCade2.app/Contents/Resources/` | `launcher/macos.rs:26-33` |
| Flatpak sandbox / Wine entry (Linux) | `com.fightcade.Fightcade` | `launcher/linux.rs:6-8` |
| ROMs, `fcadefbneo.ini` | FightCade data dir | `roms.rs` |

**Consequences:**

- Uninstalling FightCade breaks every launcher. The dependency is real, not incidental.
- What is optional: the FightCade **client** (`fcade`, the Electron UI) and matchmaking. The user
  never sees them, and none of the direct-connect flow needs them.
- Cabinet cannot legally ship the provider itself: FBNeo is open (GPL), but the FightCade fork and
  `ggponet.dll` are not public. Bundling ROMs is also out.
- The only route to dropping the FightCade install is the **RetroArch FBNeo core** — open, native,
  standalone. That swaps `quark:direct`/GGPO for RetroArch netplay (TCP/replay), i.e. it revisits
  the core network decision in `04-design.md` §2/§3, not just packaging. It stays a Phase 0/3
  question.

**Doc action:** state this plainly in `04-design.md` and the README so the dependency is not a
surprise. ("You need FightCade installed, but you never launch FightCade.")

## 3. Cabinet mode: the emulator inside the app

### 3.1 Constraint

The emulator is a **separate native process** (a 32-bit PE under Wine on macOS; native on
Windows). Tauri's window is an OS webview. There is no public API to render a foreign process's
window *inside* a webview, so "inside the app" is achieved by the app **hosting the foreign
window** in a measured region and drawing the cabinet around it. Two topologies:

| Topology | Mechanism | Fit |
|---|---|---|
| **Placement** | Move/resize the emulator window into a viewport rect measured by the frontend, using the OS's cross-process window API. | Full control of size/position; needs a permission on macOS. |
| **Frame-follow** | Read the emulator window's bounds (no permission) and position/resize the Cabinet window to frame it. | No permission; no control over the game's size. |

The plan is to try placement, and fall back to frame-follow when placement isn't permitted or
supported. Cabinet mode always degrades to "separate window + clear status" rather than failing a
launch.

### 3.2 Per-OS capability matrix

| OS | Placement API | Permission | Notes |
|---|---|---|---|
| macOS | Accessibility API (`AXUIElement` position/size) | **Accessibility** (one-time, user-granted in System Settings) | Grants are keyed to the code signature; the app is **ad-hoc signed**, so a `cdhash`-based grant may not survive an update. Must be tested. Frame-follow is the no-permission fallback. |
| Windows | `SetParent` / `SetWindowPos` | None | True child-window embedding is possible. Build first if parity matters. |
| Linux/X11 | `XReparentWindow` | None | Reparenting works on X11. |
| Linux/Wayland | — | — | Wayland forbids cross-process reparenting; fall back to frame-follow or a separate window. The app already disables the WebKitGTK DMA-BUF renderer (`lib.rs`), so treat WebKitGTK transparency as unreliable. |

**macOS-first** is the priority (it is the primary machine). Windows and Linux get the same
`WindowHost` trait with their own implementations, compile-checked now and verified by their
owners.

### 3.3 Architecture (proposed)

A new `src-tauri/src/windowing/` module mirrors the existing `launcher/` abstraction:

```
windowing/
├─ mod.rs      -> Rect types, WindowHost trait, rect math + unit tests
├─ macos.rs    -> find Wine window (CGWindowList), AX placement + permission, frame-follow fallback
├─ windows.rs  -> SetParent / SetWindowPos
└─ linux.rs    -> X11 reparent; Wayland separate-window fallback
```

Commands (async, per the current convention):

```
cabinet_status()            -> { supported, permission, emulatorWindow, mode }
cabinet_place(viewportRect) -> Result<(), String>   // placement or frame-follow
cabinet_release()           -> Result<(), String>   // restore the emulator to a normal window
```

The frontend renders a `MatchView` (full-screen bezel + HUD around a viewport), measures the
viewport with a `ResizeObserver`, and calls `cabinet_place` on change. `session.rs` attaches on
spawn and releases on stop/exit; `MatchState.status` drives enter/leave of Cabinet mode. The
HUD must sit in the **bezel**, not over the game, since the emulator window will cover the
viewport.

### 3.4 macOS permission reality (to verify in the spike)

- Placement needs the Accessibility grant. The app will detect `AXIsProcessTrusted`, prompt once,
  and deep-link to System Settings.
- The grant is identified by the app's code requirement. With ad-hoc signing, the requirement is
  `cdhash`-based and **changes on every build**, so the user may have to re-grant after an update
  (or after a fresh download/reinstall). The spike must measure this; if it is too painful,
  frame-follow becomes the default on macOS and placement is opt-in.
- Revoked/never-granted permission must produce a clear status, never a failed launch.

### 3.5 Spike log

**R0 window-topology spike (macOS, 2026-09-19).** The `windowing` skeleton is implemented
(`src-tauri/src/windowing/`, `WindowHost` trait mirroring `launcher/`), with the debug commands
(`cabinet_status`, `cabinet_place`, `cabinet_release`, `cabinet_request_permission`) and a
throwaway `CabinetCard` (since removed) that measures a viewport and places a chosen window into it.

Findings so far:

- **Window enumeration needs no permission.** `CGWindowListCopyWindowInfo` returns owner PID,
  owner name, and bounds with no Accessibility or Screen Recording grant (confirmed on this
  machine). This is the basis of the frame-follow fallback.
- **Window titles are redacted without Screen Recording permission.** `kCGWindowName` is absent for
  every on-screen window in the spike; the parser must match the Accessibility window by owner PID
  and geometry, **not** by title. The implementation matches on PID + position/size proximity and
  treats a title hit as a bonus only.
- **The Wine window is owned by the spawned `wine32on64-preloader` PID directly** (confirmed: the
  child PID we spawn equals the window owner), so PID matching is reliable and descendant expansion
  is only a safety net. The emulator window takes **~8–15 s** to appear after spawn; any attach
  logic must poll rather than sample once.
- **Accessibility is already granted to a terminal-spawned test process** (`AXIsProcessTrusted` =
  true when run from this shell), so placement can be verified in development without a fresh
  grant. A distributed, ad-hoc-signed `.app` is a separate TCC subject and will need its own
  one-time grant; grant persistence across reinstall is still to be measured.
- **Placement works end-to-end.** `live_places_emulator_window` launched FBNeo, found the window
  (`272,101 896×719`), set it to `120,120 720×480` via AX, and read back exactly `120,120 720×480`.
  The Accessibility API can both move and resize the emulator window.
- **Pending:** grant persistence after a rebuild vs. a re-downloaded `.app`; placement vs.
  frame-follow as the macOS default; z-order/focus with the Cabinet window over the game; and
  multi-instance (Dev pair) disambiguation.


## 4. Scope of the redesign (beyond embedding)

The single-window IA makes the current card grid (`LaunchCard`, `PeersCard`, `RomsCard`)
insufficient. The grill should decide the target information architecture. Candidate directions:

- **Two surfaces:** a **Launcher** (peers + ROMs + launch controls — the current cards,
  reorganized) and a **Match** (the cabinet bezel: game viewport, live health/RTT, and
  stop/after-match actions).
- **Bezel as the identity:** the cabinet frame carries marquee (game/ROM, players), a control
  panel (health, ping), and side rails — using the vertical and horizontal space a full-screen
  match window gives us.
- **Transitions:** launching enters Match automatically; exit/stop returns to the Launcher.
- **Overlay discipline:** nothing is drawn over the game; the emulator may need to be hidden while
  dialogs are open and restored afterwards (or dialogs rendered in the bezel).
- **Consistency:** one window, one navigation model, one visual language; remove the "launcher
  next to emulator" feel.

Deliberately **out of scope** for this pass (still gated, per `04-design.md` §3/§6): emotes/reactions,
voice, input recording / match history, and the RetroArch migration.

## 5. Risks and open questions

- **macOS Accessibility grant persistence under ad-hoc signing** — the largest risk; a spike
  measurement, not an assumption. Frame-follow is the fallback.
- **Z-order / focus / click-through** on macOS: the emulator window must stay above the bezel in
  its region while the Cabinet remains usable around it. Placement+raise, Window level for the
  Cabinet window, and hiding the emulator for dialogs all need prototyping. This is the spike's
  core.
- **Resize/jitter:** the user or Wine can move/resize the emulator after placement; a re-assert
  loop or `AXObserver` is needed (poll cadence is a cost).
- **Finding the right window:** the spawned `wine32on64` PID is not necessarily the window owner
  (the PE process is a descendant). Matching by PID tree, owner name, or title (ROM name) must be
  proven.
- **Wayland:** no reparenting; frame-follow or separate window only.
- **Multi-instance (Dev pair):** two emulator windows on one machine must not be confused.
- **FightCade updates** can change window titles/behaviour; keep matching defensive.
- **Does the redesign fit "coordination layer, not a FightCade clone"?** The bezel must not start
  emulating FightCade's client chrome; it frames a direct-connect match, nothing more.

## 6. Grill agenda (required before build)

Per `04-design.md` §3, gated work needs a `/grill-me` session. Proposed agenda:

1. **Cabinet mode:** placement vs. frame-follow as default on macOS; is the Accessibility prompt
   acceptable to the group; what happens on Wayland and on the Windows friend's machine.
2. **Information architecture:** two surfaces (Launcher/Match) vs. one; what lives in the bezel vs.
   the side rails; how the post-match result/next-match flow works.
3. **FightCade dependency:** confirm the "installed but never launched" framing; is the RetroArch
   migration worth revisiting to remove the dependency (ties to `04-design.md` §2/§3)?
4. **Failure UX:** no permission, no window found, emulator crashes, peer disconnects mid-match.
5. **Scope:** what of the redesign lands now vs. after Phase 3/4; does the redesign delay the
   4-person live test?
6. **The existing deferred gates** (emotes/voice/recording): fold into the same session or leave
   separate.

## 7. Phased plan (proposed)

| Step | Work | Gate |
|---|---|---|
| R0 | **Window-topology spike (macOS)** — `windowing` skeleton + debug commands; place a Dev-pair emulator into a measured rect; measure AX permission persistence across rebuild/reinstall; confirm z-order/focus; validate frame-follow | **Partially done 2026-09-19** — window enumeration + AX placement verified end-to-end (§3.5); grant persistence, z-order, and frame-follow still open |
| R1 | Design/grill session: IA + Cabinet mode decisions | Agenda §6 resolved — **pending** |
| R2 | Cabinet mode implementation (macOS-first): config toggle, placement ladder, session attach/release, `MatchView` | **Implemented 2026-09-19, off by default** — Settings → Cabinet mode; needs a live match run before "done" |
| R3 | Redesign the Launcher/Match IA per R1 | Group agrees it is streamlined — pending R1 |
| R4 | Windows/Linux `WindowHost` implementations + verification by their owners | Cross-platform parity or documented fallback — pending |

**Implementation notes (R2).** `src-tauri/src/windowing/` defines the `WindowHost` trait; macOS
uses the Accessibility API for placement and lists windows via `CGWindowList` (no permission);
Windows/Linux are stubs that report unsupported and fall back to a separate window. Commands:
`cabinet_status`, `cabinet_place`, `cabinet_release`, `cabinet_request_permission`. Config gains
`cabinet_mode`. The frontend `MatchView` (bezel + measured viewport, `ResizeObserver` re-assert
every 2 s) auto-enters when a match is running and Cabinet mode is on, and releases the window on
exit. The shell is **pseudo-maximized** to the current monitor (`setDecorations(false)` +
`setPosition`/`setSize`, restored on exit) rather than using `setSimpleFullscreen`, which keeps the
app on the same Space and avoids macOS fullscreen quirks.

**Bug fixed during the first live test (2026-09-19).** `list_windows` originally fell back to
*all* on-screen windows when no descendant of the session PIDs matched — which is exactly the state
before the emulator window appears (~8–15 s). `MatchView` then adopted that unrelated window (often
The Cabinet's own), pseudo-fullscreened it and resized it to the viewport, so the app window
vanished and the emulator later opened with nothing hosting it. Fixes: `list_windows` never falls
back (empty when no PIDs / no match); `cabinet_place` re-validates the requested window id against
the session's windows; `MatchView` distinguishes checking / unsupported / error / timed-out and
only adopts a window from the session list. `cabinet_status` now logs
`platform/permission/pids/window count` to the dev terminal. **When testing, fully restart
`scripts/dev.sh`** — an already-running dev process can be serving an older Rust backend, which
surfaces as the "unsupported" message after a Retry.

**Not yet implemented (next steps).**

- **Frame-follow.** When Accessibility is denied, the app reports the fallback mode and keeps the
  game in its own window; it does **not** yet resize the Cabinet window around the emulator.
  Deciding the exact frame-follow layout is part of the grill (§6).
- **Z-order/focus.** The emulator is raised once on first placement; if the user brings the Cabinet
  forward, the pseudo-maximized window can cover the game. A focus-aware re-raise (or a
  below-normal window level for the Cabinet on macOS) is still to be designed.
- **Grant persistence** across an ad-hoc-signed rebuild/reinstall (the spike only confirmed the
  grant works in a dev shell).
- **Multi-instance disambiguation** (Dev pair shows one window).
- **Diagnostics:** the `eprintln!` status log and the footer status line are intentionally
  temporary while Cabinet mode stabilises; trim once the live run is clean.
- **Windows/Linux hosts** (R4).

## 8. References

- `04-design.md` §2/§3 — RetroArch vs. FightCade decision, gated proposals.
- `04-design.md` §4 — architecture the Match surface reflects.
- [`launcher/mod.rs`](../src-tauri/src/launcher/mod.rs) — the abstraction `windowing/` mirrors.
- [`session.rs`](../src-tauri/src/session.rs) — process lifecycle Cabinet mode hooks into.
- `scripts/fcade-lan-macos.sh` — reference launch (Wine + `quark:direct`).
