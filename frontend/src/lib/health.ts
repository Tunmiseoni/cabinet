import type { PeerHealth } from "@/lib/api";

export type HealthLevel = "healthy" | "warn" | "unreachable" | "unknown";

export function healthLevel(
  health: PeerHealth | undefined,
  rttWarnMs: number,
): HealthLevel {
  if (!health) return "unknown";
  if (health.path === "local") return "healthy";
  if (!health.ok || health.path === "unknown") return "unreachable";
  if (health.path === "relay") return "warn";
  if (health.rttMs !== null && health.rttMs > rttWarnMs) return "warn";
  return "healthy";
}

export function healthLabel(health: PeerHealth | undefined): string {
  if (!health) return "checking…";
  if (health.path === "local") return "local";
  if (!health.ok || health.path === "unknown") return "unreachable";
  const rtt = health.rttMs !== null ? `${Math.round(health.rttMs)} ms` : "?";
  if (health.path === "relay") {
    return `${rtt} · DERP${health.relayCode ? ` (${health.relayCode})` : ""}`;
  }
  return `${rtt} · direct`;
}

export function healthWarning(
  hostname: string,
  health: PeerHealth | undefined,
  rttWarnMs: number,
): string | null {
  const level = healthLevel(health, rttWarnMs);
  if (level === "warn") {
    if (health?.path === "relay") {
      return `${hostname} is relayed via DERP${
        health.relayCode ? ` (${health.relayCode})` : ""
      } — expect higher latency.`;
    }
    return `${hostname} is at ${Math.round(health?.rttMs ?? 0)} ms, above the ${rttWarnMs} ms threshold.`;
  }
  if (level === "unreachable") {
    return `${hostname} did not answer a tailscale ping.`;
  }
  return null;
}
