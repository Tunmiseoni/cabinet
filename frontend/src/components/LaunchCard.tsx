import { useEffect, useMemo, useState } from "react";
import { PeerHealthBadge } from "@/components/PeerHealthBadge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
  MatchResult,
  MatchState,
  ParityStatus,
  PeerHealth,
  ProviderInfo,
  RomIndex,
  Tailnet,
} from "@/lib/api";
import { parityStatus } from "@/lib/api";
import { healthWarning } from "@/lib/health";
import { FlaskConical, Gamepad2, Play, Square, Trophy } from "lucide-react";

const ROLE_LABELS: Record<MatchRole, string> = {
  p1: "P1 · host",
  p2: "P2 · client",
  spectator: "Spectator",
};

function describeResult(result: MatchResult): string {
  const score = `${result.p1Score ?? "?"}–${result.p2Score ?? "?"}`;
  const winner =
    result.winnerSide === 0
      ? (result.p1Name ?? "P1")
      : result.winnerSide === 1
        ? (result.p2Name ?? "P2")
        : null;
  return winner ? `${winner} wins · ${score}` : `draw · ${score}`;
}

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
  const [parity, setParity] = useState<ParityStatus | null>(null);

  const capabilities = provider?.capabilities;
  const spectate = capabilities?.spectate ?? false;
  const devPair = capabilities?.devPair ?? false;

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
    if (!rom || provider?.kind !== "retroarch") {
      setParity(null);
      return;
    }
    let cancelled = false;
    parityStatus(rom)
      .then((status) => {
        if (!cancelled) setParity(status);
      })
      .catch(() => {
        if (!cancelled) setParity(null);
      });
    return () => {
      cancelled = true;
    };
  }, [rom, provider?.kind]);

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
    if (dev) return [];
    const warning = healthWarning(
      selectedPeer?.hostname ?? peerIp,
      health[peerIp],
      rttWarnMs,
    );
    return warning ? [warning] : [];
  }

  async function beginLaunch() {
    const found = collectWarnings();
    if (found.length > 0) {
      setWarnings(found);
      return;
    }
    await onLaunch(rom, peerIp, role, dev);
  }

  async function confirmLaunch() {
    setWarnings(null);
    await onLaunch(rom, peerIp, role, dev);
  }

  const parityBlocked = parity !== null && !parity.ok;
  const canLaunch =
    rom !== "" &&
    (dev || role === "spectator" || peerIp !== "") &&
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
              disabled={running || dev || role === "p1" || role === "spectator"}
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
              <Button
                variant="outline"
                onClick={() => void onLaunchDevPair(rom)}
                disabled={rom === "" || busy || !devPair}
                title={devPair ? undefined : "This provider has no dev pair"}
              >
                <FlaskConical className="size-4" />
                Dev pair
              </Button>
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
          {match?.status === "running" && match.peerHealth && (
            <PeerHealthBadge
              health={match.peerHealth}
              rttWarnMs={rttWarnMs}
              loading={false}
            />
          )}
          {match?.result && (
            <span className="flex items-center gap-1.5 text-sm font-medium">
              <Trophy className="size-4 text-amber-500" />
              {describeResult(match.result)}
            </span>
          )}
        </div>

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

      <Dialog open={warnings !== null} onOpenChange={(open) => !open && setWarnings(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Connection warning</DialogTitle>
            <DialogDescription>
              The selected peer may not give a good match. Launch anyway?
            </DialogDescription>
          </DialogHeader>
          <ul className="list-disc space-y-1 pl-5 text-sm">
            {(warnings ?? []).map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
          <DialogFooter>
            <Button variant="outline" onClick={() => setWarnings(null)}>
              Cancel
            </Button>
            <Button onClick={() => void confirmLaunch()}>Launch anyway</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
