import { useEffect, useMemo, useState } from "react";
import { LaunchWarningDialog } from "@/components/LaunchWarningDialog";
import { NetplayBadge } from "@/components/NetplayBadge";
import { PeerHealthBadge } from "@/components/PeerHealthBadge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type {
  Config,
  MatchRole,
  MatchState,
  ParityStatus,
  PeerHealth,
  ProviderInfo,
  RomIndex,
  Tailnet,
} from "@/lib/types";
import { parityStatus, probePort } from "@/lib/api";
import { healthWarning } from "@/lib/health";
import { useInvoke } from "@/lib/query";
import { FlaskConical, Gamepad2, Play, Square } from "lucide-react";

const ROLE_LABELS: Record<MatchRole, string> = {
  p1: "P1 · host",
  p2: "P2 · client",
  spectator: "Spectator",
};

interface LaunchCardProps {
  config: Config | null;
  tailnet: Tailnet | null;
  health: Record<string, PeerHealth>;
  romIndex: RomIndex | null;
  provider: ProviderInfo | null;
  match: MatchState | null;
  busy: boolean;
  onLaunch: (
    rom: string,
    peerIp: string,
    role: MatchRole,
    dev: boolean,
    force: boolean,
  ) => Promise<void>;
  onLaunchDevPair: (rom: string) => Promise<void>;
  onStop: () => Promise<void>;
}

