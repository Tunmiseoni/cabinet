import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import type { MatchState, Room } from "@/lib/types";
import { Crown, Gamepad2, Users } from "lucide-react";

interface RoomViewProps {
  room: Room | null;
  match: MatchState | null;
}

function seatLabel(slot: number): string {
  return `P${slot}`;
}

export function RoomView({ room, match }: RoomViewProps) {
  if (!room) return null;

  const running = match?.status === "running";

  // The live netplay seat is the truth after a rotation (a promoted spectator changes slot
  // without a relaunch), so read the local nick from it rather than from the launch request.
  const mySlot =
    match?.instances.find((instance) => instance.netplay?.selfPlayer != null)?.netplay
      ?.selfPlayer ?? null;
  const myNick = mySlot != null ? (room.seats[mySlot - 1] ?? null) : null;
  const awaiting = room.awaitingCoin;
  const iAmUp = awaiting != null && awaiting === myNick;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Users className="size-4" />
          Room
        </CardTitle>
        <CardDescription>
          {room.rom} · first to {room.firstTo} · {room.phase}
          {running ? "" : " · not running"}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-2 sm:grid-cols-2">
          {([1, 2] as const).map((slot) => {
            const holder = room.seats[slot - 1];
            return (
              <div
                key={slot}
                className="flex items-center justify-between gap-3 rounded-md border p-3"
              >
                <div className="flex items-center gap-2">
                  <Gamepad2 className="size-4 text-muted-foreground" />
                  <div className="grid gap-0.5">
                    <span className="text-sm font-medium">
                      {seatLabel(slot)} · {holder ?? "open"}
                    </span>
                    {room.hostSeat === slot && (
                      <span className="text-xs text-muted-foreground">
                        room host
                      </span>
                    )}
                  </div>
                </div>
                {holder ? (
                  <Badge variant={slot === 1 ? "default" : "secondary"}>
                    seated
                  </Badge>
                ) : (
                  <Badge variant="outline">open</Badge>
                )}
              </div>
            );
          })}
        </div>

        <div className="flex items-center justify-between rounded-md border p-3">
          <span className="flex items-center gap-2 text-sm font-medium">
            <Crown className="size-4" />
            Set score
          </span>
          <span className="font-mono text-lg">
            {room.set.p1Games} – {room.set.p2Games}
            <span className="ml-3 text-sm text-muted-foreground">
              rounds {room.set.p1Rounds}–{room.set.p2Rounds}
            </span>
          </span>
        </div>

        {room.rotation && !awaiting && (
          <div className="rounded-md border border-dashed p-3 text-sm">
            Rotating:{" "}
            <span className="font-medium">
              {room.seats[room.rotation.loserSlot - 1] ?? "loser"}
            </span>{" "}
            steps out,{" "}
            <span className="font-medium">
              {room.rotation.incoming ?? "next"}
            </span>{" "}
            steps in.
          </div>
        )}

        {awaiting && (
          <div
            className={
              iAmUp
                ? "rounded-md border border-primary bg-primary/5 p-3 text-sm"
                : "rounded-md border border-dashed p-3 text-sm text-muted-foreground"
            }
          >
            {iAmUp ? (
              <>
                <span className="font-medium">You&apos;re up</span> — press start
                (num1) to challenge.
              </>
            ) : (
              <>
                Waiting for <span className="font-medium">{awaiting}</span> to coin
                in.
              </>
            )}
          </div>
        )}

        <Separator />

        <div className="grid gap-2">
          <h3 className="text-sm font-medium">Waiting</h3>
          {room.queue.length === 0 ? (
            <p className="text-xs text-muted-foreground">
              No one waiting — both players keep playing.
            </p>
          ) : (
            <ol className="grid gap-1 text-sm">
              {room.queue.map((nick, index) => (
                <li key={`${nick}-${index}`} className="flex items-center gap-2">
                  <Badge variant="outline">{index + 1}</Badge>
                  {nick}
                </li>
              ))}
            </ol>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
