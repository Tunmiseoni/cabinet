# Remediation options

All options are compared against the confirmed root cause: **CGNAT + symmetric NAT at the ISP**. The question is how to give FightCade a usable peer-to-peer path.

Legend: ✅ recommended · ⚠️ works but costly/limited · ❌ does not solve CGNAT by itself.

---

## Option A — Ask the ISP for a public/static IP ✅ (cleanest, often paid)

- **Idea:** Have MTN move this line off CGNAT or assign a static public IPv4.
- **Then:** port-forward on the ZTE router (UDP `6000-6009`, TCP `7000-7005`) to `192.168.1.100`.
- **Pros:** fixes all peer-to-peer games, no extra software, no change to how FightCade is used.
- **Cons:** MTN may refuse or charge; often business plans only; may not be available on this plan. Does not fix symmetric NAT by itself (many CGNAT gateways are symmetric) — but a real public IP at least makes the ports reachable, which is the fallback FightCade uses.
- **Cost:** free–paid (varies).
- **Status:** user does **not** want to depend on this.

Router port-forward rule (only useful once a public IP exists):

| Service | Protocol | External ports | Internal IP | Internal ports |
|---|---|---|---|---|
| FightCade | UDP | 6000–6009 | 192.168.1.100 | 6000–6009 |
| FightCade | TCP | 7000–7005 | 192.168.1.100 | 7000–7005 |

---

## Option B — Commercial VPN with inbound port forwarding ⚠️

- **Idea:** Tunnel through a VPN provider that supports port forwarding (e.g. AirVPN, ProtonVPN paid), so FightCade sees a public, reachable endpoint.
- **Pros:** no coding; provider handles NAT traversal for forwarded ports.
- **Cons:** requires a paid subscription (user declined paid); adds latency from Nigeria; provider NAT may itself be symmetric; port-forward discovery is fiddly; must keep the VPN full-tunnel for FightCade.
- **Cost:** subscription.

---

## Option C — Self-hosted VPS + WireGuard/Tailscale with UDP DNAT ⚠️ (most reliable technical fix)

- **Idea:** Rent a cheap VPS with a public IPv4. Run WireGuard (or a Tailscale exit/subnet node) plus `iptables` DNAT to forward inbound UDP `6000-6009` / TCP `7000-7005` to this Mac's tailnet IP. FightCade then egresses via the VPS and receives inbound on the forwarded ports.
- **Pros:** reliable, works behind any NAT, full FightCade UI/matchmaking preserved.
- **Cons:** requires a paid VPS (user declined paid) and ongoing maintenance; latency via the VPS.
- **Cost:** ~$5/month.
- **Reference pattern:** "Forward Ports to a Tailscale VM Behind CGNAT Using IPTables".

---

## Option D — Direct-connect FBNeo over Tailscale (no FightCade servers) ✅ RECOMMENDED (free)

- **Idea:** The emulator itself supports a direct `quark:direct` mode. Both players launch `fcadefbneo.exe` pointed at each other's **Tailscale IP**. No FightCade matchmaking, no hole punching, no CGNAT.
- **Evidence it works:** `fcadefbneo.exe` contains the parser `quark:direct,%[^,],%d,%[^,],%d,%d,%d,%d`. A known-good community launcher (`nitsuboy/fightcade-fbneo-lan`, MIT) builds exactly:
  ```
  fcadefbneo.exe quark:direct,<rom>,<localPort>,<peerIP>,<peerPort>,<side>,0 -w
  ```
  with side `0` = P1 (local `7001`, peer `7000`) and side `1` = P2 (local `7000`, peer `7001`).
- **Why it fits this user:** Tailscale is already installed and the friend `cachyos-host` (`100.64.0.11`) is reachable. This bypasses the ISP problem entirely and costs nothing.
- **Pros:** free; uses existing Tailscale; no ISP/VPN; no port forwarding; works behind any NAT; uses the same emulator, ROMs, and rollback netcode.
- **Cons:** not FightCade matchmaking — it is a **friend-only** connection (no ranked/public lobbies). Both players must run the direct launcher and use the **same ROM set**. The Linux side needs its own launcher/script.
- **Latency reality (updated 2026-09-18):** earlier measurements had both ends on symmetric NAT with no direct path, relaying via DERP `par` (~330–500 ms). After a router restart the tailnet established a **direct path, <100 ms**, and matches were played successfully over it. The direct path is the current operating condition; DERP remains only a fallback if a direct path is ever lost.
- **Cost:** free.
- **Implementation:** see [`03-implementation-plan.md`](03-implementation-plan.md).

