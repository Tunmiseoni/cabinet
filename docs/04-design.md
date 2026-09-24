# Design: The Cabinet — Tailnet FightCade launcher (spectating)

Status: Phase 1 (macOS + Linux + Windows launchers, connection health), the RetroArch spectator provider, Cabinet mode, and Delivery (GitHub Actions per-OS matrix → GitHub Releases) are implemented as a Tauri v2 app. The rooms/KotH/lobby subsystem, the lifetime score ledger, and the emulator result watcher were **removed 2026-09-21** (see §3 and §5). Spectating (Phase 3) and packaging (Phase 4) remain. Phase 0 (RetroArch spectator spike) is locally verified over loopback (see [`07-retroarch-spike.md`](07-retroarch-spike.md)), pending the two-concurrent-spectator tailnet run. RetroArch netplay settings (netplay max-ping, spectator mute) and the frozen-core download path landed 2026-09-21; a loopback retest confirmed opt-in config isolation keeps keyboard input (default stays off), and the `retroarch-cores-v1` core release is published (see §3). See the progress note in §6. A **UX redesign + Cabinet mode** (the emulator hosted inside the app) is proposed in [`06-redesign.md`](06-redesign.md), gated on a `/grill-me` session (§3/§6). A replacement **lobby** for the removed Phase 2 (rooms, sets, automatic rotation) was designed in a `/grill-me` session on 2026-09-21 and is in [`09-lobby.md`](09-lobby.md), gated on the hardware spike in [`10-lobby-spike.md`](10-lobby-spike.md): the **local build gate S1–S5 passed 2026-09-21** (host-as-spectator, live role switching, RAM round-result detection), while the **online release gate S6–S7** (tailnet soak, beacon) is pending. The lobby is built through sets/rotation (room model, HTTP beacon, hosting, the seat/role split, the client join flow, `LobbyCard.tsx` — now the preferred home screen — the first-to-N set machine and advertised FIFO rotation, the single **Join**, and `RoomView.tsx`); the local score/history ledger and game-boundary `RESET` remain.
Project name: **The Cabinet**. Git repository: **public** (`Tunmiseoni/the-cabinet`) — history scrubbed on 2026-09-19 (see §8), so release assets download anonymously and the private-repo Actions minute cap no longer applies.
Supersedes the "chosen route" framing in [`03-implementation-plan.md`](03-implementation-plan.md); the troubleshooting record (`01`–`03`) stays as history.

## TL;DR

- We already solved the hard network problem: `quark:direct` connects FBNeo peer-to-peer over Tailscale, bypassing FightCade matchmaking and the ISP CGNAT. Four friends have been playing this way all afternoon with a **direct, <100 ms** tailnet path.
- The proposed app is a **coordination layer**, not a FightCade clone. FightCade's server, `fcade` helper, and spectator relay are all closed; there is nothing to fork.
- The one genuinely hard feature is **input-relay spectating** (watch without video bandwidth). FightCade's spectator is server-brokered over a closed TCP protocol and is unreachable from `quark:direct`. **RetroArch netplay** provides the same low-bandwidth, input-based spectating natively, cross-platform, with no Wine or patched binaries.
- Decision: run a **one-day RetroArch spectator spike first**. If its netplay feels acceptable at <100 ms, make RetroArch the primary path and skip the risky `ggponet.dll` reverse-engineering entirely. Otherwise fall back to a time-boxed FightCade shim, then to "defer spectate".
- Features in scope: cross-platform Tauri launcher, **connection-health display (per-peer ping, direct-vs-relay)**, and **input-relay spectating**. (The former king-of-the-hill room and per-player score tracking were removed 2026-09-21.)
- Stack: **Tauri v2 (Rust backend) + Vite web frontend** (see §3 and §6).

## 1. How FightCade works (established from the installed 2.1.45 client)

FightCade 2 is three layers:

| Layer | What it is | Open? |
|---|---|---|
| Electron shell | Nativefier wrapper around `web.fightcade.com` — chat/lobby UI | Closed (the server is the app) |
| `fcade` helper | PyInstaller binary: login, status, matchmaking, launches emulator | Closed |
| Emulator | `fcadefbneo.exe` (PE under bundled Wine on macOS) + `ggponet.dll` (GGPO rollback) | FBNeo/GGPO upstream open; FightCade fork/args closed |

Match flow: FightCade's **Quark** server introduces two players, they UDP hole-punch, then GGPO runs a direct session. CGNAT + symmetric NAT breaks the hole-punch, so the app hangs in chat. `quark:direct` skips Quark and hands GGPO explicit endpoints — the basis of the working scripts.

**The Cabinet never launches FightCade, but it does require FightCade to be installed.** The app bypasses the client entirely (no `fcade`, no Electron lobby, no matchmaking), yet the FightCade installation is the *provider* of everything the launch needs: the only emulator binary with `quark:direct` (`fcadefbneo.exe`; upstream FBNeo lacks the mode), Wine + `.wine32` (macOS), the Flatpak sandbox (Linux), the ROM directory, and `fcadefbneo.ini`. Uninstalling FightCade breaks every launcher, and the binary cannot be bundled (the FightCade fork and `ggponet.dll` are not public). The only route to removing the dependency is the RetroArch FBNeo core (§2), which revisits the netplay stack rather than just packaging. See [`06-redesign.md`](06-redesign.md) §2.

### Spectator architecture (key finding)

Static analysis of the installed binaries:

- `fcadefbneo.exe` supports `quark:direct`, `quark:served`, `quark:stream`, `quark:replay`, `quark:training`.
- `ggponet.dll` contains a `SpectatorBackend`, the export `ggpo_start_streaming`, and FightCade-only TCP code (`TcpProtocol`, `Sending streamed frame to quark`, `Received event %d from server`, `tcp.cpp`/`tcp_proto.cpp`).
- Imports from `ggponet.dll`: `ggpo_start_session`, `ggpo_start_streaming`, `ggpo_start_replay`, `ggpo_close_session`, `ggpo_idle`, `ggpo_synchronize_input`, `ggpo_get_stats`, `ggpo_advance_frame`, `ggpo_client_connect`, `ggpo_client_chat`. Exports add `ggpo_client_set_game_event`, `ggpo_set_frame_delay`, `ggpo_log`, `ggpo_logv`, `ggpo_start_synctest`, no `ggpo_start_spectating`.
- Upstream GGPO *does* support P2P spectating (`ggpo_start_spectating(..., host_ip, host_port)`; "any player may serve as host"), but FightCade replaced it with the server-side `ggpo_start_streaming`.

**Conclusion:** FightCade spectating = emulator opens TCP to FightCade's Quark server and is fed a stream. `quark:direct` has no spectator field and never contacts the server. True input-relay spectate therefore requires either (a) a `ggponet.dll` shim, or (b) a different netplay stack.

## 2. What RetroArch is (and why it matters)

RetroArch is an open-source multi-emulator frontend; it loads "cores", and **FBNeo is a core** — the same emulator engine, native on macOS/Windows/Linux, no Wine/Flatpak.

Its netplay (from official docs):

- TCP, server-authoritative. The server can be a player **or a non-playing spectator**.
- Up to 16 players and **many spectators**; "spectators send no input data" — they receive the stream and render locally. This is input-relay, **not video**.
- Replay/rollback: clients rewind/replay on delayed input. Requires identical core + content CRC + deterministic core.
- Per-packet cost ~28 bytes/frame (~1.7 KB/s per player); the server duplicates to spectators → 2 spectators ≈ ~5 KB/s total. Video would be ~1000× more.
- Spectator CPU cost is on the spectator's own machine (one extra emulator instance), not the host.
- Lobby is optional; manual "connect to host IP" with NAT traversal off is exactly right over Tailscale.

