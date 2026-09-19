import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import type { RoomPlayer, RoomState } from "@/lib/api";
import { Crown, LogOut, Swords, Trophy, UserPlus } from "lucide-react";

interface RoomCardProps {
  room: RoomState;
  selfNodeId: string | null;
  secret: string | null;
  busy: boolean;
  onEnqueue: () => Promise<void>;
  onLeaveQueue: () => Promise<void>;
  onLeave: () => Promise<void>;
  onReport: (won: boolean) => Promise<void>;
}

function name(player: RoomPlayer | null): string {
  return player ? player.handle : "—";
}

function isSelf(player: RoomPlayer | null, selfNodeId: string | null): boolean {
  return Boolean(player && selfNodeId && player.nodeId === selfNodeId);
}

export function RoomCard({
  room,
  selfNodeId,
  secret,
  busy,
  onEnqueue,
  onLeaveQueue,
  onLeave,
  onReport,
}: RoomCardProps) {
  const selfInQueue = room.queue.some((player) => player.nodeId === selfNodeId);
  const selfIsChampion = isSelf(room.champion, selfNodeId);
  const selfIsChallenger = isSelf(room.challenger, selfNodeId);
  const selfInMatch = Boolean(
    room.currentMatch &&
      (isSelf(room.currentMatch.p1, selfNodeId) ||
        isSelf(room.currentMatch.p2, selfNodeId)),
  );
  const known = selfInQueue || selfIsChampion || selfIsChallenger || selfInMatch;

  const ledger = Object.entries(room.ledger).sort(
    (a, b) => b[1].wins - a[1].wins || b[1].games - a[1].games,
  );

  return (
    <Card className="flex min-h-0 flex-col">
      <CardHeader>
        <div className="flex items-start justify-between gap-3">
          <div>
            <CardTitle className="flex items-center gap-2">
              <Trophy className="size-4" />
              {room.rom}
            </CardTitle>
            <CardDescription>
              Hosted by {room.host.handle}
              {secret ? ` · secret ${secret}` : ""}
            </CardDescription>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Badge variant={room.phase === "playing" ? "default" : "secondary"}>
              {room.phase}
            </Badge>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void onLeave()}
              disabled={busy}
            >
              <LogOut className="size-4" />
              Leave
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-2">
          <div className="flex items-center justify-between gap-3 rounded-md border p-3">
            <div className="flex items-center gap-2">
              <Crown className="size-4 text-amber-500" />
              <span className="text-sm text-muted-foreground">Champion</span>
            </div>
            <span className="font-medium">{name(room.champion)}</span>
          </div>
          <div className="flex items-center justify-between gap-3 rounded-md border p-3">
            <div className="flex items-center gap-2">
              <Swords className="size-4 text-muted-foreground" />
              <span className="text-sm text-muted-foreground">Challenger</span>
            </div>
            <span className="font-medium">{name(room.challenger)}</span>
          </div>
        </div>

        {room.currentMatch && (
          <div className="rounded-md border border-primary/40 bg-primary/5 p-3">
            <div className="mb-1 text-xs font-medium text-muted-foreground">
              {selfInMatch ? "You are in this match" : "Match in progress"}
            </div>
            <div className="flex items-center justify-center gap-3 font-medium">
              <span>{room.currentMatch.p1.handle}</span>
              <span className="text-xs text-muted-foreground">vs</span>
              <span>{room.currentMatch.p2.handle}</span>
            </div>
            {selfInMatch && (
              <div className="mt-3 flex items-center justify-center gap-2">
                <Button
                  size="sm"
                  onClick={() => void onReport(true)}
                  disabled={busy}
                >
                  I won
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void onReport(false)}
                  disabled={busy}
                >
                  I lost
                </Button>
              </div>
            )}
          </div>
        )}

        <div className="flex items-center justify-between gap-3">
          <div className="text-sm text-muted-foreground">
            {room.queue.length === 0
              ? "Queue is empty"
              : `${room.queue.length} in queue: ${room.queue
                  .map((player) => player.handle)
                  .join(", ")}`}
          </div>
          {!known && !room.currentMatch && (
            <Button size="sm" onClick={() => void onEnqueue()} disabled={busy}>
              <UserPlus className="size-4" />
              Join queue
            </Button>
          )}
          {selfInQueue && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => void onLeaveQueue()}
              disabled={busy}
            >
              Leave queue
            </Button>
          )}
        </div>

        {ledger.length > 0 && (
          <>
            <Separator />
            <div className="grid gap-1">
              <span className="text-xs font-medium text-muted-foreground">
                Scoreboard
              </span>
              {ledger.map(([nodeId, entry]) => (
                <div
                  key={nodeId}
                  className="flex items-center justify-between gap-3 py-1 text-sm"
                >
                  <span className="truncate font-medium">{entry.handle}</span>
                  <span className="shrink-0 font-mono">
                    {entry.wins}–{entry.losses}
                    <span className="ml-2 text-xs text-muted-foreground">
                      {entry.games} game{entry.games === 1 ? "" : "s"}
                    </span>
                  </span>
                </div>
              ))}
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
