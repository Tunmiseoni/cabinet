# RetroArch netplay/spectator spike (Phase 0)

Status: **live verified on the tailnet (2026-09-20) — feel rated acceptable; decision matrix §7
row 1 is met.** Mechanics (core/ROM parity, content loading, netplay host/client/spectator,
spectator no-input, desync-free soak) were first proven on one machine over loopback, then
confirmed live cross-OS: a macOS host (P1) + Linux client (P2) + a Windows spectator connected,
the spectator claimed no player slot, and the session ran ~11 min with no desync, at ~55–83 ms
after the handshake spike.

The one criterion not exercised live is **2 concurrent spectators** (the live runs had one
watcher at a time); the 4-instance loopback soak in §3 covers that topology. A 2-spectator
tailnet run is deferred (needs all four machines online), not blocked.

This is the Phase 0 gate in [`04-design.md`](04-design.md) §6, and the evidence for the
FightCade-dependency question in [`06-redesign.md`](06-redesign.md) §2 (whether the RetroArch
netplay route can replace the FightCade install entirely).

> Not to be confused with the redesign's **R0 window-topology spike** (`06-redesign.md` §3.5).

---

## 1. Goal and exit criteria

**Goal:** decide whether to make RetroArch's netplay/spectating the primary input-relay path
(free, native, cross-platform, no Wine, no `ggponet.dll` shim) or fall back.

| # | Criterion | How it is judged |
|---|---|---|
| 1 | Host + 1 client + **2 concurrent spectators** connect and stay synced | Host log shows `player 2`; both spectators connect without a player slot; no desync over a match |
| 2 | "Spectators send no input" | Per-process outbound bytes for a spectator ≈ 0 while a player's is ~1.2 KB/s |
| 3 | Low bandwidth | Measured per-stream bytes match the ~1–2 KB/s/player estimate |
| 4 | **Acceptable feel at <100 ms** | Two humans rate it against FightCade/GGPO over the tailnet |

1–3 are proven locally (§3). 4 is the gate and requires the friends.

## 2. Frozen reference set (parity)

RetroArch netplay refuses to sync unless **frontend version, core version, and content CRC**
match. The set below is frozen; do not let any machine auto-update its core.

| Item | Value |
|---|---|
| Frontend | **RetroArch 1.22.2** (macOS build git `69a4f0ea`) |
| Core revision (all OSes) | **`GIT6bb3167`** — FBNeo reports `v1.0.0.03 260918 GIT6bb3167` (see the macOS rebuild note below) |
| Content CRC (netplay) | `0x46119843` (the `sfiii3nr1.zip` file) |
| ROM | `sfiii3nr1.zip`, sha256 `4ed142c90fc1a4632d20600d2f5b46caac813422caab62cce3fa9f85ee5cc4dc`, CRC32 `26199456`, 70,724,519 bytes, TorrentZipped |

Core sha256 (freeze these exact files — the buildbot `latest` channel is rolling):

| OS | File | sha256 |
|---|---|---|
| macOS arm64 | `fbneo_libretro.dylib` | `6472c6312fe6ad49a8001efabbdc4d2b542848a7c964d0a082cd736e01e8a4eb` |
| Linux x86_64 | `fbneo_libretro.so` | `a154a08d0f97ff1c66e6ec22a5c209854f31eb2ed7d831b2cb4c0101d48a2448` |
| Windows x86_64 | `fbneo_libretro.dll` | `0a92f3b61dba68b34df0a93afe1b24b98debb3c25dfe63bf21c02034fdfa1179` |

