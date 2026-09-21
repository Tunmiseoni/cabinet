# Diagnosis — why online challenges hang

## 1. Architecture of FightCade 2 on macOS

FightCade 2 is not a single program. Three layers matter:

1. **Electron app** (`Contents/MacOS/Fightcade2`) — the chat / lobby UI, a Nativefier wrapper around `web.fightcade.com`.
2. **`fcade` helper** (`Contents/MacOS/emulator/fcade`, a PyInstaller binary) — performs login/status and the **networking handshake** for a match. It exposes a URL scheme handler `fcade://` (registered as `FightCade2Handler.app`).
3. **Emulator** (`Contents/MacOS/emulator/fbneo/fcadefbneo.exe`) — a 32-bit Windows PE running under the bundled Wine (`Contents/Resources/wine/bin/wine32on64`) via Rosetta 2. Netplay uses `ggponet.dll` (GGPO rollback).

Why the asymmetry:

| Feature | Needs peer-to-peer? | Works? |
|---|---|---|
| Test game / training | No (local) | Yes |
| Tutorial | No (local) | Yes |
| Spectating | No (streams from a game server) | Yes |
| **Challenge / accept challenge** | **Yes (direct GGPO session)** | **No** |

## 2. FightCade's own log shows the failure

In `Applications/FightCade2.app/Contents/MacOS/emulator/fcade.log` (captured before log rotation) during a match attempt:

```
2026-09-18 15:54:08  Listening on 127.0.0.1:7001 (udp)
2026-09-18 15:54:08  Quark: 1789743210916-4364.0
2026-09-18 15:54:08  Request sent, waiting for partner in quark '1789743210916-4364.0'...
2026-09-18 15:54:13  Listening on 0.0.0.0:44364/udp
2026-09-18 15:54:16  Listening on 0.0.0.0:6004/udp
2026-09-18 15:54:19  Puncher failed. Using ports.
```

Interpretation:
- `Quark` is FightCade's rendezvous service; both players exchange candidate endpoints and attempt **UDP hole punching**.
- `Puncher failed. Using ports.` means hole punching did not succeed, so it fell back to the fixed ports (UDP `6000-6009`, TCP `7000-7005`).
- Those fixed ports are **not reachable inbound**, so the helper waits indefinitely for the partner. The emulator never launches and the user stays in the chat UI.

Log rotation note: `fcade.log` is rotated on each launch (`fcade.log.1` → `.2` → `.3`), so history can be lost between attempts. Capture logs right after a failed challenge.

## 3. Traceroute proves CGNAT

```
$ traceroute -m 6 -w 1 -q 1 8.8.8.8
 1  192.0.2.1      <- home ZTE router (LAN side)
 2  198.51.100.2       <- ISP private network
 3  198.51.100.3     <- ISP private network
 4  198.51.100.4    <- ISP private network
 5  203.0.113.12     <- first public hop
```

The home router's WAN side is in `198.51.100.0/24`, i.e. the ISP puts the router behind its own NAT. This is **Carrier-Grade NAT (CGNAT)**: many customers share one public IPv4 address. Port forwarding on the home router therefore dead-ends at the ISP.

## 4. STUN proves symmetric NAT (the worst case for hole punching)

Querying four different public STUN servers (different destination IPs) from the same host:

```
stun.l.google.com:19302  -> 203.0.113.10:15644
stun1.l.google.com:19302 -> 203.0.113.10:2021
stun3.l.google.com:19302 -> 203.0.113.10:2339
stun4.l.google.com:19302 -> 203.0.113.10:4145
```

Same public IP, but a **different mapped port for every destination**. That is **endpoint-dependent mapping (symmetric NAT)**. A mapping created toward peer A cannot be reused by peer B, which is exactly what breaks UDP hole punching.

Together, CGNAT + symmetric NAT is the worst combination for the GGPO/Quark puncher.

## 5. What it is NOT

- **Not the macOS Application Firewall.** `socketfilterfw --getappblocked` reports `permitted` for:
  - `/Applications/FightCade2.app`
  - `/Applications/FightCade2.app/Contents/MacOS/emulator/fcade`
  - `/Applications/FightCade2.app/Contents/Resources/wine/bin/wine32on64`
  A previous manual attempt used the wrong bundle name (`"FightCade 2.app"` with a space); the actual app is `FightCade2.app`. This did not cause the failure, though it is worth correcting for clarity.
- **Not a FightCade bug / not an outdated client.** `update.log` shows `current version: 2.1.45`, `latest version: 2.1.45`.
- **Not Tailscale interference.** `tailscale status` was `stopped` during the failing attempt, and the default route is the normal home gateway (`192.0.2.1` via `en0`). Tailscale's `utun` interfaces do not carry FightCade traffic.

## 6. Why Tailscale does not fix FightCade automatically

Tailscale gives this Mac the address `100.64.0.1` and can reach the friend. But FightCade's matchmaking server issues **public** endpoints and the client/helper connects to those; it has no concept of the tailnet. So the CGNAT is still in the path for FightCade's own netplay, even while Guilty Gear Strive (which can bind/route over the tailnet) works fine.

**Measurement history (2026-09-18):** the friend `cachyos-host` is *also* behind symmetric NAT (`tailscale netcheck` → `MappingVariesByDestIP: true`; public `203.0.113.11`). With symmetric NAT on both ends, Tailscale initially had no direct path: `tailscale status --json` showed peer `CurAddr` empty, `Relay = par`, and `tailscale ping 100.64.0.2` returned `via DERP(par)` at 329–514 ms with `direct connection not established`.

**Current state (later, 2026-09-18):** after restarting the home router, the tailnet established a **direct path with <100 ms** ping, and four friends played matches over it all afternoon. The DERP-relay figures above are therefore stale. The NAT analysis still explains why FightCade's *own* matchmaking fails (it never uses the tailnet), but it no longer constrains the `quark:direct` route.

This confirms the fix: make the **emulator** connect over the tailnet directly, bypassing FightCade matchmaking. See [`02-options.md`](02-options.md).
