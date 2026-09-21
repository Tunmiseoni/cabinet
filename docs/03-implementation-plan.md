# Implementation plan — direct-connect FBNeo over Tailscale (free)

Goal: play FightCade's FBNeo games with the friend `cachyos-host` by connecting the emulators directly over Tailscale, bypassing FightCade matchmaking and therefore the ISP CGNAT.

**Status:** launchers implemented and **verified end-to-end 2026-09-18** — a group played matches over the direct tailnet path (<100 ms). macOS launcher at `~/bin/fcade-lan` (plus `~/Desktop/FightCade LAN.command`), Linux launcher at `scripts/fcade-lan-linux.sh`, Windows launcher at `scripts/fcade-lan-windows.bat`. Spectating is **not** available on this route; see [`04-design.md`](04-design.md) for the current plan.

## Evidence that the mechanism works (verified on this Mac)

| Check | Result |
|---|---|
| `quark:direct` parser exists in the binary | ✅ `strings fcadefbneo.exe` → `quark:direct,%[^,],%d,%[^,],%d,%d,%d,%d` |
| Argument format matches upstream | ✅ `nitsuboy/fightcade-fbneo-lan/src/process.c` emits the identical string |
| Direct launch opens the emulator (no `fcade` helper) | ✅ ran `wine32on64 fcadefbneo.exe "quark:direct,..." -w`; process stayed alive 20 s+ |
| Correct local port is bound per side | ✅ `lsof` shows UDP `*:7001` for side 0, `*:7000` for side 1 |
| Emulator actually transmits to the configured peer | ✅ fake peer listening on `127.0.0.1:7999` received 10 UDP packets **from `:7001`**, 5-byte GGPO payloads |
| Direct tailnet path works both ways | ✅ after the router restart, `tailscale ping` is direct (<100 ms) and real matches ran |
| End-to-end match over `quark:direct` | ✅ four players played matches on 2026-09-18 |

What this proves: the emulator parses the direct string, binds the right port, and sends real UDP to whatever peer IP/port it is given. If that IP is a Tailscale address, the OS routes it via `utun`, so Tailscale carries the packets.

What is **still not available on this route** (see [`04-design.md`](04-design.md)):
- **Spectating.** FightCade's spectator is server-brokered over a closed TCP protocol and cannot be reached from `quark:direct`. This is the main open design problem; RetroArch netplay or a `ggponet.dll` shim are the candidate solutions.
- Automatic match-result/overlay files in direct mode are **unverified** (`bVidSaveOverlayFiles 1`); they mattered only for the KotH/score tracking that was **removed 2026-09-21** (see [`04-design.md`](04-design.md) §5).

## Path status (updated 2026-09-18) — important

- Both ends are behind **symmetric NAT** (`MappingVariesByDestIP: true`): Mac public `203.0.113.10`, friend public `203.0.113.11`.
- Initially Tailscale had **no direct path** and relayed via DERP `par` (329–514 ms).
- **After a router restart the tailnet established a direct path (<100 ms)**; matches were played over it. The DERP figures are stale.
- DERP (`par`/`lhr`) remains only a fallback if a direct path is lost.
- **`quark:direct` is confirmed working end-to-end.**

| Item | Value |
|---|---|
| FightCade app | `/Applications/FightCade2.app` |
| Wine binary | `Contents/Resources/wine/bin/wine32on64` |
| Wine prefix | `Contents/Resources/.wine32` |
| Emulator | `Contents/MacOS/emulator/fbneo/fcadefbneo.exe` |
| Emulator cwd (required) | `Contents/MacOS/emulator/fbneo` |
| ROMs dir | `Contents/MacOS/emulator/fbneo/ROMs/` (has `sfiii3nr1.zip`) |
| This Mac tailnet IP | `100.64.0.1` |
| Friend tailnet IP | `100.64.0.2` (`cachyos-host`, Linux) |
| Windows friend tailnet IP | `100.64.0.3` (native FightCade, `%APPDATA%\Fightcade`) |
| Direct arg format | `quark:direct,<rom>,<localPort>,<peerIP>,<peerPort>,<side>,0 -w` |
| P1 mapping (side 0) | local `7001`, peer `7000` |
| P2 mapping (side 1) | local `7000`, peer `7001` |

