# Docs — The Cabinet

The Cabinet is being re-scoped from a FightCade FBNeo launcher into a general multi-game session
coordinator. `04-design.md` is the spec for the FightCade-launcher era (its *scope* is superseded);
[`design-general-emulation.md`](design-general-emulation.md) is the agreed re-scope, and
[`12-fightcade-ripout.md`](12-fightcade-ripout.md) is the first implementation step. `01`–`03` are
the original investigation into
**"Test game, tutorial, and spectating work, but challenging (or being challenged by) another player leaves me stuck in the chat interface."**

> Working assumption: "Fight Kid" = **FightCade 2** (v2.1.45) on macOS, installed at `/Applications/FightCade2.app`.

## TL;DR

- **Root cause: the ISP is running Carrier-Grade NAT (CGNAT) with symmetric NAT.** MTN (home/fixed broadband, ZTE router) does not give this connection a reachable public IPv4 endpoint.
- FightCade's peer-to-peer matchmaking (GGPO / "Quark" UDP hole punching) cannot establish a path to the opponent, so it falls back to fixed ports that cannot be reached. The match never starts, leaving the user in the chat UI.
- Server-based modes (test game, tutorial, spectating) do not need peer-to-peer, which is why they work.
- **Not** the macOS firewall, and **not** a FightCade bug.
- The connection already has **Tailscale**, but FightCade's matchmaking does not use tailnet addresses, so it doesn't help by itself. Both peers are behind symmetric NAT, so a direct path was not initially available and traffic relayed via DERP (`par`, 329–514 ms RTT). **Update (2026-09-18):** after a router restart the tailnet established a **direct path, <100 ms**; the DERP-relay measurements are stale. Keep the NAT analysis for FightCade's own matchmaking, which still cannot traverse CGNAT.
- **Chosen fix: direct-connect FBNeo over Tailscale** using the emulator's built-in `quark:direct` mode (bypasses FightCade servers entirely). Free, no ISP request, no VPN subscription. Verified working over the now-direct <100 ms path; GGPO rollback absorbs the rest.
- **The fix is now an app.** [`04-design.md`](04-design.md) specifies **The Cabinet**, a Tauri v2 desktop launcher that wraps `quark:direct`. Phase 1 (launchers, connection health), the RetroArch spectator provider, Cabinet mode, and distribution (GitHub Actions → GitHub Releases, plus the Windows launcher adapter; the repo is public after a 2026-09-19 history scrub) are implemented; the rooms/KotH subsystem and score tracking were removed on 2026-09-21. See the root [`README.md`](../README.md) to run it.

## Documents

| File | Contents |
|---|---|
| [`01-diagnosis.md`](01-diagnosis.md) | Evidence, logs, and the confirmed root cause |
| [`02-options.md`](02-options.md) | All remediation routes with pros/cons/cost |
| [`03-implementation-plan.md`](03-implementation-plan.md) | Step-by-step plan for the chosen (free, Tailscale) route |
| [`04-design.md`](04-design.md) | Current spec: The Cabinet Tauri app, network health, spectating (rooms/KotH and score tracking removed 2026-09-21) |
| [`05-ggst-tailnet-enforcement.md`](05-ggst-tailnet-enforcement.md) | Separate companion-tool research: keeping Guilty Gear Strive on the tailnet — ToS boundary, firewall enforcement feasibility, phased plan |
| [`06-redesign.md`](06-redesign.md) | Proposal (gated on a /grill-me session): Cabinet mode (the emulator hosted inside the app) and the wider redesign pass. Also answers "does FightCade need to be installed?" |
| [`07-retroarch-spike.md`](07-retroarch-spike.md) | Phase 0 RetroArch netplay/spectator spike: frozen core/ROM parity, harness, local loopback verification (2 players + 2 spectators), and the live feel-test protocol |
| [`08-cleanup.md`](08-cleanup.md) | Cleanup backlog: the applied refactor pass and the deferred proposals |
| [`09-lobby.md`](09-lobby.md) | Design for the replacement lobby (rooms, sets, winner-stays rotation, automatic RAM result detection, local scores/history) — agreed in a `/grill-me` session 2026-09-21, not implemented |
| [`10-lobby-spike.md`](10-lobby-spike.md) | Hardware spike that gates the lobby: host-as-spectator, live role switching, RAM result detection, tailnet stability, beacon reachability |
| [`11-training-mode.md`](11-training-mode.md) | Research (no code): two training paths — Fightcade's bundled peon2 script / `3rd_training_lua` and the FBNeo `--lua` hook (Path A, licensing-gated), and a Cabinet-driven RetroArch FBNeo practice mode using `WRITE_CORE_RAM`/replay over the command socket (Path B, added 2026-09-22). Includes the `sfiii3nr1` address mapping and the gating spikes |
| [`12-fightcade-ripout.md`](12-fightcade-ripout.md) | **Plan (not executed):** the first step of the re-scope — full FightCade rip-out (provider, `launcher/` tree, config, ROM defaults, UI, scripts, docs) with a file-by-file inventory and phased checklist |
| [`design-general-emulation.md`](design-general-emulation.md) | **Agreed re-scope (2026-09-25 `/grill-me`):** The Cabinet as a general multi-game session coordinator — three transport drivers (`native_online`/`netplay`/`stream`), declarative per-game profiles, Sunshine+Moonlight streaming, invariant core vs per-game variables, and the open branches |

## Environment (as observed)

| Item | Value |
|---|---|
| Machine | Apple M1 (arm64), macOS 26.6.2 (build 25G83) |
| FightCade | 2.1.45, `/Applications/FightCade2.app` |
| Emulator | FBNeo (`fcadefbneo.exe`, PE32) under bundled Wine `wine32on64` (Rosetta 2) |
| Networking helper | `Contents/MacOS/emulator/fcade` (PyInstaller) |
| Router | ZTE, admin at `http://192.0.2.1` |
| LAN IP | `192.0.2.100` |
| Public IP (observed) | `203.0.113.10` |
| Tailscale (this Mac) | `100.64.0.1` (`mac-host`, `me@`); symmetric NAT; nearest DERP `jnb` 122.7 ms |
| Tailscale (friend) | `100.64.0.2` (`cachyos-host`, `friend@`); public `203.0.113.11`, LAN `192.0.2.101`; **also symmetric NAT**; nearest DERP `lhr` 114 ms |
| Tailscale (Windows friend) | `100.64.0.3`; native FightCade at `%APPDATA%\Fightcade` |
| Tailscale path (measured) | **Direct, <100 ms** after a 2026-09-18 router restart; earlier DERP `par` 329–514 ms relay is stale |
| Launchers | `scripts/fcade-lan-macos.sh` (installed to `~/bin/fcade-lan`), `scripts/fcade-lan-linux.sh` (cachyos friend; Flatpak-aware), `scripts/fcade-lan-windows.bat` + `scripts/fcade-lan-windows-firewall.bat` (Windows friend) |
| The Cabinet app | Tauri v2 launcher (launchers, connection health, RetroArch spectator provider, Cabinet mode). Run with `scripts/dev.sh`. See root [`README.md`](../README.md). |
| Friend install (cachyos) | FightCade Flatpak `com.fightcade.Fightcade` on CachyOS; uses the Flatpak's bundled Wine |
| Friend install (Windows) | FightCade native at `%APPDATA%\Fightcade`; tailnet `100.64.0.3`; no Wine needed |
| ROM present | `sfiii3nr1.zip` (Street Fighter III: 3rd Strike) |
