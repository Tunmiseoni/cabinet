import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { OverlayStatus, ScoreSnapshot, Tailnet } from "@/lib/api";
import { AlertTriangle, Trophy } from "lucide-react";

interface LifetimeCardProps {
  scores: ScoreSnapshot | null;
  tailnet: Tailnet | null;
  overlay: OverlayStatus | null;
  onEnableOverlay: () => void;
  enablingOverlay: boolean;
}

function winRate(wins: number, games: number): string {
  if (games === 0) return "—";
  return `${Math.round((wins / games) * 100)}%`;
}

function hostnameFor(tailnet: Tailnet | null, ip: string): string {
  return tailnet?.peers.find((peer) => peer.ip === ip)?.hostname ?? ip;
}

export function LifetimeCard({
  scores,
  tailnet,
  overlay,
  onEnableOverlay,
  enablingOverlay,
}: LifetimeCardProps) {
  const totals = scores?.totals;
  const records = scores?.records ?? [];
  const overlayOff = overlay !== null && !overlay.enabled;

  return (
    <Card className="flex min-h-0 flex-col">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Trophy className="size-4" />
          Lifetime scores
        </CardTitle>
        <CardDescription>
          {totals && totals.games > 0
            ? `${totals.wins}W · ${totals.losses}L · ${totals.games} games · ${winRate(totals.wins, totals.games)} win rate`
            : "No games recorded yet — play a match to start tracking."}
        </CardDescription>
      </CardHeader>
      <CardContent className="min-h-0 flex-1">
        {overlayOff && (
          <div className="mb-3 flex items-start gap-2 rounded-md border border-amber-500/40 bg-amber-500/10 p-2 text-xs text-amber-700 dark:text-amber-400">
            <AlertTriangle className="mt-0.5 size-3.5 shrink-0" />
            <span>
              Overlay saving is off, so results cannot be tracked. Set{" "}
              <code className="font-mono">bVidSaveOverlayFiles 1</code> in{" "}
              <span className="font-mono">{overlay?.iniPath}</span>{" "}
              (close FightCade first).
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="ml-2 h-6 px-2 align-middle"
                onClick={onEnableOverlay}
                disabled={enablingOverlay}
              >
                {enablingOverlay ? "Enabling…" : "Enable"}
              </Button>
            </span>
          </div>
        )}

        {totals && totals.games > 0 && (
          <div className="mb-3 flex flex-wrap gap-2">
            <Badge variant="secondary">
              {totals.currentStreak} win streak
            </Badge>
            <Tooltip>
              <TooltipTrigger asChild>
                <Badge variant="outline">best {totals.bestStreak}</Badge>
              </TooltipTrigger>
              <TooltipContent>Longest win streak</TooltipContent>
            </Tooltip>
          </div>
        )}

        <ScrollArea className="h-64 pr-3">
          {records.map((record) => (
            <div key={record.opponent}>
              <div className="flex items-center justify-between gap-3 py-2">
                <div className="flex min-w-0 flex-col">
                  <span className="truncate font-medium">
                    {hostnameFor(tailnet, record.opponent)}
                  </span>
                  <span className="truncate font-mono text-xs text-muted-foreground">
                    {record.opponent}
                  </span>
                </div>
                <div className="flex shrink-0 items-center gap-2 text-sm">
                  <span className="font-mono">
                    {record.wins}–{record.losses}
                  </span>
                  <span className="text-xs text-muted-foreground">
                    {record.games} game{record.games === 1 ? "" : "s"}
                  </span>
                </div>
              </div>
              <Separator />
            </div>
          ))}
          {records.length === 0 && (
            <p className="py-6 text-center text-sm text-muted-foreground">
              No opponents recorded yet.
            </p>
          )}
        </ScrollArea>
      </CardContent>
    </Card>
  );
}
