import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { NetplayConnection, NetplayInfo } from "@/lib/types";

const VARIANT: Record<
  NetplayConnection,
  "default" | "secondary" | "destructive" | "outline"
> = {
  connected: "default",
  connecting: "secondary",
  disconnected: "outline",
  failed: "destructive",
};

const STATUS: Record<NetplayConnection, string> = {
  connecting: "netplay…",
  connected: "netplay",
  disconnected: "netplay ended",
  failed: "netplay failed",
};

function badgeLabel(netplay: NetplayInfo, prefix: string): string {
  const head = prefix ? `${prefix} · ` : "";
  const status =
    netplay.connection === "connected" && netplay.pingMs !== null
      ? `${netplay.pingMs} ms`
      : STATUS[netplay.connection];
  return `${head}${status}`;
}

function tooltip(netplay: NetplayInfo): string {
  const lines = netplay.events.slice(-5).map((event) => event.text);
  if (netplay.coreWarning) lines.unshift("core version mismatch with a peer");
  return lines.length > 0 ? lines.join("\n") : "no netplay events yet";
}

interface NetplayBadgeProps {
  netplay: NetplayInfo | null | undefined;
  label?: string;
}

export function NetplayBadge({ netplay, label = "" }: NetplayBadgeProps) {
  if (!netplay) return null;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Badge
          variant={VARIANT[netplay.connection]}
          className="cursor-default font-mono tabular-nums"
        >
          {badgeLabel(netplay, label)}
        </Badge>
      </TooltipTrigger>
      <TooltipContent className="max-w-sm whitespace-pre-wrap font-mono text-xs">
        {tooltip(netplay)}
      </TooltipContent>
    </Tooltip>
  );
}