export function LaunchCard({
  config,
  tailnet,
  health,
  romIndex,
  provider,
  match,
  busy,
  onLaunch,
  onLaunchDevPair,
  onStop,
}: LaunchCardProps) {
  const [rom, setRom] = useState("");
  const [peerIp, setPeerIp] = useState("");
  const [role, setRole] = useState<MatchRole>("p1");
  const [dev, setDev] = useState(false);
  const [warnings, setWarnings] = useState<string[] | null>(null);

  const parityQuery = useInvoke<ParityStatus | null>(
    `parity:${rom}:${provider?.kind ?? ""}`,
    () => parityStatus(rom),
    { enabled: rom !== "" && provider?.kind === "retroarch" },
  );
  const parity = parityQuery.data;

  const capabilities = provider?.capabilities;
  const spectate = capabilities?.spectate ?? false;
  const devPair = capabilities?.devPair ?? false;
  const developerMode = config?.developerMode ?? false;

  const rttWarnMs = config?.rttWarnMs ?? 150;
  const onlinePeers = useMemo(
    () => (tailnet?.peers ?? []).filter((peer) => peer.online),
    [tailnet],
  );
  const roms = romIndex?.roms ?? [];

  useEffect(() => {
    if (!rom && roms.length > 0) setRom(roms[0].shortName);
  }, [rom, roms]);

  useEffect(() => {
    if (!spectate && role === "spectator") setRole("p1");
  }, [spectate, role]);

  useEffect(() => {
    if (!developerMode && dev) setDev(false);
  }, [developerMode, dev]);

  useEffect(() => {
    if (peerIp) return;
    const preferred = config?.defaultPeerIp;
    if (preferred && onlinePeers.some((peer) => peer.ip === preferred)) {
      setPeerIp(preferred);
    } else if (onlinePeers.length > 0) {
      setPeerIp(onlinePeers[0].ip);
    }
  }, [config?.defaultPeerIp, onlinePeers, peerIp]);

  const selectedPeer = onlinePeers.find((peer) => peer.ip === peerIp);
  const running = match?.status === "running";

  function collectWarnings(): string[] {
    if (dev || role === "p1") return [];
    const warning = healthWarning(
      selectedPeer?.hostname ?? peerIp,
      health[peerIp],
      rttWarnMs,
    );
    return warning ? [warning] : [];
  }

  async function beginLaunch() {
    const found = collectWarnings();
    if (
      provider?.kind === "retroarch" &&
      !dev &&
      (role === "p2" || role === "spectator") &&
      peerIp !== ""
    ) {
      const port = config?.retroarchPort ?? 55435;
      const status = await probePort(peerIp, port);
      if (!status.reachable) {
        found.push(
          `host not reachable on ${peerIp}:${port}${
            status.error ? ` (${status.error})` : ""
          } — start the host first, confirm the peer IP, and allow inbound TCP ${port} on the host`,
        );
      }
    }
    if (found.length > 0) {
      setWarnings(found);
      return;
    }
    await onLaunch(rom, peerIp, role, dev, false);
  }

  async function confirmLaunch() {
    setWarnings(null);
    await onLaunch(rom, peerIp, role, dev, true);
  }

  const parityBlocked = parity !== null && !parity.ok;
  const hostNeedsNoPeer = provider?.kind === "retroarch" && role === "p1";
  const canLaunch =
    rom !== "" &&
    (dev || hostNeedsNoPeer || peerIp !== "") &&
    !running &&
    !busy &&
    !parityBlocked;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Gamepad2 className="size-4" />
          Launch match
        </CardTitle>
        <CardDescription>
          {provider
            ? provider.install.installed
              ? `${provider.install.label} · ready`
              : `Not found — ${provider.install.detail}`
            : "Checking emulator…"}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="grid gap-2">
            <Label>ROM</Label>
            <Select value={rom} onValueChange={setRom} disabled={running}>
              <SelectTrigger>
                <SelectValue placeholder="Choose a ROM" />
              </SelectTrigger>
              <SelectContent>
                {roms.map((entry) => (
                  <SelectItem key={entry.path} value={entry.shortName}>
                    {entry.shortName}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="grid gap-2">
            <Label>Peer</Label>
            <Select
              value={peerIp}
              onValueChange={setPeerIp}
              disabled={running || dev || role === "p1"}
            >
              <SelectTrigger>
                <SelectValue placeholder="Choose a peer" />
              </SelectTrigger>
              <SelectContent>
                {onlinePeers.length === 0 && (
                  <SelectItem value="__none" disabled>
                    No peers online
                  </SelectItem>
                )}
                {onlinePeers.map((peer) => (
                  <SelectItem key={peer.ip} value={peer.ip}>
                    {peer.hostname} · {peer.ip}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="grid gap-2">
            <Label>Role</Label>
            <Select
              value={role}
              onValueChange={(value) => setRole(value as MatchRole)}
              disabled={running}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="p1">{ROLE_LABELS.p1}</SelectItem>
                <SelectItem value="p2">{ROLE_LABELS.p2}</SelectItem>
                {spectate && (
                  <SelectItem value="spectator">{ROLE_LABELS.spectator}</SelectItem>
                )}
              </SelectContent>
            </Select>
          </div>
          {developerMode && (
            <div className="grid gap-2">
              <Label>Developer mode</Label>
              <label className="flex h-9 items-center gap-2 text-sm text-muted-foreground">
                <input
                  type="checkbox"
                  className="size-4 accent-primary"
                  checked={dev}
                  disabled={running}
                  onChange={(event) => setDev(event.target.checked)}
                />
                Loopback (127.0.0.1), ignore peer
              </label>
            </div>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {running ? (
            <Button variant="destructive" onClick={() => void onStop()}>
              <Square className="size-4" />
              Stop
            </Button>
          ) : (
            <>
              <Button onClick={() => void beginLaunch()} disabled={!canLaunch}>
                <Play className="size-4" />
                Launch
              </Button>
              {developerMode && (
                <Button
                  variant="outline"
                  onClick={() => void onLaunchDevPair(rom)}
                  disabled={rom === "" || busy || !devPair}
                  title={devPair ? undefined : "This provider has no dev pair"}
                >
                  <FlaskConical className="size-4" />
                  Dev pair
                </Button>
              )}
            </>
          )}
          {match && match.status !== "idle" && (
            <span className="text-sm text-muted-foreground">
              {match.status === "running"
                ? `${match.rom} · ${match.dev ? "loopback" : match.peerIp} · ${match.instances
                    .map(
                      (instance) =>
                        `${instance.roleLabel}@${instance.port ?? "—"} (pid ${instance.pid})`,
                    )
                    .join(" + ")}`
                : (match.message ?? "finished")}
            </span>
          )}
          {match?.status === "running" &&
            match.instances.map((instance) => (
              <NetplayBadge
                key={instance.role}
                netplay={instance.netplay}
                label={instance.roleLabel}
              />
            ))}
          {match?.status === "running" && match.peerHealth && (
            <PeerHealthBadge
              health={match.peerHealth}
              rttWarnMs={rttWarnMs}
              loading={false}
            />
          )}
        </div>

        {match?.status === "running" && (
          <div className="flex flex-col gap-1">
            {match.instances
              .filter((instance) => instance.netplay?.lastEvent)
              .map((instance) => (
                <span
                  key={instance.role}
                  className="truncate text-xs text-muted-foreground"
                  title={instance.netplay?.lastEvent ?? undefined}
                >
                  {instance.roleLabel}: {instance.netplay?.lastEvent}
                </span>
              ))}
          </div>
        )}

        {parity && (
          <p
            className={
              parity.ok
                ? "text-xs text-muted-foreground"
                : "text-xs text-destructive"
            }
            title={`core ${parity.corePath} · rom ${parity.romPath}`}
          >
            {parity.ok ? "Parity OK" : parity.detail}
          </p>
        )}
      </CardContent>

      <LaunchWarningDialog
        warnings={warnings}
        onCancel={() => setWarnings(null)}
        onConfirm={() => void confirmLaunch()}
      />
    </Card>
  );
}
