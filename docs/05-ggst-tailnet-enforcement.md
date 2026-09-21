# Guilty Gear Strive over Tailscale — feasibility, terms, and enforcement options

Investigation for a possible **companion tool** (separate from The Cabinet) that checks whether
**Guilty Gear Strive (GGST)** is actually using the Tailscale path to a friend, and — if not —
helps push it back onto the tailnet.

> Scope note: The Cabinet is committed to FightCade / FBNeo `quark:direct`, where the peer IP is
> **passed to the launcher** — that is why it works. GGST has no such argument; it picks its own
> P2P candidate via Arc System Works matchmaking. This document covers a *separate* tool. Any
> code decided here lives outside this repo; the existing Tailscale parser
> (`src-tauri/src/tailscale.rs`) can be reused.

**Status:** research only. No code written. Open decisions in §8.

---

## 1. Problem statement

GGST is played via **Steam under CrossOver**; the user and friends are on a Tailscale tailnet
because the ISP path is poor (CGNAT, intermittent NAT/IPv6). Both machines run Tailscale and the
game *sometimes* picks the `100.x` tailnet address for the P2P match connection.

Observed symptoms:

- Tailscale ping to the friend is ~80 ms, but the in-game connection is ~200 ms.
- The game intermittently "drops off" Tailscale and uses the normal ISP path (NAT-based, not the
  direct tailnet path).
- Restarting Steam **and** the game restores the good path until it happens again.

Goal: detect when GGST's P2P connection is **not** on the tailnet, and adjust it. The user's
preferred adjustment is **firewall-based forced routing** (see §5), with the safe fallback being
detect + guided restart.

---

## 2. How the networking actually works (hard constraints)

These constrain what any tool can do:

1. **Tailscale only carries traffic addressed to tailnet IPs** (`100.64.0.0/10`, and the
   `fd7a:...` IPv6 range). It does **not** intercept arbitrary traffic to public IPs. Therefore a
   tool **cannot reroute a public-IP path *through* Tailscale.** "Force it onto the tailnet" can
   only mean *deny the non-tailnet candidate so the game falls back to the `100.x` candidate.*
2. **Per-process scoping is mandatory for enforcement.** `tailscaled`'s own WireGuard packets go
   to the peer's **same public IP**. A global rule blocking that IP would kill Tailscale itself.
   Any firewall rule must be scoped to the GGST process (pid/program), never to the address alone.
3. **GGST gameplay is fully peer-to-peer.** Matchmaking/auth goes through Arc System Works
   servers, but in-match data is P2P (community-confirmed; the game's own "ping" reflects the
   P2P path). Server latency does not affect the match connection.
4. **A live match cannot be rerouted.** The P2P connection is fixed at match start. Any fix is
   *detect + restart/re-negotiate*, which is exactly why the manual Steam+game restart works.
5. **Why it ever works at all:** the tailnet interface (`100.64/10`) looks like a
   local/private/LAN address, so it can appear as a candidate and may be selected. When candidate
   selection or NAT traversal favours the public path (or a relay), latency jumps. That is the
   80 ms → 200 ms symptom. (Exact candidate logic is unverified — see Phase 0.)

---

## 3. Terms-of-service / policy assessment

> Not legal advice. I am not a lawyer. This is an interpretation of publicly available terms to
> set a safe engineering boundary.

Sources reviewed: **Steam Subscriber Agreement** (store.steampowered.com/subscriber_agreement).
Arc System Works' own GGST EULA could **not** be retrieved; treat that as an open item.