Tradeoff: FightCade uses dedicated UDP/GGPO and is generally regarded as better for fighting-game feel; RetroArch is TCP/replay (head-of-line blocking) but wins on spectating, cross-platform simplicity, and openness.

## 3. Decisions

- **App name: `The Cabinet`** (slug `the-cabinet`). Git repository: **public** at `Tunmiseoni/the-cabinet` since the 2026-09-19 history scrub (§8).
- **Stack: Tauri v2 + Vite.** Rust backend for process/socket/tailnet work; the UI is a Vite-built web frontend rendered in the OS webview. Frontend stack is **React + TypeScript + Tailwind CSS v4 + shadcn/ui** (vanilla HTML/CSS/JS was the initial sketch). Node/npm is used only for the frontend build and the Tauri CLI, not the backend.
- **Rust toolchain via `rustup`** (not Homebrew's keg-only `rustup`, which conflicts with the `rust` formula). See §6 Prerequisites.
- **Architecture: coordination layer, not a FightCade clone.** Reuse `quark:direct` and the existing scripts; the app orchestrates.
- **Do not clone/mod FightCade.** Server, helper, and spectator relay are closed; `github.com/Fightcade` has no public repos.
- **RetroArch is a co-equal candidate, not a consolation fallback.** Its spectator support is input-based and free.
- **Phase 0 gate = one-day RetroArch spectator spike.** If netplay feel is acceptable at <100 ms, make RetroArch primary and skip `ggponet` RE. If not, attempt the shim (hard time-box), then fall back to deferring spectate.
- **Keep the FightCade install** for 2-player play regardless; if the shim fails we simply lose true spectate on that path.
- **Removed 2026-09-21: the rooms/lobbies/KotH subsystem and all score/result tracking.** The host-authoritative room (`player`/`room` state machine, UDP discovery, TCP control channel, `service` orchestration), the local per-opponent lifetime score ledger (`scores`), and the emulator overlay/result watcher (`results`) were deleted outright. Reasons: the room model and its shared ledger did not work well, and lifetime score tracking was not wanted. The direct-launch + connection-health flow is unaffected. A replacement lobby was **designed fresh in a `/grill-me` session (2026-09-21)** — see [`09-lobby.md`](09-lobby.md), gated on the hardware spike in [`10-lobby-spike.md`](10-lobby-spike.md). The accepted root cause of v1 was overlay-based result detection plus an untrusted host-authoritative ledger, with room liveness on a bespoke TCP control channel; the replacement is decentralized, uses per-ROM RAM result detection over RetroArch's command socket, and drops the custom control protocol. No v1 scaffolding was kept.
- **All four machines run the same build.** If a shim is used, all four carry the shimmed emulator (any of them may be the serving player).
- **Spectator acceptance bar: 2 concurrent spectators** (4-person group: 2 play, 2 watch).
- **Connection health is a first-class feature.** Tailnet-level signals first (`tailscale status --json` + `tailscale ping`: RTT, direct-vs-relay, tx/rx) — no emulator cooperation needed. Fine-grained GGPO stats (ping, queue lengths, frames-behind) only later, via the shim path if ever. Complementary: recommend `bShowFPS 2` (ping+jitter on-screen overlay) for direct matches.
- **Gated proposals: emotes, opt-in voice, and input recording / match history.** Match reactions, mic/audio (default off, explicit per-session opt-in), and recording match inputs for replays are **not** in scope until a **/grill-me session** stress-tests each. Voice especially must answer "why not Discord?"; recording/replay must answer "why not FightCade's existing `quark:replay`/`.fr` files?" and settle determinism/storage/consent before it enters the roadmap. **Update 2026-09-21:** the lobby grill settled match history (local, per-machine set/round index) and replay approach (RetroArch's native `.replay`) — both are now scoped inside [`09-lobby.md`](09-lobby.md) (history in the lobby; replays deferred behind spike item S8). Emotes and voice remain gated.
- **Gated: UX redesign + Cabinet mode (host the emulator inside the app).** Proposed in [`06-redesign.md`](06-redesign.md): a full-screen cabinet bezel/HUD framing a viewport the emulator window is placed into (macOS Accessibility, Windows `SetParent`, X11 reparent, Wayland fallback), with a frame-follow fallback when placement is unsupported or unpermitted, plus the wider single-window information-architecture redesign. **macOS-first Cabinet mode is implemented off by default (`cabinetMode`; spike R0 + R2 done 2026-09-19)**, but the wider redesign and the cross-platform hosts remain **gated on a `/grill-me` session**. Does not block Phases 1–4.
- **Development harness:** abstract "launch an emulator instance" so dev can use isolated/native instances instead of three Wine instances sharing one `WINEPREFIX`. **Implemented:** a `launch_dev_pair` command starts both sides (P1 local 7001, P2 local 7000) on `127.0.0.1`, plus a developer-loopback option for a single instance. Two Wine instances in the shared FightCade prefix were verified to coexist and bind their ports correctly, so no prefix isolation is needed for pairs. The home-screen **Dev pair** button and the loopback launch option are hidden unless the `developerMode` setting is on. **Developer mode is session-scoped:** it is forced off at every app start (`Config::reset_developer_mode` in `lib.rs` setup), so it never carries across launches even if a previous session saved it on.
- **Shim constraints (if used):** pin FightCade 2.1.45, disable auto-update, run Linux from a writable copy of `fbneo/`, and re-sign the macOS `.app` after replacing `ggponet.dll`.
- **Docs:** this file is the spec; `01`–`03` are amended for the direct-path finding and kept as history; `AGENTS.md` records conventions and setup.
- **Distribution: GitHub Actions builds a per-OS matrix and publishes installers to GitHub Releases (implemented 2026-09-19).** `.github/workflows/release.yml` builds macOS arm64, Linux x86_64, and Windows x86_64 natively and uploads the bundles to a **draft** GitHub Release via `tauri-action`; `.github/workflows/ci.yml` runs the test/clippy gate (including `cargo fmt --check`) on all three OSes for every push/PR (`.github/workflows/smoke.yml` is a separate manual `workflow_dispatch` smoke-launch that builds each OS and uploads a screenshot; it is not run by `ci.yml` or `release.yml`). Each OS builds its own binary (Tauri does not cross-compile cleanly); friends download the release artifact and run it, without cloning the repo or installing a toolchain. Public-repo Actions runners are free, so the matrix has no minute cost. **In-app auto-update via `tauri-plugin-updater` is implemented (2026-09-24):** `bundle.createUpdaterArtifacts` is on, so the release emits the updater payloads — macOS `.app.tar.gz`, Linux AppImage, Windows NSIS `-setup.exe`, each with a `.sig` — and `tauri-action` publishes a `latest.json` beside them. The app checks `releases/latest/download/latest.json` once on launch (skipped in dev) and on demand from Settings → Application update, offering a home-screen banner; installing downloads the payload, verifies its Minisign signature, swaps it in place, and relaunches. Only **published** releases are visible (the workflow still opens a draft), so nothing reaches friends until the release is published. The **first updater-enabled build must be installed by hand** (older builds carry no updater); after that, updates are in-app. Linux `.deb` installs cannot self-update (AppImage only). macOS stays ad-hoc signed (see below): the updater's Minisign signature is independent of Apple code signing, and because the app fetches the payload itself there is no `com.apple.quarantine` attribute to trip Gatekeeper — **still to be verified live**. The private signing key lives outside the repo (`~/.tauri/the-cabinet-updater.key`, no password) and in GitHub Secrets (`TAURI_SIGNING_PRIVATE_KEY`); `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is set empty. Local `tauri build` now needs that key too, since `createUpdaterArtifacts` is on (the `cargo`/frontend test gate is unaffected). The **Windows launcher adapter is implemented** (`launcher/windows.rs`, native `fcadefbneo.exe`), so the Windows friend can play.
- **Repository visibility: public, after the §8 publication scrub (completed 2026-09-19).** A public repo makes release-asset downloads anonymous (no GitHub account/collaborator access for the friends) and removes the private-repo Actions minute cap. The history rewrite (`git-filter-repo` over all commits) and the visibility flip were done together on 2026-09-19; the scrub replaced the real addresses/handles with the placeholders recorded in §8.
- **macOS builds are ad-hoc signed (`bundle.macOS.signingIdentity: "-"`), not notarized (2026-09-19).** Tauri's bundler does not sign at all when no identity is configured, so the bundler left only the linker's ad-hoc stamp: no `_CodeSignature/CodeResources`, `Info.plist` unbound, and Gatekeeper reported the downloaded app as **"damaged"** (the `v0.1.2` regression). Ad-hoc signing produces a valid bundle signature, so the app opens after the normal "unidentified developer" bypass (right-click → Open, or Privacy & Security → Open Anyway); the `.github/workflows/release.yml` gate now runs `codesign --verify --deep --strict` on both the `.app` and the DMG's copy before a release is published. Removing the last prompt requires a paid Apple Developer account and notarization.
- **Linux AppImage: a post-build workaround strips over-bundled display-stack libraries (temporary, 2026-09-20).** See the subsection below; removed once upstream Tauri can exclude them via config.
- **Linux install detection is resilient and AppImage-safe (2026-09-20).** Host tools (`flatpak`, `wine`, `tailscale`) are spawned with `LD_LIBRARY_PATH`/`LD_PRELOAD` removed (`process.rs`), so the AppImage's bundled libraries never shadow the host's. Without this, `flatpak info com.fightcade.Fightcade` linked the bundle's glib while loading the host's `libostree-1.so.1`, exited non-zero, and the app reported *"no FightCade Flatpak or native install found"* even though it had already found the ROMs under `~/.var/app/com.fightcade.Fightcade/data` (ROM scanning only stats the directory; Flatpak detection ran the CLI). Detection now also: treats an existing Flatpak data dir as proof of a Flatpak install (not only `flatpak info` success), searches `~/Games/*` to match `scripts/fcade-lan-linux.sh`, accepts a Flatpak data dir as a settings override, and derives a native or Flatpak install from the resolved ROM directory as a last resort.
- **Diagnostics: file logging, full emulator capture, and a blocking port preflight (2026-09-20).** The app logs to `app_log_dir` via `tauri-plugin-log` (Info by default; `verboseLogging` raises it to Debug after a restart; 5 MB rotation, keep 3). Every launch captures the emulator's stdout+stderr to `app_log_dir/sessions/<utc>/emulator-<role>.log` for **all** providers; RetroArch additionally gets `--verbose`, so its netplay lines land in the file. Before a non-dev RetroArch client/spectator launch the app probes `peer:55435` (TCP, 3 s) and refuses with a clear message when the host is unreachable — the connection dialog's "Launch anyway" sets `force` to override — and dev pairs wait for the loopback host to bind before starting the client. Settings → Diagnostics offers *Open logs folder* and *Collect diagnostics* (writes a bundle: version, config, provider, tailnet, last match, session index; the structured sections redact home paths, IPv4/IPv6, `*.ts.net` names, and the configured handle/peer, and the bundle tails the latest session's emulator logs plus the recent app-log with a "not redacted" warning); session dirs are pruned to `SESSION_KEEP`. All backend logging goes through `tauri-plugin-log` (no stray `eprintln!`), and mutexes use `sync::MutexExt::lock_or_recover`, so a poisoned lock cannot panic a worker. The captured netplay lines are parsed into `MatchState.instances[].netplay` (`netplay.rs`) — connection status, player slots, ping, and a capped event log — so live match health is visible in the UI; the benign host reverse-probe lines (`Failed to connect to client.`) are ignored. Logs contain tailnet IPs, so they live outside the repo and must never be committed.
- **RetroArch netplay settings, opt-in config isolation, and frozen-core download (2026-09-21).** Settings → RetroArch gains three fields: `retroarchMaxPingMs` (default 0 = off) emits `netplay_max_ping`; `retroarchMuteSpectators` (default on) emits `audio_mute_enable = "true"` for spectators; and `retroarchIsolatedConfig` (default **off**) writes a minimal `base.cfg` and passes `-c` ahead of `--appendconfig`, re-pointing `input_autoconfig_dir` at the host's real autoconfig folder so shaders/playlists don't leak in (a loopback retest passed 2026-09-21 — `base.cfg` loads, Vulkan stock shader instead of the user's GL/GLSL, keyboard input works; the default stays off pending a product decision). The frozen cores are distributed as assets of a **published** GitHub Release (`retroarch-cores-v1`, uploaded by `scripts/publish-cores.sh` with `parity-manifest.txt` + `CORE-NOTICE.txt`), and installed in-app by a **Download frozen core** button that fetches the host's asset and verifies its sha256 (`ureq` + `sha2`) into `<app_data_dir>/cores/<platform>/`. **2026-09-21:** the managed core is stored under its canonical name `fbneo_libretro.<ext>`; an earlier release-asset-named copy (`fbneo_libretro-<platform>.<ext>`) is still found and is migrated to the canonical name on provider startup, and a completed download clears any stale asset-named leftover. When detection fails, `detect()` reports which side is missing specifically — the RetroArch binary (with an install hint) or the core (pointing at Settings → RetroArch → Download frozen core). A `netplay_spectate_password` field was considered and **dropped** (the tailnet is already invite-only). The cores release is **published** as `retroarch-cores-v1` (2026-09-21) and the in-app download was verified end-to-end. See [`07-retroarch-spike.md`](07-retroarch-spike.md) §8/§9 and F8/F10/F17. **2026-09-21:** macOS sessions also pin `video_driver = "vulkan"` (all roles) — RetroArch's Metal driver is broken on Apple Silicon + macOS 26 (~8 fps), and a menu-selected driver is not persisted because sessions set `config_save_on_exit = "false"`; Linux/Windows keep the user's driver (F22).
- **SF3 keyboard preset for Cabinet-launched sessions (2026-09-21).** Settings → RetroArch gains an editable **keyboard preset** (default on) that maps a 6-button fighting-game layout to FBNeo's Classic RetroPad for Cabinet launches only: Up/Down/Left/Right = Space/S/A/D, LP/MP/HP = U/I/O, LK/MK/HK = J/K/L, Start/Coin = 1/5 (RetroPad Y/X/L/B/A/R/start/select). It is applied in the per-role `--appendconfig` (`input_player1_*` for any seated instance — host or joiner — with `input_player2_*` duplicated for a seat-2 instance; spectators send no input), so the real `retroarch.cfg` is only **read**, never written. RetroArch allows only one keyboard key per action, so keys that already drive a hotkey have that hotkey set to `nul` **for the session** — the effective hotkey set is resolved from the host `retroarch.cfg` (or upstream defaults under config isolation), and only the colliding ones are disabled, so save-state/screenshot/exit keep working. `retroarch_hotkey_map` returns the effective bindings plus which collide so Settings can flag them.
- **FBNeo SOCD preset, applied through a per-session core-options file (2026-09-22).** Settings → RetroArch gains a **SOCD preset** (default **Simultaneous Neutral**) choosing how the FBNeo core resolves opposing directions, e.g. held left+right. The core's own default is *Last Input Priority (8 Way)*, which is the behavior the preset exists to change; `disabled`/`neutral`/`last4`/`last8`/`first`/`up`/`down` map to the core option `fbneo-socd` values `0`–`6`. It is written for **every** Cabinet launch, independent of the keyboard preset (SOCD matters for gamepads too). Core options cannot be set from `--appendconfig`: RetroArch reads them from a file, and a per-core `config/<core>/<core>.opt` (else, when enabled, a per-game `.opt`) wins over `core_options_path`. So `write_overrides` also emits `global_core_options = "true"`, `game_specific_options = "false"`, and `core_options_path = "<overrides>/<role>/core-options.cfg"`, and writes that file with the SOCD pin first plus the player's existing FBNeo options (their `fbneo-socd` dropped) so their DIPs/samplerate/hiscores survive; `core::resolve_core_options_source` mirrors RetroArch's resolution (per-game `.opt` → per-core `.opt` → the configured global `core_options_path`/`retroarch-core-options.cfg`). SOCD is applied by each peer's own core **after** netplay input sync, so peers with different modes can diverge: the preset is per machine, every player in a room should leave it at the shared default, and a room-wide pin (advertised on the beacon) is deferred. `core::resolve_core_options_source` takes the per-core directory from the same rule RetroArch uses (`runloop.c`'s `validate_per_core_options` + `APPLICATION_SPECIAL_DIRECTORY_CONFIG`, verified against 1.22.2 source): the host's **Config directory** (`rgui_config_directory`, commonly a `~` path) if set, else the directory of the config RetroArch loaded, else the platform default `<root>/config` — `~/Library/Application Support/RetroArch/config` on macOS, `~/.config/retroarch/config` (and the Flatpak root) on Linux, and the executable's `config\` on Windows. The lookup is content-filtered (`fbneo-` keys), and an ignored live test reports what a given machine resolves: `cargo test live_resolves_the_host_core_options -- --ignored --nocapture`. See [`07-retroarch-spike.md`](07-retroarch-spike.md) F23 and [`09-lobby.md`](09-lobby.md) §5.
- **The frozen core is made visible to RetroArch's own core scan (2026-09-21).** RetroArch decides whether a loaded core supports netplay via `core_info_current_supports_netplay()`, which looks the core up **by basename** in RetroArch's *scanned* core list. Our managed core lives under `app_data_dir`, outside that list, so on a machine with no other FBNeo install the lookup misses, the entry stays zeroed, and `init_netplay` refuses with *"Core does not support netplay."* (this was the Linux friend's failure; a peer only passed because an unrelated `fbneo_libretro.<ext>` happened to sit in its RetroArch cores folder). To fix it, the launch preflight (`launch_match_inner`) calls `provider.ensure_core_visible()`: if the directory RetroArch scans — `libretro_directory` read from the host `retroarch.cfg`, else the program-relative `cores/`, else the per-OS default — has no `fbneo_libretro.<ext>`, the app copies the core it is about to load into it and writes a deterministic `fbneo_libretro.info` beside it; an existing file is never clobbered (the basename lookup hits either way, and `-L` still loads our frozen core for parity). `download_managed_core` writes the same `.info` next to the managed core, and the netplay tracker surfaces `Core does not support netplay`/`...between different platforms` as a failed connection with an event. A `core_info_savestate_bypass = "true"` override is also emitted for newer RetroArch, but it does not exist on older builds and is not the fix. See [`10-lobby-spike.md`](10-lobby-spike.md) L9.
- **Lobby sets + FIFO rotation (2026-09-21).** `lobby/sets.rs` rolls `results.rs` round outcomes into first-to-two-round **games** and a first-to-N **set** (draws advance nothing), and derives the two seats and the waiting queue from the host's netplay observation (keyed by netplay nickname). Because the RetroArch command socket is loopback-only, the host cannot toggle a remote instance: on set end the host advertises a **rotation instruction** (`{id, loserSlot, incoming}`) on the beacon, and each machine's own reconciler toggles its local instance once per id (loser steps out, queue head steps in). With nobody waiting, no rotation is advertised and the same two players start the next set. The join UI collapsed to a single **Join** (a full room joins as a spectator instead of erroring), a host-side RAM runner publishes the live score, a joiner-side follower polls the beacon, and `RoomView.tsx` shows seats, score, queue, and the pending rotation. See [`09-lobby.md`](09-lobby.md) §7.2–§7.4/§12. Queue observability (does the host's netplay log expose spectator nicks?) and the host's own step-out need a live tailnet check — recorded in `09-lobby.md` §9; game-boundary `RESET` remains spike S9.
- **Cleanup pass + deferred backlog (2026-09-20).** A balanced, behavior-preserving refactor landed: uniform `log` logging, a `sync::MutexExt::lock_or_recover` helper, new `constants.rs`/`time.rs`, `diagnostics.rs` split out of `commands.rs`, `retroarch.rs` moved to the top level, `session::LaunchOptions`, and a frontend `types.ts`/`hooks.ts` split. Follow-up passes (2026-09-21) landed the `providers/` tree, the neutral `contracts` module, the frontend query layer (`frontend/src/lib/query.ts`), the placeholder-identifier genericization, and then the two heavier refactors: **typed errors** (`error.rs`'s `CabinetError` + `CommandError` boundary, replacing every `Result<_, String>`), the **`time` crate**, and the oversized-file splits (`tailscale`, `diagnostics`, `windowing/macos`, `launcher/linux`, `commands`). Only low-value items remain deferred (see [`08-cleanup.md`](08-cleanup.md) §3/§5c).

### Linux AppImage EGL workaround (temporary — remove when upstream fixes bundling)

The Linux AppImage is post-processed by `scripts/patch-appimage.sh`, called by `scripts/tauri-build.sh` (the `tauriScript` for `release.yml`, so the patched file is what `tauri-action` uploads) and by `scripts/build.sh` for local/source builds. It removes the display-stack libraries from the built `*.AppDir` and repacks with `appimagetool`.

- **Symptom:** on Mesa 25+ hosts (the CachyOS friend runs Mesa 26) the AppImage opens a blank window; `WebKitWebProcess` aborts with `Could not create default EGL display: EGL_BAD_PARAMETER`, and no `WEBKIT_*` flag (DMA-BUF, compositing, software GL) helps.
- **Cause:** Tauri's linuxdeploy bundler sweeps the Ubuntu 22.04 build machine's Wayland/X11 client libraries (`libwayland-*`, `libxkbcommon*`, `libxcb-*`, `libXau`, `libXdmcp`) into the bundle and puts them ahead of the host's via `LD_LIBRARY_PATH`; the host's Mesa then fails to negotiate EGL. Upstream: tauri-apps/tauri#15976 (root cause + minimal fix), #15665.
- **Verified 2026-09-20** on the CachyOS machine via `scripts/diagnose-linux.sh`: the stock `v0.1.3` AppImage aborts with the EGL error, while both `LD_PRELOAD`-ing the host `libwayland-client` and removing the 10 bundled libraries render the UI.
- **Removal (the whole point of the workaround):** when upstream PR #15662 (`bundle.linux.appimage.excludeLibraries`) lands, bump `@tauri-apps/cli` (`package.json`) and the `tauri` crate (`Cargo.toml`), add to `src-tauri/tauri.conf.json`:
  `"bundle": { "linux": { "appimage": { "excludeLibraries": ["libwayland-*.so*", "libxkbcommon*.so*", "libxcb-*.so*", "libXau.so*", "libXdmcp.so*"] } } }`,
  then delete `scripts/patch-appimage.sh`. The guarded call sites in `scripts/tauri-build.sh` and `scripts/build.sh` become no-ops, and the `tauriScript` line in `release.yml` can stay. Nothing else needs reverting.
- **Updater interaction (2026-09-24):** repacking changes the AppImage's bytes, so the `.sig` Tauri wrote during the build no longer matches and the in-app updater would reject the Linux update. The script therefore **re-signs** the repacked AppImage with `tauri signer sign` when `TAURI_SIGNING_PRIVATE_KEY` is present (CI), and drops the stale `.sig` rather than shipping a wrong one when it is not.

## 4. Architecture

```
the-cabinet
├─ frontend/           -> Vite + React + Tailwind v4 + shadcn/ui (launcher + connection health)
└─ src-tauri/          -> Rust backend
   ├─ peer registry    -> `tailscale status --json` (stable 100.x / MagicDNS / node id)
   ├─ nethealth        -> per-peer RTT/path polling (`tailscale status --json` + `tailscale ping`); peer badges + pre-match gate
   ├─ ROM index        -> scan the configured emulator's ROM dir
   ├─ session launcher -> port of scripts/fcade-lan-{macos,linux,windows}
   ├─ match supervisor -> watches process exit + live peer health
   └─ provider layer   -> FightCade (Wine/Flatpak/native) + RetroArch (native netplay)
```

Gated components (no build order until grilled, see Decisions): `replay recorder` (capture per-match input streams + core/ROM identity; likely reuses the emulator's native replay format rather than a custom one) and `match history` (index of past matches with playback, backed by the recorder).

Launcher abstraction covers: macOS Wine (`wine32on64` + `.wine32`), Linux Flatpak (`com.fightcade.Fightcade`) or native, Windows native, and a developer loopback mode. Phase 1 implements the macOS Wine adapter and the loopback mode; the **Linux adapter is implemented** (Flatpak-preferred, native/Wine fallback, `launcher/linux.rs`) and the **Windows adapter is implemented** (`launcher/windows.rs`, native `fcadefbneo.exe`). The Rust modules map onto the diagram as `tailscale` (peer registry + nethealth), `roms` (ROM index), `launcher` (session launcher abstraction), `session` (match supervisor), `providers` (FightCade vs. RetroArch), and `config`/`commands` (settings + IPC).

Connection health (`nethealth`) polls `tailscale status --json` for per-peer `Online`, `CurAddr`/`Relay`, `Active`, `TxBytes`/`RxBytes`, and runs `tailscale ping --c N <peer>` for RTT plus the `via direct …` vs `via DERP(…)` path. The home screen shows one badge per peer (RTT in ms + path); launching warns when the path is relayed or RTT is above threshold. In-match the wrapper re-pings periodically — coarse by design, since per-frame GGPO stats live inside the closed emulator (visible on-screen via `bShowFPS 2`, ping+jitter). **Implemented (2026-09-19):** a session health thread pings the match peer every 4 s and the launcher card shows the live RTT/path badge. Fine-grained GGPO stats (queue lengths, frames-behind) arrive only with the shim path, if ever; on the RetroArch path the tailnet-level signals are the wrapper's display.

Gated extras (no build order until grilled): `reactions` (match emotes rendered by the wrapper overlay — never injected into the emulator), `voice` (opt-in mic/audio, default off, per-session consent with a visible live indicator), and `input recording / match history` (record each match's inputs — not video — keyed to core + ROM identity so past matches can be listed and replayed; lean on the emulator's existing replay encoding where possible). None of these block Phases 1–3.

## 5. King of the Hill and score tracking (removed)

> The replacement lobby is designed in [`09-lobby.md`](09-lobby.md) and gated on the spike in
> [`10-lobby-spike.md`](10-lobby-spike.md). Everything below is the historical record of what was
> removed.

**Removed 2026-09-21.** The room/KotH state machine, per-player ledger, UDP discovery, TCP control protocol, host-authoritative orchestration, and overlay-based result detection were all deleted. The implementation lived in `src-tauri/src/{player,room,discovery,control,service,scores,results}.rs` plus the `RoomCard`/`RoomsCard`/`LifetimeCard` frontend cards; none of it remains.

Why it went:
- The host-authoritative room model and its shared ledger did not work well in practice.
- Lifetime score tracking (per-opponent W/L derived from the emulator overlay) was not wanted.

What this means:
- The app was, until the replacement lobby landed, a **direct launcher**: pick a ROM + an online peer + a role, launch `quark:direct` (FightCade) or RetroArch netplay, and watch connection health. The **Lobby is now the preferred path** ([`09-lobby.md`](09-lobby.md)); the direct **Launch match** card is gated behind Developer mode and kept for loopback diagnostics, while `launch_match`/`launch_dev_pair` remain registered for it. There is still no queue or host-authoritative score state.
- The UDP discovery port (`47810`) and TCP control port (`47811`) are gone, along with their settings fields.
- The `fbneo/fightcade/` overlay watcher is gone; the app no longer reads `winner.txt`/`p1score.txt`/`p2score.txt`. `bVidSaveOverlayFiles` is no longer touched, and the launcher card no longer shows a match result line.
- No scaffolding was kept. A replacement lobby, if designed, starts fresh.

The full protocol/ledger/result design below is retained only as a historical record of what was removed.

State machine held by the room host:

- Room state: `champion`, `challenger`, `queue[]`, `currentMatch`.
- On match end: detect winner (overlay file / exit / manual), mutate the ladder (winner stays), promote the next challenger, launch the next `quark:direct` pair.
- Roles per match: assign P1/P2 (side 0 / side 1).

Per-player ledger:

- Fields: `wins`, `losses`, `draws`, `games_played`, optional `current_streak` / `best_streak`.
- Storage: room-host local store (JSON or SQLite) so scores survive sessions and can be shown in the lobby.
- **Implemented (local, 2026-09-19):** `scores.rs` persists a per-opponent ledger to `<app_config_dir>/scores.json`. Because direct mode bypasses `fcade`, there are no real handles — the opponent key is the launch **peer IP**, and the local player's identity is the launch **side** (`plans[].config.side`). A game is counted from an overlay score increment: the session diffs `p1score`/`p2score`, and a single-game delta (`+1` on exactly one side) is attributed as win/loss to the local side. Counter resets and over-large deltas are ignored (no double counting). Dev-pair (loopback) sessions are excluded. **Verified live (2026-09-19, real direct match):** a decisive game incremented the ledger — `scores.json` recorded `1W/1L` for the peer over two games, and the lobby's lifetime card updated in place without a restart.
- Result sources, in order of preference:
  1. `fbneo/fightcade/winner.txt` (+ `p1score.txt`/`p2score.txt`) when `bVidSaveOverlayFiles 1` — **verified in direct mode (2026-09-19)**. Details in §7.
  2. Emulator exit + per-player confirm.
  3. Manual dispute resolution by the room host.
- Draw handling: a drawn game (double KO / timeout with equal rounds) **does not mutate the ladder — the same two players replay it**. A manual "Draw / replay" button lets a player request it when the overlay is ambiguous.

### Room protocol (Phase 2, removed)

Agreed wire format, implemented incrementally in Phase 2 (2.1 state machine → 2.2 discovery → 2.3 control → 2.4 orchestration → 2.5 shared ledger → 2.6 mid-match re-ping), then removed entirely on 2026-09-21.

**Identity.** A player was keyed by the Tailscale **node id** (the `Peer` map key in `tailscale status --json`, and `Self.ID`) — stable across IP/`DNSName`/hostname changes. The display name was a user-set **handle** (defaults to the tailnet hostname). The room state carried `node_id`, `handle`, and `ip` for display/launch purposes.

**Discovery (UDP, default `47810`, configurable).** Each app probed every online tailnet peer's discovery port with a small JSON datagram and waited briefly for replies:

```
-> {"magic":"cabinet/1","kind":"probe"}
<- {"magic":"cabinet/1","kind":"room","roomId":"...","host":"<handle>","rom":"sfiii3nr1",
    "phase":"lobby|playing","champion":"<handle>","queue":3,"players":2,"secretRequired":true}
```

A host listened on the port; a non-host replied nothing. An empty list meant nobody was hosting. Discovery revealed no secrets.

**Control (TCP, default `47811`, configurable).** Newline-delimited JSON, one message per line. The host listened; clients connected and drove:

```
client -> {"v":1,"kind":"hello","roomId":"...","nodeId":"...","handle":"...","secret":"...","want":"play|spectate"}
host   -> {"v":1,"kind":"welcome","playerId":"...","state":{...RoomState}}
client -> {"v":1,"kind":"enqueue"}          // join the KotH queue
client -> {"v":1,"kind":"leave"}
client -> {"v":1,"kind":"result","matchId":"...","outcome":"win|loss","confidence":"overlay|manual"}
host   -> {"v":1,"kind":"state","state":{...RoomState}}     // on every change
host   -> {"v":1,"kind":"ping"} / client -> {"v":1,"kind":"pong"}
```

The client could open this connection only if it presented the room's `secret`; a wrong/missing secret was rejected with an `error` message.

**RoomState (host-authoritative, versioned).**

```
{ "roomId", "host": {nodeId,handle,ip}, "rom", "revision":N,
  "phase": "lobby|playing",
  "champion": {nodeId,handle} | null,
  "challenger": {nodeId,handle} | null,
  "queue": [{nodeId,handle}],
  "currentMatch": { "matchId", "p1":{nodeId,handle,ip,side}, "p2":{...}, "startedAtMs" } | null,
  "ledger": { "<nodeId>": {handle,wins,losses,draws,games} } }
```

**Orchestration (authority-pull).** On every state message the client compared `currentMatch` against its own `nodeId`:
- named in `currentMatch` → launch `quark:direct` locally with the assigned side and the opponent's `ip`;
- was in a match, now not → stop its instance;
- not named → stay idle.

**Result flow.** The client watched its **local** overlay (`results.rs`) and knew its own side. On a decisive score increment it sent `result` with `matchId` (to reject stale reports) and `confidence=overlay`; the host recorded W/L and advanced. If a match ended with no overlay signal, the existing manual "I won" button (`confidence=manual`) applied.

**Shared ledger.** The host persisted `RoomState.ledger` (per `node_id`) to `<app_config_dir>/room-ledger.json` so scores survived sessions. When a room session was active, the local `quark` launches were host-driven and the local ledger was bypassed (`track_scores=false`) to avoid double counting.

## 6. Phased plan

| Phase | Work | Gate |
|---|---|---|
| 0 | RetroArch spectator spike: host + 1 client + 2 spectators over Tailscale; measure feel and bandwidth. **Local loopback verified 2026-09-20 (2 players + 2 spectators, parity, spectator no-input, no desync). Live cross-OS tailnet verified 2026-09-20 (macOS host + Linux client + Windows spectator, ~11 min synced, ~55–83 ms; feel rated acceptable). 2 concurrent spectators proven on loopback only — tailnet run deferred.** Decision §7 row 1: RetroArch primary spectator path, skip the shim. See [`07-retroarch-spike.md`](07-retroarch-spike.md) | Feel acceptable at <100 ms? **Yes (2026-09-20)** |
| 0b | If not: FightCade `ggponet.dll` shim spike (static RE of `quark:stream` arg semantics, then a minimal shim) | Hard time-box; if the `quark:stream` middle token is a server session id, stop |
| 1 | Tauri launcher: peer registry, ROM index, side/role, spawn/teardown, Wine/Flatpak/native; **connection health** (per-peer ping + direct/relay badge, pre-match warn on relay/high RTT) | Replaces scripts with equivalent behavior; unhealthy peer flagged before launch |
| 2 | ~~Room + KotH + score ledger; auto-launch next pair; result detection; periodic re-ping of match peers.~~ **Removed 2026-09-21** — the room/KotH/ledger/result-tracking subsystem was deleted. The mid-match re-ping (former 2.6) survives as connection health. See §5 | Dropped |
| 3 | Spectate integration via the Phase 0 outcome; else documented as deferred; show spectator link quality with the same nethealth signals | 2 concurrent spectators |
| **Delivery** | GitHub Actions per-OS build matrix publishing installers to GitHub Releases; repo public (history scrubbed 2026-09-19, §8) so downloads are anonymous (see §3 and §7) — promoted ahead of further live testing. **Implemented 2026-09-19:** `.github/workflows/ci.yml` (3-OS test/clippy gate) + `release.yml` (draft Release via `tauri-action`), and the Windows launcher adapter. **2026-09-24:** in-app auto-update (`tauri-plugin-updater` over the signed `latest.json` feed) replaced manual re-download; the AppImage self-updates, while `.deb` and any pre-updater install still download by hand | Friend machines install once, then update from inside the app without cloning or rebuilding |
| 4 | Docs + packaging | Reproducible on all four machines |
| **Lobby** | Replacement for Phase 2: RetroArch-only rooms (beacon discovery + manual join), first-to-N sets with automatic winner-stays rotation, per-ROM RAM result detection, local per-machine scores/history. Designed in [`09-lobby.md`](09-lobby.md); result detection reads health via `READ_CORE_RAM` (no per-fighter counter exists). **Largely built 2026-09-21:** control-socket plumbing (`providers/retroarch/command.rs`), per-ROM result detection (`lobby/results.rs`), the room model + HTTP beacon (`lobby/room.rs`/`beacon.rs`), hosting, the seat/role split, the client join flow (`lobby_*` commands), `LobbyCard.tsx` (the preferred home-screen path, with a Stop match control), and — added 2026-09-21 — the first-to-N sets + FIFO rotation layer (`lobby/sets.rs`, the host set runner and joiner seat follower, the beacon rotation instruction, the single **Join**, and `RoomView.tsx`); the local score/history ledger and game-boundary `RESET` remain. The direct **Launch match** card is now developer-mode only; `lobby_start`/`lobby_join` call the shared `launch_match_inner` while the `launch_match`/`launch_dev_pair` commands stay for that card. | **Build gate (S1–S5) passed locally 2026-09-21**; **release gate [`10-lobby-spike.md`](10-lobby-spike.md) S6–S7 pending** (tailnet soak, beacon reachability) |
| Gated | Emotes + opt-in voice. (Match history and replays were moved into the lobby: history scoped in [`09-lobby.md`](09-lobby.md), replays deferred behind spike item S8.) | Admitted to the roadmap only after a /grill-me session |
| Redesign | **Cabinet mode** — host the emulator inside the app (bezel/HUD + measured viewport; per-OS placement, frame-follow fallback) — and the wider single-window IA redesign. See [`06-redesign.md`](06-redesign.md); R0 spike + macOS R2 implemented off by default, cross-platform hosts and the IA redesign still gated | Admitted only after a /grill-me session (agenda in 06) |

**Progress note.** Phase 1 is implemented (macOS-first). The app currently provides: peer registry and per-peer connection health (`tailscale status --json` + `tailscale ping`, direct/DERP/RTT badges, relay/high-RTT warnings), ROM index, config/settings overrides, the macOS/Linux/Windows launchers with spawn/stop/exit detection, a developer loopback pair, the opt-in RetroArch provider (host/client/spectator) with a core+ROM parity gate, and macOS Cabinet mode off by default. Verification: Rust unit tests for parsers/ports/config/launchers/parity plus opt-in live smoke tests (status, ping, ROM scan, real emulator launch and loopback pair); the shell launchers remain the reference. The **rooms/KotH/score-ledger/result-watcher subsystem was removed on 2026-09-21** (see §5). Its replacement **lobby is now the preferred path** (rooms, HTTP beacon discovery, hosting, and the client join flow built; `LobbyCard.tsx` is the home screen, and the direct **Launch match** card is developer-mode only) — see [`09-lobby.md`](09-lobby.md). Phase 0 (RetroArch spectator spike) is verified locally and on the tailnet; Phases 3 and 4 remain, and Phase 0b (the `ggponet.dll` shim) is skipped. Off by default (2026-09-19): macOS Cabinet mode (`src-tauri/src/windowing/`; `cabinet_*` commands, a `cabinetMode` setting, the frontend `MatchView` bezel), verified end-to-end on a live Wine emulator; Windows/Linux hosts and the wider IA redesign remain gated on the redesign grill.

**Phase 0 (RetroArch spectator spike) — live verified, 2026-09-20.** The frozen reference set is pinned (RetroArch 1.22.2; FBNeo cores for macOS/Linux/Windows all at revision `GIT6bb3167`; `sfiii3nr1.zip` sha256/CRC recorded), and a harness (`scripts/retroarch-spike.sh`, `parity|smoke|host|client|spectator|measure|stop`) runs isolated per-role configs without touching the real `retroarch.cfg`. On this Mac: the TorrentZipped ROM loads in the FBNeo core; host + client + 2 spectators connected over loopback with matching content CRC `0x46119843`; the playing client sent ~1.23 KB/s while each spectator sent ~5 B/s (spectators claim no player slot and send no input); and a 2-minute soak showed no desync. RetroArch pauses on focus loss (`pause_nonactive`), which stalls multi-instance loopback tests — the harness disables it. **Live tailnet run (macOS host + Linux client + Windows spectator, app-driven) stayed synced ~11 min at ~55–83 ms after the handshake; the spectator claimed no player slot; feel was rated acceptable**, so §7 row 1 applies: RetroArch is the primary spectator path and the `ggponet.dll` shim (Phase 0b) is skipped. Two concurrent spectators are proven on loopback and their tailnet run is deferred (needs all four machines online). The independently built cores initially differed in FBNeo's `GIT_DATE` token (macOS empty because the upstream Makefile uses GNU `date -d`), triggering RetroArch's soft "different version of the core" warning; the macOS core was rebuilt from the same commit with `GIT_DATE=260918` to match (`docs/07` §2). Full protocol and results in [`07-retroarch-spike.md`](07-retroarch-spike.md).

### Prerequisites (do once per machine)

Rust (via the official installer, not Homebrew):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

The Tauri CLI is **not** installed globally via `cargo install`; it ships as a prebuilt npm package and is a devDependency in the root `package.json` (`@tauri-apps/cli`). The frontend lives in `frontend/` with its own `package.json`; `src-tauri/` is the backend.

```sh
npm install                      # root: Tauri CLI
npm install --prefix frontend    # frontend: React/Vite/Tailwind/shadcn
npm run tauri dev                # dev: Vite on :1420 + Rust backend
npm run tauri build              # production bundle
```

Per-OS system dependencies (not Rust crates; Cargo cannot fetch these):

| OS | Required |
|---|---|
| macOS (this machine) | Xcode Command Line Tools — already installed at `/Library/Developer/CommandLineTools` |
| Windows (native friend) | MSVC C++ Build Tools + WebView2 runtime (ships with Win10/11); NSIS/WiX only for installers |
| Linux (CachyOS friend) | `libwebkit2gtk-4.1-dev`, `build-essential`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` |

Cross-compiling Tauri across OSes is painful; each of the four machines builds its own binary. Test an early build on the CachyOS machine first — WebKitGTK is the least predictable webview.

## 7. Open questions

- ~~Does `bVidSaveOverlayFiles 1` actually write `winner.txt`/scores in **direct** mode, or does it need FightCade match metadata?~~ **Moot — the overlay watcher was removed 2026-09-21** (§5). Kept as a record: it did write them in direct mode with no FightCade metadata (`game.txt`, `p1name.txt`/`p2name.txt`, `p1character.txt`/`p2character.txt`, `p1score.txt`/`p2score.txt`, `winner.txt`), and the files updated live mid-session. The app no longer reads any of this.
- `quark:stream` argument semantics: is the middle token a host IP or a server session id?
- Is RetroArch FBNeo netplay feel acceptable for fighting games at <100 ms? **Answered 2026-09-20:** mechanics proven locally and confirmed live on the tailnet (macOS host + Linux client + Windows spectator, ~11 min synced, ~55–83 ms); feel rated **acceptable**, so §7 row 1 applies — RetroArch primary for the spectator path. Only 2-concurrent-spectators on the tailnet remains deferred. See [`07-retroarch-spike.md`](07-retroarch-spike.md).
- Multi-spectator fan-out ceiling and whether each spectator needs its own inbound UDP port / firewall rule.
- Connection-health UX: poll cadence for `tailscale status --json` / `tailscale ping` (idle vs. in-match), RTT warn threshold (group plays direct at <100 ms — warn above ~150 ms?), and Windows/macOS CLI output parity for parsing.
- Can the wrapper detect mid-match degradation, or only pre-match? (GGPO stats need the shim; tailnet re-ping mid-match is coarse.)
- Emotes/voice gate: what survives /grill-me? Open: emote surface (lobby-only vs. in-match overlay), voice transport (WebRTC over tailnet vs. "just use Discord"), push-to-talk vs. open mic, per-session consent UX.
- Input recording / match history gate: what survives /grill-me? Open: reuse emulator replay files (FightCade `.fr` / `quark:replay`, RetroArch replay) vs. capture raw inputs; where recordings live and who holds them; determinism/version pinning (core + ROM CRC) so replays stay playable; storage growth and retention; recording consent and visibility (is there a "recording" indicator?). **Settled 2026-09-21 ([`09-lobby.md`](09-lobby.md)):** match history is a local per-machine set/round index; replays use RetroArch's native `.replay`, deferred until spike item S8 proves recording during netplay.
- Lobby gate: does [`10-lobby-spike.md`](10-lobby-spike.md) S1–S7 pass? Open: beacon transport/port; the exact slot hand-off sequence (spike S2/S3); where `sfiii3nr1`'s round counters live (spike S5); whether a designated always-on "table" machine is wanted; whether the lobby ships before or after Phase 4.
- Redesign / Cabinet mode gate (see [`06-redesign.md`](06-redesign.md)): does the macOS Accessibility grant survive an ad-hoc-signed update, or does frame-follow become the default? Does the macOS emulator window stay above the bezel while remaining clickable? What is the target single-window information architecture? Is the FightCade dependency worth removing via a RetroArch migration? Does the redesign delay Phase 3?
- Training-mode gate (research in [`11-training-mode.md`](11-training-mode.md)): **two paths.** *Path A (FightCade)* — Fightcade's training is the community **peon2/fbneo-training-mode** Lua script (bundled, launched via the `/**training` client command / `quark:training`), and **3rd_training_lua** is a richer 3rd-Strike-only alternative; both are offline-only FBNeo Lua scripts. Open: the exact `quark:training`/`.lua` command-line token order; whether the scripts run unmodified on `sfiii3nr1`; and **licensing** — neither repo declares a license, so neither can be bundled in this public MIT repo. *Path B (RetroArch, added 2026-09-22)* — the core cannot run the Lua scripts (no Lua engine, no input injection), but RetroArch 1.22.2 exposes `WRITE_CORE_RAM`/`WRITE_CORE_MEMORY` and replay controls over the command socket the app already speaks, so the Cabinet can implement the peon2-equivalent aids itself (infinite timer, health/meter refill) against the lobby-validated `sfiii3nr1` addresses — no third-party assets, so **Path B sidesteps the Path A licensing gate** while giving up scripted dummies/hitboxes/frame data. A Cabinet "Practice" launch (either path) is gated on a /grill-me session and the spikes in `11` §8.
- macOS: signature/quarantine handling after replacing `ggponet.dll`; behavior when FightCade auto-updates.
- **Builds/distribution (resolved 2026-09-19).** Chosen route: **GitHub Actions per-OS build matrix → GitHub Releases** (see §3). Each OS builds its own binary; friends download the release installer instead of cloning, installing OS/Rust/JS deps, and building natively — the path that derailed the first Linux attempt with stale package DBs / missing toolchains. The repo was made **public** after the §8 publication scrub (completed 2026-09-19), so release-asset downloads are anonymous (no GitHub account or collaborator access needed) and private-repo Actions minute caps no longer apply. The update mechanism is in-app (`tauri-plugin-updater` over the signed GitHub Releases feed, implemented 2026-09-24 — see §3); a plain re-download is only the fallback for a first install and for Linux `.deb`. **Implemented 2026-09-19:** `.github/workflows/release.yml` builds each OS natively and uploads the bundles to a draft GitHub Release via `tauri-action`, and `.github/workflows/ci.yml` gates every push/PR on the 3-OS test/clippy matrix. Tag `v0.1.0` to cut the first release. The pre-rename `v0.1.0` assets are `cabinet_*` (`com.cabinet.app`); `v0.1.1` cuts the renamed app (`The Cabinet`, `com.the-cabinet.app`), and the config migration in `lib.rs` carries the old identifier forward. To reverse the earlier native Linux build, `scripts/uninstall-linux.sh` reports (dry-run) or removes the extracted repo, the pacman packages the setup script added (reconstructed from `/var/log/pacman.log`, `flatpak` always preserved), the Rust toolchain when it postdates the run, and the Tauri/npm caches; `--install-appimage` moves the machine onto the release AppImage. **Frozen RetroArch cores (2026-09-21)** ship separately as assets of a **published** release `retroarch-cores-v1` (tag-only, no app installers) uploaded by `scripts/publish-cores.sh`; the app downloads + sha256-verifies them in Settings → RetroArch. That release is published (2026-09-21).
- RetroArch path: ROM/core parity and content-CRC matching across the four machines, and whether FBNeo core serialization is enabled. **Answered 2026-09-20** ([`07-retroarch-spike.md`](07-retroarch-spike.md)): the FBNeo core's `.info` declares `savestate = deterministic`; the frozen macOS/Linux/Windows cores embed the same revision (`GIT6bb3167`); netplay matched content CRC `0x46119843` across all instances; the TorrentZipped `sfiii3nr1.zip` loads; and a live cross-OS session synced with no desync. One cross-build wrinkle: the cores differed in FBNeo's `GIT_DATE` token (macOS empty due to upstream's GNU-only `date -d`), which made RetroArch warn softly on the version string; the macOS core was rebuilt from the same commit with `GIT_DATE=260918` to match (`07` §2), and the app's parity gate now covers the rebuilt sha.
- **Dependency maintenance (deferred, out of scope as of 2026-09-19).** Dependabot is configured (`.github/dependabot.yml`) for weekly npm (root + frontend), cargo, and GitHub Actions updates, and it raises security alerts. Its output is **not** being actioned for now: the open bump PRs (Actions group, TypeScript 6 → 7 in `frontend`) and the medium `glib` advisory in `src-tauri/Cargo.lock` (a transitive Tauri/GTK dependency; unsoundness in `VariantStrIter`) are left as-is. Revisit when an advisory affects a shipped build or a bump is needed for a feature. Merging any of these still has to pass the 3-OS `ci.yml` gate.

## 8. Risks and tradeoffs

- A `ggponet.dll` shim is tied to FightCade 2.1.45 and will break on update; must pin and disable auto-update.
- Modifying a signed macOS `.app` requires re-signing; the Linux Flatpak `/app` is read-only (needs a writable copy).
- RetroArch's TCP/replay netplay may feel worse than FightCade's UDP/GGPO for fighting games.
- The 8 GB M1 dev harness (Chrome + editor + multiple emulator instances) may swap; use native/isolated instances.
- Tailscale ACLs + per-OS firewall rules are prerequisites for spectating.
- `tailscale status --json` format is version-dependent (Tailscale warns it may change between releases); pin a minimum Tailscale version and parse defensively.
- Voice is a subsystem, not a feature (device selection, echo, consent UX); mic defaults off with a visible live indicator is mandatory. Scope risk stays until the /grill-me gate is passed.
- Input recordings are only replayable against the exact core + ROM build; a core/ROM update silently invalidates history. Storage and retention are unbounded unless capped, and recordings capture player behavior (consent/visibility needed). Scope risk stays until the /grill-me gate is passed.
- Tailscale Personal free tier allows up to 6 users (unlimited devices) — the 4-person group is within limits.
- Lobby (core built, sets/rotation pending — [`09-lobby.md`](09-lobby.md)): per-ROM RAM result detection is per-game and version-sensitive (core/ROM updates can break it); host-as-spectator is the load-bearing unknown and reverses prior non-goal 07-F18; the control plane is RetroArch's unauthenticated local command socket; no host migration (single point of failure per session); beacon adds a listening socket + per-OS firewall surface. All gated on [`10-lobby-spike.md`](10-lobby-spike.md).

### Publication scrub (completed 2026-09-19)

The repository was made public on 2026-09-19 after a full-history scrub. The real values below were replaced with placeholders in **all files and in git history** (via `git-filter-repo`, with commit authors rewritten to the GitHub noreply identity), and the GitHub repository was deleted and recreated so no pre-scrub objects survive:

- Tailscale addresses/hostnames: three `100.64.0.x` addresses, `mac-host`, `cachyos-host`, `windows-host`
- Tailnet MagicDNS suffix: `example-tailnet.ts.net`
- Public IPs: `203.0.113.10`, `203.0.113.11`, and the first public traceroute hop `203.0.113.12`
- ISP private hops: `198.51.100.2`–`198.51.100.4`
- LAN IPs and router: `192.0.2.100`, `192.0.2.101`, `192.0.2.1`
- Account handles: `me@`, `friend@`
- ROM short names: kept (`sfiii3nr1` is a public MAME identifier)

Going forward, use only the sanctioned placeholders listed in [`AGENTS.md`](../AGENTS.md) (`100.64.0.1`–`100.64.0.3`, `192.0.2.1`/`.100`/`.101`, `198.51.100.2`–`.4`, `203.0.113.x`, `example-tailnet.ts.net`, the `*-host` aliases, `player-one`), and keep `*.local` and `.env*` in `.gitignore`.

## 9. Reference links

- Community direct-connect launcher: <https://github.com/nitsuboy/fightcade-fbneo-lan> (MIT)
- Upstream GGPO SDK (spectator API reference): <https://github.com/pond3r/ggpo>
- RetroArch netplay (user): <https://www.retroarch.com/netplay.php>
- RetroArch netplay protocol (developer): <https://docs.libretro.com/development/retroarch/netplay/>
- Native macOS client (evaluate later): <https://github.com/Jayian1890/Macade>