### Exact command template (macOS)

```sh
cd "/Applications/FightCade2.app/Contents/MacOS/emulator/fbneo"
WINEPREFIX="/Applications/FightCade2.app/Contents/Resources/.wine32" \
WINEDEBUG=-all \
"/Applications/FightCade2.app/Contents/Resources/wine/bin/wine32on64" \
  "fcadefbneo.exe" "quark:direct,sfiii3nr1,7001,100.64.0.11,7000,0,0" -w
```

(That is P1 pointing at the friend. The friend runs the mirror image.)

---

## Option E — Other virtual-LAN tools (Radmin VPN / Hamachi / ZeroTier) ❌/⚠️

- **Idea:** Replace Tailscale with another virtual LAN.
- **Reality:** These give connectivity just like Tailscale, but **FightCade matchmaking still ignores them**, so they only help via the same `quark:direct` trick (Option D). No advantage over the existing Tailscale; Radmin/Hamachi are Windows-only. ZeroTier/Lanemu would work but add a new dependency for no benefit here.
- **Cost:** free (mostly).

---

## Option F — Native macOS client: Macade ⚠️ (not a guaranteed fix)

- **Idea:** `Jayian1890/Macade` — "Fightcade for macOS, rebuilt as a native Mac app", Apple-Silicon native, bundled FBNeo.
- **Pros:** native SwiftUI, no Wine, embedded gameplay/spectating; potentially simpler and faster than the Wine build.
- **Cons:** **early public software**; it advertises "Fightcade-compatible ... netplay launch flows", which most likely still uses FightCade matchmaking and therefore **would still be subject to the CGNAT problem**. Also a separate, less battle-tested codebase; ROM provisioning and friend-side compatibility are unknowns.
- **Cost:** free.
- **Verdict:** worth watching, but do **not** rely on it to fix CGNAT. Re-evaluate after Option D if the user wants a nicer UI.

---

## Option G — RetroArch + FBNeo core + Netplay over Tailscale ⚠️

- **Idea:** Skip FightCade entirely; use RetroArch's built-in netplay hosting over Tailscale IPs.
- **Pros:** free, cross-platform (Mac + Linux), no Wine needed on the Mac if a native core is used.
- **Cons:** different netcode/experience than FightCade's GGPO rollback; input latency/config differences; still friend-only. More of a fallback than a fix.
- **Cost:** free.

---

## Option H — Port-forward the ZTE router alone ❌

- **Idea:** Just forward ports without fixing CGNAT.
- **Reality:** the packets never reach the router's WAN side because the ISP's CGNAT owns the public edge. **This cannot work alone.** Only relevant as a step *after* Option A.

---

## Comparison

| Option | Solves CGNAT? | Cost | Preserves FightCade UI/matchmaking | Effort | Verdict |
|---|---|---|---|---|---|
| A. ISP public IP | ✅ (if granted) | free–paid | ✅ | low | not desired |
| B. Paid VPN w/ port forward | ✅ (usually) | paid | ✅ | low | declined (paid) |
| C. VPS + WireGuard DNAT | ✅ | ~$5/mo | ✅ | high | declined (paid) |
| **D. Tailscale direct-connect** | ✅ | **free** | ❌ (friend-only) | medium | **recommended** |
| E. Radmin/Hamachi/ZeroTier | only with D | free | ❌ | medium | no benefit |
| F. Macade | likely ❌ | free | ✅ | low | evaluate later |
| G. RetroArch netplay | ✅ | free | ❌ | medium | fallback |
| H. Port forward only | ❌ | free | ✅ | low | insufficient |

**Chosen route: Option D** — free, uses the existing Tailscale link, and works regardless of the ISP's NAT.