> **The macOS core is a local rebuild (2026-09-20).** FBNeo builds its `library_version`
> from `GIT_DATE` + `GIT_VERSION`, and `Makefile.common` computes `GIT_DATE` with GNU-only
> `date -d @epoch`, which fails on macOS. The official buildbot mac core therefore reported
> `v1.0.0.03  GIT6bb3167` (empty date) while Linux/Windows reported `v1.0.0.03 260918
> GIT6bb3167`; RetroArch warns (softly) whenever those strings differ. The mac core was rebuilt
> from the **same** commit (`6bb3167a044e19e7106a5110d5531aa9c6afa96f`) with
> `GIT_VERSION=6bb3167 GIT_DATE=260918`, so all three OSes now report one string. It lives in
> the app-managed core dir (`<app_data_dir>/cores/macos-arm64/fbneo_libretro.dylib`); the
> sha256 above is the rebuilt file (the pre-rebuild buildbot file was `674e76fb…`). Rebuild:
> `git clone https://github.com/libretro/fbneo && git checkout 6bb3167a…`, then
> `make -C src/burner/libretro platform=osx GIT_VERSION=6bb3167 GIT_DATE=260918`.

RetroArch 1.22.2 downloads (verified paths):

- macOS: `buildbot.libretro.com/stable/1.22.2/apple/osx/universal/RetroArch_Metal.dmg`
- Windows: `buildbot.libretro.com/stable/1.22.2/windows/x86_64/RetroArch-Win64-setup.exe`
- Linux: `buildbot.libretro.com/stable/1.22.2/linux/x86_64/RetroArch.7z`

Cores: `buildbot.libretro.com/nightly/<os>/<arch>/latest/fbneo_libretro.<ext>.zip` (grab once,
verify the sha256 above, then keep the extracted file).

Verify a machine with:

```sh
scripts/retroarch-spike.sh parity /path/to/parity-manifest.txt
```

## 3. Local verification (loopback, 2026-09-20)

Four instances on one M1, all over `127.0.0.1`, using the frozen set:

| Check | Result |
|---|---|
| TorrentZipped ROM loads in FBNeo | ✅ `Romset found … sfiii3nr1`; no errors; `FBNeo Running v1.0.0.03 GIT6bb3167` |
| Core/content CRC identical across instances | ✅ `[Content] CRC32: 0x46119843` on all four |
| Host + client + 2 spectators connect | ✅ host: `spike-p2 has joined as player 2 (ping: 31 ms)`; two `Got connection from: "spike-spec"/"spike-spec2"` |
| Spectators claim no player slot | ✅ **no** `joined as player 3`; spectator logs show only `Connected to: "spike-host"` |
| Spectators send no input | ✅ spectators out **~5 B/s**; client out **~1.23 KB/s** |
| Sustained sync | ✅ ~2+ minutes of continuous exchange, no desync / CRC failure / disconnect |

**Common pitfall found:** RetroArch pauses when its window loses focus (`pause_nonactive`), so
with several instances on one desktop the host pauses and the session stalls at the handshake.
The harness sets `pause_nonactive = "false"`. (This is a test-harness issue, not a real-world
one — remote players each have focus.)

### Measured bandwidth (2 players + 2 spectators)

| Role | In | Out |
|---|---|---|
| Host (P1) | ~1.24 KB/s | ~6.10 KB/s |
| Client (P2) | ~1.23 KB/s | ~1.23 KB/s |
| Spectator ×2 | ~2.45 KB/s each | **~5 B/s** |

Derived: one player's input stream ≈ **1.23 KB/s (~20 B/frame at 59.6 fps)**; a spectator
receives both players' streams (≈2.45 KB/s); the host's egress is the sum of per-client
forwards (≈6.1 KB/s for this topology). This is input-relay, not video — ~1000× less.

CPU ≈ **15–24%** and RSS ≈ **150–270 MB** per instance on the M1 (rendering-dominated; a
spectator renders the game locally, which dominates its cost). Host CPU stays modest.

## 4. Harness

`scripts/retroarch-spike.sh` (macOS/Linux bash; Windows friends use the commands in §5). It
writes isolated per-role configs and logs under `$RETROARCH_SPIKE_DIR` and never touches the
real `retroarch.cfg`. It forces `TAILSCALE_BE_CLI=1` on macOS so status/ping calls do not spawn
menu-bar icons.

```
scripts/retroarch-spike.sh parity [manifest]
scripts/retroarch-spike.sh smoke [frames]
scripts/retroarch-spike.sh host [port]
scripts/retroarch-spike.sh client <host-ip> [port]
scripts/retroarch-spike.sh spectator <host-ip> [port]
scripts/retroarch-spike.sh spectator2 <host-ip> [port]
scripts/retroarch-spike.sh measure [seconds]
scripts/retroarch-spike.sh status
scripts/retroarch-spike.sh stop
```

