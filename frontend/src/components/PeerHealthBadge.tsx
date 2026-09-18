import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { PeerHealth } from "@/lib/api";
import { healthLabel, healthLevel, type HealthLevel } from "@/lib/health";

const VARIANT: Record<HealthLevel, "default" | "destructive" | "outline"> = {
  healthy: "default",
  warn: "destructive",
  unreachable: "outline",
  unknown: "outline",
};

interface PeerHealthBadgeProps {
  health: PeerHealth | undefined;
  rttWarnMs: number;
  loading: boolean;
}

export function PeerHealthBadge({
  health,
  rttWarnMs,
  loading,
}: PeerHealthBadgeProps) {
  const level = healthLevel(health, rttWarnMs);

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Badge
          variant={VARIANT[level]}
          className="cursor-default font-mono tabular-nums"
        >
          {health ? healthLabel(health) : loading ? "checking…" : "—"}
        </Badge>
      </TooltipTrigger>
      <TooltipContent className="max-w-xs whitespace-pre-wrap">
        {health?.raw || "No ping result yet."}
      </TooltipContent>
    </Tooltip>
  );
}