| Clause | What it says | Bearing here |
|---|---|---|
| SSA §2.G (Restrictions) | Prohibits hosting/emulating/**tunneling** Valve's network protocols or "network play utilizing commercial or non-commercial gaming networks" without Valve's consent. | Broad wording, but aimed at emulating/redirecting **Valve's** matchmaking. GGST's P2P is not a Valve protocol. Doubtful it reaches a private tailnet; not enforced against VPN users in practice. |
| SSA §3.A (Payment) | Prohibits "IP proxying … to disguise the place of your residence" to dodge geo-restrictions/pricing. | Purpose here is latency, not geo/pricing. Not a breach on its face. |
| SSA §4.B (Cheating) | "You agree that you will **not tamper with the execution of Steam or Content and Services** unless otherwise authorized by Valve." | **The real risk.** Injecting into/hooking/patching the GGST process to reroute its sockets = tampering. Avoid. |
| SSA §4.D (Enforcement) | Valve may restrict/terminate for breach. | Consequence if §4.B is breached. |

GGST currently has **no anti-cheat** (community consensus), so nothing would auto-detect process
tampering — but that does not override §4.B or ArcSys's modding terms.

**Safe boundary:**

| Action | Verdict |
|---|---|
| Read peer remote IP / which interface a socket uses (`lsof`, `netstat`, `ss`, `tailscale status --json`) | ✅ No ToS concern |
| Alert "not on tailnet" and offer/perform a **restart of Steam + GGST** | ✅ Automating a user action; does not touch game internals |
| **OS firewall rule** deprioritising/blocking the non-tailnet path, scoped to the game process | ✅ Your own machine's config; not tampering with the game |
| Inject / hook / patch the GGST process to force routing | ❌ Violates SSA §4.B (and likely ArcSys terms) |

Running a Windows game under CrossOver/Wine is itself normal and tolerated (Steam ships Proton),
and is not part of this concern.

---

## 4. Options considered

Legend: ✅ recommended · ⚠️ works but costly/limited · ❌ rejected.

| # | Option | Mechanism | Risk | Notes |
|---|---|---|---|---|
| A | **Read-only monitor** | Watch GGST sockets, classify remotes (tailnet `100.x` / peer public / relay / ArcSys) | ✅ none | Useful on all platforms; no admin |
| B | **Detect + guided restart** | On non-tailnet detection, prompt then restart Steam + GGST | ✅ none | Reproduces the known-good manual fix; no admin; all 3 OS |
| C | **Firewall-enforced fallback** | Deny the peer's public endpoint for the GGST pid, forcing the `100.x` candidate | ⚠️ low | User-preferred; per-process scoping essential; needs elevation; can break the match if the tailnet candidate isn't available |
| D | Global block of the peer public IP | Same but address-scoped | ❌ | Kills `tailscaled` itself (see §2.2) |
| E | Inject/hook the game process | Patch socket/bind behaviour | ❌ | ToS violation (SSA §4.B) |
| F | Force interface preference globally | Route metrics / service order | ❌ | Tailscale adds routes only for `100.64/10` without an exit node; does not stop a public candidate; affects all apps |

Chosen direction: **A → B now; C as an optional elevated power-user mode.** D–F rejected.

---

## 5. Recommendation

**Build a separate companion tool, outside this repo**, rather than a module inside The Cabinet.
Reasons:

- The Cabinet's peer IP is an *input* to `quark:direct`; GGST needs OS-level enforcement with
  elevated privileges — a different risk class from a cross-platform GUI launcher.
- Keeps The Cabinet's committed FightCade/FBNeo scope and phase plan intact.
- The tool can still reuse this repo's Tailscale logic: `src-tauri/src/tailscale.rs` already parses
  `tailscale status --json` including `CurAddr` (the peer's public endpoint) and `Relay`, plus
  `PathKind`/`PeerHealth` and `tailscale ping` RTT parsing.

Decisions already made with the user:

- **Location:** separate tool out of this repo; code here may be reused.
- **Adjustment preference:** firewall-based forced routing would be nice (as the elevated option).
- **macOS enforcement depth and platform priority:** still open (§8).

---

## 6. Platform feasibility for firewall enforcement (Option C)

| OS | Per-process deny possible? | How | Caveat |
|---|---|---|---|
| **Linux** | ✅ | `nftables` `meta skuid` / cgroup v2 match (or `iptables -m owner`) | Steam game runs as the same user → prefer a cgroup/slice match; root required |
| **Windows** | ✅ | Windows Filtering Platform, or `netsh advfirewall firewall add rule … program="…\GGST.exe" remoteip=<peerPublic> dir=out action=block` | Admin required; path is the CrossOver/Steam-launched exe |
| **macOS** | ❌ with built-ins | `pf` cannot match by process; `user` matching would also catch `tailscaled` | True per-process needs a signed **Network Extension** (Apple entitlement + notarization) or a third-party firewall's CLI; otherwise stay Option B |

Because enforcement is a **persistent, privileged change to the host**, it must be opted into
explicitly, with a clear warning, an easy off switch, and auto-rollback (see §7). Per AGENTS.md,
such changes to the host require asking the user first.

### 6.1 macOS enforcement depth — the options

macOS is the limiting platform because its built-in packet filter cannot reliably match by
process. The realistic choices, shallowest to deepest:

| Depth | Mechanism | Admin / setup | Can scope to GGST pid? | Trade-offs |
|---|---|---|---|---|
| **0. Detect-only + guided restart** | Read GGST sockets; prompt and restart Steam + GGST | None | n/a (no filtering) | ✅ Zero risk, all OS. Cannot prevent a bad path once a match starts — only detect and recover. **Recommended default.** |
| **1. `pf` with `user` match** | `pf` rule matching the socket owner | Root (`pfctl`) | ❌ | Both GGST and `tailscaled` run as the same user, so this also blocks Tailscale's own traffic. Does not actually work here. |
| **2. Third-party firewall CLI** | `LuLu` / `Little Snitch` / `Murus` scripting | Install vendor system/network extension; some paid | ⚠️ per-app if the vendor supports it | Avoids building a Network Extension, but adds a commercial/closed dependency, an opaque automation surface, and per-user licensing. |
| **3. Signed Network Extension** | `NEFilterDataProvider` (content filter system extension) | Apple Developer Program, Developer ID, entitlements, notarization, elevated install, user approval in System Settings | ✅ | The only correct per-process option, but heavy: ongoing signing/notarization cost and a system-extension approval UX. |
| **4. Route / interface tricks** | Service order, route metrics | Root | ❌ | Tailscale adds routes only for `100.64/10` (no exit node), so it cannot demote the public candidate. Not viable. |

Decision implication: if the user wants macOS enforcement beyond detect-only, **option 3** is the
only faithful implementation; **option 1/4** do not work and **option 2** trades a build for a
dependency. Otherwise macOS stays on **option 0** while Linux/Windows get pid-scoped enforcement.

---

## 7. Phased plan

### Phase 0 — read-only diagnosis spike (no committed code)
Prove the premise on the user's Mac during a live match:

- Find the GGST pid; enumerate its UDP sockets/remotes (`lsof -i -nP -p <pid>`, `nettop -p <pid>`).
- Classify each remote: tailnet `100.x` / `fd7a:`, peer public IP, DERP relay, ArcSys server.
- Capture one **good** (≈80 ms) and one **bad** (≈200 ms) match and diff them.
- Confirm whether the good path is really the `100.x` candidate (and how it is selected) or
  something else.
- Output: a short findings note appended to this document. **Go/no-go** for Phase 2 enforcement.

### Phase 1 — detector + guided restart (safe; no admin; all 3 OS)
- Watch for GGST by process path.
- Read the P2P remote endpoint; reuse `tailscale.rs` to know which remotes are tailnet.
- If the peer path is not `100.x`/`fd7a:`, surface "not on tailnet" and offer a one-click
  **restart Steam + GGST** (the action already done manually).
- Optional: show tailscale ping RTT vs the game's reported ping, to make the gap visible.

### Phase 2 — optional enforcement (power-user; elevated)
- Learn the peer's public endpoint from `tailscale status --json` (`CurAddr`).
- Add a **pid/program-scoped** deny for that endpoint (never address-scoped), behind an explicit
  toggle, with:
  - a clear warning that if the tailnet candidate is unavailable, the match fails;
  - automatic rollback (timeout / on game exit);
  - an always-available "disable rules" action.
- Linux and Windows first. macOS enforcement only if a Network Extension is deemed worth the
  Apple entitlement/notarization cost; otherwise macOS stays on Phase 1.

---

## 8. Open questions / decisions pending

1. **macOS depth:** stay detect-only + guided restart (option 0, recommended, zero admin), or
   invest in a signed Network Extension (option 3) for true per-process enforcement? See §6.1.
2. **Platform priority:** macOS first (detect + guided restart) with enforcement later, all three
   OSes in lockstep, or Linux/Windows enforcement with macOS detect-only?
3. **Detection sign-off:** Phase 0 must confirm that a non-tailnet remote can be reliably
   classified from the game's sockets before any enforcement work is justified.
4. **ArcSys EULA:** retrieve and review GGST's own terms (still unchecked) before shipping
   enforcement.
5. **Tool identity:** name/repo for the companion tool; whether it ever folds back into The
   Cabinet as a general tailnet game launcher.

---

## 9. Publication-scrub reminder

This repo is public. Any notes, logs, or config added for this work must use placeholders, not
real identifiers: `100.x.x.x` / `100.64.0.10`, `203.0.113.x`, `192.0.2.x`,
`example-tailnet.ts.net`. Never commit tokens, keys, ROMs, emulator binaries, or local machine
paths with personal data.