## 5. Per-role commands and machine prep

Every machine needs: RetroArch 1.22.2, the frozen FBNeo core for its OS, and the byte-identical
ROM. A host (only) must accept inbound **TCP 55435**.

An overrides file adds the spike settings on top of the normal config (keep NAT traversal and
public announce off; we are on the tailnet, not the public lobby):

```ini
config_save_on_exit = "false"
pause_nonactive = "false"
netplay_nat_traversal = "false"
netplay_public_announce = "false"
netplay_allow_slaves = "true"
netplay_require_slaves = "false"
netplay_check_frames = "600"
netplay_ping_show = "true"
netplay_nickname = "<your-nick>"
# spectators only:
netplay_start_as_spectator = "true"
```

**macOS / Linux** — use the harness (`host`, `client`, `spectator`).

**Windows** (cmd/powershell), from the RetroArch directory:

```bat
:: host (player 1)
retroarch.exe -L cores\fbneo_libretro.dll roms\sfiii3nr1.zip --appendconfig overrides-host.cfg -H --port 55435 --nick <nick>
:: client (player 2)
retroarch.exe -L cores\fbneo_libretro.dll roms\sfiii3nr1.zip --appendconfig overrides-client.cfg -C 100.x.x.x --port 55435 --nick <nick>
:: spectator
retroarch.exe -L cores\fbneo_libretro.dll roms\sfiii3nr1.zip --appendconfig overrides-spec.cfg -C 100.x.x.x --port 55435 --nick <nick>
```

Firewall (host only): allow inbound TCP 55435 for `retroarch.exe` (Windows: `netsh advfirewall
firewall add rule name="RetroArch Netplay" dir=in action=allow program="<path>\retroarch.exe"
protocol=TCP localport=55435`; macOS: allow RetroArch when prompted, or add it to the firewall
allow list).

## 6. Live 4-person test protocol

1. **Parity first.** Every machine runs `retroarch-spike parity` against the manifest and must
   print `PARITY OK`. Fix any mismatch before starting.
2. Agree on host (a player = P1) and one challenger (P2); the other two are spectators.
3. Start order: host → client → spectators. Host is a player, so `-H` (player 1).
4. **Confirm the topology**: host log shows `player 2`; the two spectator logs show only
   `Connected to`. If a spectator logs `joined as player 3`, `netplay_start_as_spectator` did
   not take.
5. Play several full matches. Watch for desync; the netplay ping OSD (`netplay_ping_show`)
   shows the connection RTT in-game.
6. **Measure:** on the host, sample `nettop` for the RetroArch PID and/or
   `tailscale status --json` `TxBytes`/`RxBytes` deltas; record RTT/path with
   `tailscale ping <peer>` (expect direct <100 ms on the current tailnet).
7. **Feel survey:** both players rate input feel (e.g. 1–5) and whether rollback artifacts are
   noticeable, versus their FightCade/GGPO experience. Spectators note whether watching is
   smooth enough.

## 7. Decision

| Outcome | Decision |
|---|---|
| Feel acceptable at <100 ms, 2 spectators stable | **RetroArch primary** for the spectator path; skip the `ggponet.dll` shim; proceed to Phase 3 integration and revisit the RetroArch migration in `06-redesign.md` §2 |
| Feel marginal | Try tuning first (input latency frames, `netplay_max_ping`); then a hard-time-boxed shim (Phase 0b), else defer |
| Feel unacceptable | Keep `quark:direct`/GGPO; document spectate as deferred (or pursue the FightCade `ggponet.dll` shim) |

**Chosen (2026-09-20): row 1.** Live feel was acceptable and the cross-OS session stayed synced;
the 2-concurrent-spectator half of the criterion is proven on loopback and its tailnet run is
deferred. So: take RetroArch as the **primary spectator path**, skip the `ggponet.dll` shim
(Phase 0b), and revisit the RetroArch migration in `06-redesign.md` §2.