## Step 0 — Preconditions

- Both machines have Tailscale up and can reach each other:
  - `tailscale status` on the Mac should show `100.64.0.2` online.
  - `tailscale ping 100.64.0.2` should return `pong` (direct or via DERP).
- Both machines have a FightCade install with the **same ROM set** (`sfiii3nr1.zip` here).
- Verify the ROM by name (the name passed to `quark:direct` is the ROM's short name, not the `.zip` filename with extension):
  - Mac side: `ls "/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo/ROMs"`.

## Step 1 — macOS launcher (DONE)

`scripts/fcade-lan-macos.sh` → installed as `~/bin/fcade-lan`; `~/Desktop/FightCade LAN.command` is the double-click entry point. It prompts for:
1. peer Tailscale IP (default `100.64.0.2`),
2. ROM (chosen from `fbneo/ROMs/*.zip`, short name),
3. side (P1 = `0` → local `7001`/peer `7000`; P2 = `1` → mirror).

Then it runs, from `Contents/MacOS/emulator/fbneo`:
```sh
WINEPREFIX=".../.wine32" WINEDEBUG=-all ".../wine32on64" \
  "fcadefbneo.exe" "quark:direct,$ROM,$LOCAL,$PEER_IP,$PEER,$SIDE,0" -w
```

## Step 2 — Linux launcher for the friend (DONE)

`scripts/fcade-lan-linux.sh`. Prefers the **FightCade Flatpak** and falls back to a native/system-Wine install.

**Friend's setup (probed 2026-09-18):** FightCade Flatpak `com.fightcade.Fightcade` on CachyOS. There is also a stale non-Flatpak copy at `~/Games/fightcade-online-retro-gaming/` and a Lutris prefix — ignored; the Flatpak is the real install. System `/usr/bin/wine` exists (no `wine64`), but we use the Flatpak's own Wine instead.

| Flatpak sandbox path | Host path |
|---|---|
| `/app/fightcade/Fightcade/emulator/fbneo/fcadefbneo.exe` | (read-only) |
| `/app/bin/wine`, `/app/fightcade/Resources/wine.sh` | Wine + wrapper |
| `/var/data/wineprefixes/wine-11.0` | `~/.var/app/com.fightcade.Fightcade/data/wineprefixes/` |
| `/var/data/ROMs/fbneo/` | `~/.var/app/com.fightcade.Fightcade/data/ROMs/fbneo/` (`sfiii3nr1.zip`, 71M) |

Flatpak mode launches via:
```sh
flatpak run --command=/bin/sh com.fightcade.Fightcade -c '
  . /app/bin/get-wine-prefix
  cd /app/fightcade/Fightcade/emulator/fbneo
  export WINEDEBUG=-all
  exec /app/fightcade/Resources/wine.sh fcadefbneo.exe \
    "quark:direct,<rom>,<localPort>,<peerIP>,<peerPort>,<side>,0" -w
'
```
The `cd` matters: FBNeo resolves `config/` and `ROMs/` relative to cwd, and Flatpak does not inherit a usable host cwd. `--share=network` means the sandbox uses the host network namespace, so Tailscale routing applies. Manual launch was verified working by the friend.

## Step 2b — Windows launcher (DONE)

`scripts/fcade-lan-windows.bat` for the Windows friend (tailnet `100.64.0.3`). Windows FightCade is native, so there is no Wine or sandbox — the script just finds the emulator and runs it.

- Default install: `%APPDATA%\Fightcade\emulator\fbneo\fcadefbneo.exe`; auto-detects a few fallbacks and honours an `FC_DIR` override.
- Prompts for peer Tailscale IP (default `100.64.0.1`, this Mac), a numbered ROM menu built from `ROMs\*.zip`, and P1/P2.
- Runs from the fbneo dir:
  ```bat
  cd /d "%APPDATA%\Fightcade\emulator\fbneo"
  fcadefbneo.exe "quark:direct,sfiii3nr1,7001,100.64.0.1,7000,0,0" -w
  ```
  (P1 pointing at this Mac; the friend's default peer is this Mac's tailnet IP.)

`scripts/fcade-lan-windows-firewall.bat` self-elevates and adds an inbound allow rule for `fcadefbneo.exe` (`netsh advfirewall`, profile `any`, because Windows may classify the Tailscale adapter as Public). Run once; safe to re-run.

## Step 3 — Coordinate the session

1. Both start Tailscale.
2. Agree on ROM and who is P1/P2.
3. Mac: run `fcade-lan`, enter the friend's IP, ROM `sfiii3nr1`, choose P1.
4. Friend: run their launcher with peer `100.64.0.1`, same ROM, choose P2.
   - Windows friend (`100.64.0.3`): double-click `fcade-lan-windows.bat` (run the firewall `.bat` once beforehand). Peer default is already this Mac.
   - cachyos friend (`100.64.0.2`): run `scripts/fcade-lan-linux.sh`.
5. Launch roughly together; the emulator should open and the match begin.

## Step 4 — Verification

- Watch for a log written by `ggponet.dll`: it references `quark.log` and `quark.log.ignore`. Check `fbneo/quark.log` (and the fbneo working dir) after a session for the direct handshake.
- In-game, FBNeo's on-screen stats can show rollback/ping (config comment: `bVidScanDebug` family; `bShowFPS`/stats option). Confirm frames are exchanging.
- If it fails: confirm both used opposite sides, correct peer IPs, and identical ROMs.

## Step 5 — Firewall (only if needed)

If the emulator's inbound UDP is blocked, allow the Wine binary:
```sh
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add \
  "/Applications/FightCade2.app/Contents/Resources/wine/bin/wine32on64"
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp \
  "/Applications/FightCade2.app/Contents/Resources/wine/bin/wine32on64"
```
Note: current checks report `permitted`, so this is likely unnecessary. The earlier manual attempt used the wrong name (`"FightCade 2.app"`); the correct bundle is `FightCade2.app`.

## Optional step — GUI launcher

If dialogs are not wanted, port `nitsuboy/fightcade-fbneo-lan` (raylib GUI, MIT):
- `brew install raylib`
- Adjust the non-Windows `LDFLAGS` for macOS (the upstream Makefile targets Linux: `-lGL -lX11 -lrt`).
- The friend can run the same project's Linux build.
- Not required for the fix; the shell script is simpler and cross-platform.

## Open questions / risks

1. **Friend's FightCade layout** — RESOLVED: Flatpak `com.fightcade.Fightcade`; the script uses the Flatpak's own Wine. The `~/Games/...` copy and Lutris prefix are ignored.
2. **ROM parity** — CONFIRMED: both have `sfiii3nr1.zip`.
3. **Latency** — RESOLVED: after a router restart the tailnet runs **direct, <100 ms**; matches played end-to-end. The earlier DERP-only (329–514 ms) state is stale.
4. **Real UDP data flow** — RESOLVED: full matches were played over the direct path.
5. **No matchmaking** — this is friend-only. Public/ranked FightCade still requires a real public IP (Option A) or a paid option (B/C).
6. **Client updates** — a FightCade auto-update could change emulator paths or the `quark:direct` argument; re-verify after updates.
7. **Log rotation** — capture `fcade.log*` right after any failed attempt before it rotates.
8. **Tailscale on both ends** — if the friend's node goes offline, expect failure; `tailscale ping` reports the path.
9. **Flatpak read-only cwd** — the emulator runs from `/app/fightcade/Fightcade/emulator/fbneo` (read-only). Config and ROMs are symlinked to writable `/var/data`; only incidental writes (extra screenshots/recordings) could fail.

## Reference links

- Community launcher: <https://github.com/nitsuboy/fightcade-fbneo-lan> (MIT) — source of the `quark:direct` invocation.
- Native macOS client (evaluate later): <https://github.com/Jayian1890/Macade>
- FightCade help/FAQ: <https://www.fightcade.com/help> · <https://www.fightcade.com/faq>
- Tailscale CGNAT troubleshooting: <https://tailscale.com/docs/reference/troubleshooting/network-configuration/cgnat-conflicts>
