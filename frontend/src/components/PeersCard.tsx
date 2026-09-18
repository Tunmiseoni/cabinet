import { PeerHealthBadge } from "@/components/PeerHealthBadge";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import type { Peer, PeerHealth, Tailnet } from "@/lib/api";
import { formatBytes, formatLastSeen, osLabel } from "@/lib/format";
import { Users } from "lucide-react";

interface PeersCardProps {
  tailnet: Tailnet | null;
  loading: boolean;
  error: string | null;
  health: Record<string, PeerHealth>;
  healthLoading: boolean;
  rttWarnMs: number;
}

interface PeerRowProps {
  peer: Peer;
  health: PeerHealth | undefined;
  healthLoading: boolean;
  rttWarnMs: number;
}

function PeerRow({ peer, health, healthLoading, rttWarnMs }: PeerRowProps) {
  return (
    <div className="flex items-center justify-between gap-3 py-2">
      <div className="flex min-w-0 flex-col">
        <div className="flex items-center gap-2">
          <span className="truncate font-medium">
            {peer.isSelf ? `${peer.hostname} (you)` : peer.hostname}
          </span>
          {peer.active && (
            <Badge variant="secondary" className="text-[10px]">
              active
            </Badge>
          )}
        </div>
        <span className="truncate text-xs text-muted-foreground">
          {peer.ip} · {osLabel(peer.os)}
          {!peer.online && ` · last seen ${formatLastSeen(peer.lastSeen)}`}
          {peer.online &&
            (peer.txBytes > 0 || peer.rxBytes > 0) &&
            ` · ↑${formatBytes(peer.txBytes)} ↓${formatBytes(peer.rxBytes)}`}
        </span>
      </div>
      {peer.isSelf ? (
        <Badge variant="secondary">self</Badge>
      ) : peer.online ? (
        <PeerHealthBadge
          health={health}
          rttWarnMs={rttWarnMs}
          loading={healthLoading}
        />
      ) : (
        <Badge variant="outline">offline</Badge>
      )}
    </div>
  );
}

export function PeersCard({
  tailnet,
  loading,
  error,
  health,
  healthLoading,
  rttWarnMs,
}: PeersCardProps) {
  const self = tailnet?.selfPeer ?? null;
  const peers = tailnet?.peers ?? [];
  const onlineCount = peers.filter((peer) => peer.online).length;

  return (
    <Card className="flex min-h-0 flex-col">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Users className="size-4" />
          Tailnet peers
        </CardTitle>
        <CardDescription>
          {error
            ? "Tailscale unavailable"
            : loading
              ? "Scanning tailnet…"
              : `${onlineCount} of ${peers.length} online${
                  tailnet?.backendState
                    ? ` · backend ${tailnet.backendState.toLowerCase()}`
                    : ""
                }`}
        </CardDescription>
      </CardHeader>
      <CardContent className="min-h-0 flex-1">
        <ScrollArea className="h-64 pr-3">
          {self && (
            <>
              <PeerRow
                peer={self}
                health={undefined}
                healthLoading={false}
                rttWarnMs={rttWarnMs}
              />
              {peers.length > 0 && <Separator />}
            </>
          )}
          {peers.map((peer) => (
            <div key={peer.dnsName || peer.ip}>
              <PeerRow
                peer={peer}
                health={health[peer.ip]}
                healthLoading={healthLoading}
                rttWarnMs={rttWarnMs}
              />
              <Separator />
            </div>
          ))}
          {!self && peers.length === 0 && !error && (
            <p className="py-6 text-center text-sm text-muted-foreground">
              No peers found.
            </p>
          )}
        </ScrollArea>
      </CardContent>
    </Card>
  );
}