## 8. Friend-machine prep (one-time)

1. Install RetroArch **1.22.2** for the OS (URLs in §2).
2. Replace/place the FBNeo core with the **frozen** file for that OS (sha256 in §2). Do not use
   the Core Downloader afterwards.
3. Put `sfiii3nr1.zip` where RetroArch can load it and confirm its sha256 matches. If the
   FightCade copy differs, copy the reference file instead.
4. Host only: allow inbound **TCP 55435**.
5. Run `parity` and confirm `PARITY OK`.

**Distribution of the frozen cores is still an open decision** (GitHub pre-release assets on the
public repo vs. manual copy). Nothing has been published yet.

## 9. Remaining unknowns

- ~~**Real feel at <100 ms** — untested until the live run.~~ **Answered (2026-09-20):** the
  live tailnet run was rated acceptable; §7 row 1 applies.
- ~~**Cross-OS parity in practice.**~~ **Answered (2026-09-20):** a macOS host + Linux client +
  Windows spectator synced and played with no desync. One wrinkle: the independently built cores
  differed in FBNeo's build-date token (`v1.0.0.03  GIT6bb3167` on macOS vs `v1.0.0.03 260918
  GIT6bb3167` on Linux/Windows), so RetroArch emitted a soft "different version of the core"
  warning. The macOS core was rebuilt from the same commit with the date forced (§2), so all
  three now report one string.
- **Host-as-spectator** (a non-playing server) was not exercised; only a player-hosts topology.
- **`netplay_spectate_password`** (spectator-only password) was not exercised.
- **Two concurrent spectators on the tailnet** — proven on loopback (§3), deferred live (needs
  all four machines online at once).
- **Spectator audio.** Each spectator runs its own emulator and plays sound; the group may want
  to mute spectators.
- **Core updates.** The buildbot `latest` channel is rolling; the frozen sha256s must be
  re-frozen deliberately, never silently. The macOS core is now a local rebuild (§2), so a core
  bump must be rebuilt the same way on macOS, not re-downloaded.

## 10. Licensing

- The Cabinet is **MIT**.
- RetroArch is **GPLv3**.
- The FBNeo libretro core's `.info` declares `license = "Non-commercial"` (the
  `06-redesign.md` §2 shorthand "FBNeo is open (GPL)" is imprecise for the libretro core).

Implication for the `06-redesign.md` §2 RetroArch-migration route: embedding a GPLv3 frontend and a non-commercial core in
an MIT app is a real packaging/compliance task (source offer, license files, no commercial
sale), not a drop-in. The spike's harness uses stock RetroArch, so none of this applies yet.

## 11. Publication scrub reminder

The repo is public. Use placeholders only: `100.x.x.x`, `203.0.113.x`, `example-tailnet.ts.net`.
Never commit ROMs, cores, or emulator binaries. The frozen cores live outside the repo; the
`parity-manifest` may be committed (hashes and URLs only).

## 12. References

- [`04-design.md`](04-design.md) §2/§6 — RetroArch rationale and the Phase 0 gate.
- [`06-redesign.md`](06-redesign.md) §2 — the FightCade dependency and the RetroArch migration.
- [`scripts/retroarch-spike.sh`](../scripts/retroarch-spike.sh) — the harness.
- RetroArch netplay (user): <https://www.retroarch.com/netplay.php>
- RetroArch netplay protocol: <https://docs.libretro.com/development/retroarch/netplay/>
- Multi-controller / Request Device: <https://docs.libretro.com/guides/netplay-multiple-controllers/>

## 13. Results log

### 2026-09-20 — local loopback (this machine, macOS arm64, macOS 26.6.2)

- Frozen cores downloaded and hashed; all three report `GIT6bb3167`.
- Smoke: `sfiii3nr1.zip` (TorrentZipped) loads; FBNeo `v1.0.0.03 GIT6bb3167`; no errors; 600
  frames clean.
- Loopback 2P+2S: spectator out ≈ 5 B/s vs client out ≈ 1.23 KB/s; no desync over ~2 min.
- Raw evidence captured to `$RETROARCH_SPIKE_DIR/results-loopback.txt` (outside the repo).

### 2026-09-20 — live tailnet (macOS + Linux + Windows)

Driven by the app (`provider=RetroArch`), one machine per OS. Evidence is the app log plus the
per-role `emulator-<role>.log` captures.

- **Topology:** macOS **host (P1)** + Linux **client (P2)** + Windows **spectator** on the last
  session; earlier rounds rotated the host (macOS/Windows) with a spectator from another OS.
  Every role connected and stayed up.
- **RTT:** the first join pinballed (~266–309 ms), then settled to **~55–83 ms**; preflight TCP
  RTT was ~39–80 ms, direct path.
- **Spectator claims no slot:** the watcher logged `Connected to: <host>` only; the host logged
  `<client> has joined as player 2`, never `player 3`.
- **Stability:** the final session ran **~11 minutes** and stayed synced — no desync, content-CRC
  failure, or timeout lines across any emulator log.
- **Feel: acceptable** — both players rated it usable; this is §7 row 1 (RetroArch primary for
  the spectator path, skip the `ggponet` shim).
- **Found and fixed:** every cross-OS join logged `WARNING: A netplay peer is running a
  different version of the core`. Cause: FBNeo computes `GIT_DATE` with GNU-only `date -d`, so
  the official macOS core shipped an empty date while Linux/Windows had `260918`; RetroArch's
  handshake compares the full `library_version` and warns (softly — not a failure). Rebuilt the
  macOS core from the same commit with `GIT_DATE=260918` (§2); it now reports
  `v1.0.0.03 260918 GIT6bb3167`, matching the others. The app's parity gate passed before and
  after.
- **Not exercised live:** 2 concurrent spectators (one watcher at a time; loopback covers it),
  host-as-spectator, `netplay_spectate_password`, and per-role byte accounting over the tailnet.
- **Logs:** kept outside the repo (app log dir + `sessions/<utc>/emulator-<role>.log`).

### 2026-09-20 — app provider slice (macOS, this machine)

RetroArch is now a selectable provider in the app (Settings → Match provider; default FightCade).
The app generates the per-role appendconfig under its config dir, launches host/client/spectator
from `InstanceState` roles, skips the overlay/result watcher for RetroArch, and gates launch on a
frozen-set parity check (core `GIT` + sha256, ROM sha256).

- Unit: exact `-L … --appendconfig … [-H | -C ip] --port … --nick …` argv for all three roles;
  capabilities; parity pass/fail. `scripts/test.sh` green.
- Windowing: a native RetroArch window is matched by spawned PID and placed
  (`240,87 897×700 → 120,120 720×480`, read back exactly) — opt-in
  `windowing::macos::tests::live_places_retroarch_window`. A single-window Accessibility fallback
  was added because RetroArch's window can briefly report CG/AX bounds that disagree (and its title
  is hidden without Screen Recording permission); the same fallback also un-flaked the FightCade
  placement test. Cabinet mode is therefore viable for RetroArch; frame-follow remains the fallback
  when Accessibility is denied.
- Core resolution: when `retroarchCore` is unset, the provider now probes, in order, the
  `retroarchPath` sibling `cores/`, the platform-standard RetroArch core dirs (macOS
  `~/Library/Application Support/RetroArch/cores`, Linux `~/.config/retroarch/cores` including the
  Flatpak path, Windows `C:\RetroArch-Win64\cores`), and only then the app-data managed path — so an
  installed frozen core is found without a Settings entry.
- Not yet verified in-app: managed core download (release unpublished). Cross-OS parity and the
  live input-feel run were verified 2026-09-20 (see the live tailnet entry below); the macOS
  core is the rebuilt file from §2.

### 2026-09-20 — in-app diagnostics (netplay failures were undiagnosable)

The first real in-app three-machine test surfaced two symptoms with no usable evidence trail: the
host and client sat in the FBNeo attract screen ("insert coin") without connecting, and a
spectator showed RetroArch's **"Failed to initialize netplay."** (a `netplay_new()` failure, i.e.
the client could not reach the host). On a Finder-launched `.app`, RetroArch's stderr and the
app's own `eprintln!` go nowhere retrievable.

Added so the next run is diagnosable:

- **App log file** via `tauri-plugin-log` in `app_log_dir` (macOS `~/Library/Logs/com.the-cabinet.app`,
  Linux `~/.local/share/com.the-cabinet.app/logs`, Windows `%LOCALAPPDATA%\com.the-cabinet.app\logs`):
  Info by default, `verboseLogging` (Settings) raises to Debug after restart, 5 MB rotation keep 3.
- **Full emulator capture**: every role's stdout+stderr is piped to
  `app_log_dir/sessions/<utc>/emulator-<role>.log` (all providers); RetroArch also runs with
  `--verbose`. Its generated appendconfig is dumped at debug level.
- **Blocking preflight**: non-dev RetroArch clients/spectators probe `peer:55435` (TCP, 3 s) first;
  unreachable ⇒ launch refused with the error in the dialog (override via "Launch anyway"). Dev
  pairs wait for the loopback host to accept before spawning the client, closing the start race.
- **Diagnostics bundle**: Settings → Diagnostics → *Open logs folder* / *Collect diagnostics*
  (version, config, provider, tailnet summary, last match, session index, last 200 log lines).

Re-run 2026-09-20: the captured logs confirm the host is listening and clients/spectators connect
(see the live tailnet entry above). Remaining nit: hosts logged repeated `Failed to connect to
client` / join-leave churn around retries and late spectators — noisy but not fatal (the session
stayed synced); the start-race fix holds for the app-driven launches.

## 14. Findings and follow-up tasks from the live logs

Consolidated from the 2026-09-20 live run's app + per-role emulator logs (kept outside the repo;
see §11). The run *worked* — a macOS host, Linux client, and Windows spectator stayed synced ~11
min — but the logs surfaced the items below.

### Fixed

- **Cross-OS core-version warning.** Every cross-OS join logged `[Netplay] WARNING: A netplay peer
  is running a different version of the core`. Root cause: FBNeo's `Makefile.common` computes
  `GIT_DATE` with GNU-only `date -d @epoch`, so the official macOS core carried an empty date while
  Linux/Windows had `260918`; RetroArch's handshake compares the full `library_version` and warns
  (soft — not a failure). Fixed by rebuilding the macOS core from the same commit with
  `GIT_DATE=260918` and re-freezing it (§2). The app's parity gate passed before and after.

### Derived tasks

| # | Finding (evidence) | Task | Priority |
|---|---|---|---|
| F1 | Host logs `Failed to connect to client.` + `A netplay client has disconnected` twice per join (32× each; Windows + macOS hosts); never blocked play. The string is not in `network/netplay/netplay_frontend.c`. | Confirm it is expected with `netplay_nat_traversal=false` (host reverse-probe), then document it and/or drop it from captured logs. | Med |
| F2 | `[FBNeo] Unknown device type for port 0/1, forcing "Classic" instead` — 18–36× per session on all OSes; the appendconfig (`retroarch::write_overrides`) sets no `input_libretro_device_p*`. | Pin the libretro input device (6-button Classic) so mappings are deterministic and the warning stops. | High |
| F3 | Force-launching a client before the host binds produces `Failed to set up netplay sockets` / `Failed to initialize netplay` and exits in 6–9 s (linux `205008`/`205023`). | Improve the force path: wait/retry for the host, or clearly label a force-launch as likely to fail. | Med |
| F4 | `--verbose` was always passed (`retroarch::spec`); `logging::capture_stream` wrote untimestamped, uncapped lines — spectator logs ran 600+ lines, dominated by Metal `mvk-warn` noise. | Gate `--verbose` behind `verboseLogging`; timestamp captured lines. | High — **fixed 2026-09-21 (cleanup)** |
| F5 | Session dirs were UTC (`time::utc_stamp`) while the app log timestamped locally. | Unify timestamps or record the offset so app and emulator events correlate without arithmetic. | Low — **fixed 2026-09-21 (cleanup): app log, capture, and session dirs are all UTC** |
| F6 | The app logs only spawn/exit — connection status, player slot, and ping live only inside the captured log. | Parse the captured netplay lines (`Connected to`, `joined as player N (ping X)`, `Netplay disconnected`) into `MatchState` and show netplay health in the UI. | High |
| F7 | Nickname falls back to the literal `"player"` (`RetroArchProvider::new`) when `retroarchNickname`/`handle` are unset — the macOS machine showed as `player` beside peers' handles. | Default to the user's handle / tailnet hostname and prompt once. | Med |
| F8 | Appendconfig sets no `netplay_max_ping` (`retroarch::write_overrides`); first-join pings reached 266–309 ms before settling. | Decide on a `netplay_max_ping` cap (and whether `netplay_spectate_password` is wanted) and apply/document it. | Low |
| F9 | `scripts/fcade-lan-windows-firewall.bat` adds a rule only for `fcadefbneo.exe` with no port; no inbound TCP 55435 rule for `retroarch.exe`, though §5 requires one for a host. | Add/document a Windows firewall rule for RetroArch's netplay TCP port. | High |
| F10 | The app launches RetroArch without `-c`, so it inherits the user's real `retroarch.cfg`; logs show `[GLSL] Stock GLSL shaders will be used` ×14, `[GL] none shader…` ×3, and playlist/`App Intents` scanning. | Decide whether to pass a minimal generated base config so sessions are reproducible and per-machine config doesn't leak in. | Med |
| F11 | Live runs had one spectator at a time; the 2-concurrent-spectator criterion is proven only on loopback. | Run a 2-spectator tailnet session (deferred — needs all four machines online). | Deferred |
| F12 | The untracked handoffs (`handoff-cabinet-mode-2026-09-20.md`, `handoff-retroarch-provider-2026-09-20.md`) still describe pre-live state. | Refresh or delete them so they don't mislead. | Low — **Done 2026-09-21 (cleanup): both handoffs deleted** |
| F13 | ~~Rooms/KotH bypass the provider seam: `service.rs:139` calls `commands::resolve_launcher` (always FightCade) and always reads overlay results.~~ | ~~Gate room hosting/joining on `overlay_results`…~~ **Moot — the rooms/KotH/overlay subsystem was removed 2026-09-21** ([`04-design.md`](04-design.md) §5). | Dropped |
| F14 | The diagnostics bundle (`diagnostics.rs`) lists session dirs/files with sizes and tails the app log, but omitted the per-session `emulator-*.log` contents — exactly where the netplay lines live. | Include the latest session's emulator-log tail in the bundle. | High — **fixed 2026-09-21 (cleanup)** |
| F15 | The diagnostics bundle embeds tailnet IPs and absolute home/config paths (`config`, provider detail, last match) with no redaction or warning. | Redact identifiers or warn that the bundle is not for public sharing. | Med — **fixed 2026-09-21 (cleanup): home paths + IPv4 redacted, warning header added** |
| F16 | Session dirs and `emulator-*.log` were never pruned; only the app log rotates. | Add retention (count/age/size cap). | Med — **fixed 2026-09-21 (cleanup): session dirs pruned to the newest `SESSION_KEEP`** |
| F17 | Each spectator renders its own audio locally, which echoes in a voice call; the task list doesn't address it (§9). | Default spectators to muted, or add a setting. | Med |
| F18 | Host-as-spectator (non-playing server) was not exercised; the app exposes only P1/P2/Spectator. | Decide whether to support and test it. | Low |
| F19 | A client issued `NETPLAY_CMD_LOAD_SAVESTATE`, followed by `Failed to load state` against the empty per-role savestate dirs. | Confirm netplay state sync cannot pull a stale/wrong state. | Low |
| F20 | Per-machine `retroarchPort` isn't validated against peers; preflight only proves TCP reachability. | Validate the expected netplay port or document "host's port must match on all peers". | Low |

### Confirmed benign

- Spectator claimed no player slot (`player 3` never appears); content CRC `0x46119843` matched
  across peers; all emulator exits were code 0; no desync, CRC failure, or timeout lines.
