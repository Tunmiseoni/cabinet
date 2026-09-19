import { useState } from "react";
import { Badge } from "@/components/ui/badge";
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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { DiscoveredRoom, RomIndex, Tailnet } from "@/lib/api";
import { Loader2, Plus, RefreshCw, Users } from "lucide-react";

interface RoomsCardProps {
  rooms: DiscoveredRoom[];
  loading: boolean;
  error: string | null;
  inRoom: boolean;
  romIndex: RomIndex | null;
  tailnet: Tailnet | null;
  onRefresh: () => void;
  onHost: (rom: string, secret: string | null) => Promise<void>;
  onJoin: (room: DiscoveredRoom, secret: string | null) => Promise<void>;
}

function hostnameFor(tailnet: Tailnet | null, ip: string): string {
  return tailnet?.peers.find((peer) => peer.ip === ip)?.hostname ?? ip;
}

export function RoomsCard({
  rooms,
  loading,
  error,
  inRoom,
  romIndex,
  tailnet,
  onRefresh,
  onHost,
  onJoin,
}: RoomsCardProps) {
  const roms = romIndex?.roms ?? [];
  const [hostOpen, setHostOpen] = useState(false);
  const [hostRom, setHostRom] = useState("");
  const [hostSecret, setHostSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [joinTarget, setJoinTarget] = useState<DiscoveredRoom | null>(null);
  const [joinSecret, setJoinSecret] = useState("");
  const [joinBusy, setJoinBusy] = useState(false);

  async function submitHost() {
    if (!hostRom) return;
    setBusy(true);
    try {
      await onHost(hostRom, hostSecret.trim() === "" ? null : hostSecret.trim());
      setHostOpen(false);
      setHostSecret("");
    } finally {
      setBusy(false);
    }
  }

  async function submitJoin() {
    if (!joinTarget) return;
    setJoinBusy(true);
    try {
      await onJoin(joinTarget, joinSecret.trim() === "" ? null : joinSecret.trim());
      setJoinTarget(null);
      setJoinSecret("");
    } finally {
      setJoinBusy(false);
    }
  }

  return (
    <Card className="flex min-h-0 flex-col">
      <CardHeader>
        <div className="flex items-start justify-between gap-3">
          <div>
            <CardTitle className="flex items-center gap-2">
              <Users className="size-4" />
              Rooms
            </CardTitle>
            <CardDescription>
              {loading
                ? "Probing the tailnet…"
                : rooms.length === 0
                  ? "No rooms — host one to get started"
                  : `${rooms.length} room${rooms.length === 1 ? "" : "s"} on the tailnet`}
            </CardDescription>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button
              variant="ghost"
              size="icon"
              onClick={onRefresh}
              disabled={loading}
              aria-label="Refresh rooms"
            >
              {loading ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <RefreshCw className="size-4" />
              )}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setHostRom(roms[0]?.shortName ?? "");
                setHostOpen(true);
              }}
              disabled={inRoom}
            >
              <Plus className="size-4" />
              Host
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent className="min-h-0 flex-1">
        {error && (
          <p className="pb-3 text-xs text-destructive">{error}</p>
        )}
        {inRoom && (
          <p className="pb-3 text-xs text-muted-foreground">
            Leave your current room before joining another.
          </p>
        )}
        <div className="grid gap-2">
          {rooms.map((room) => (
            <div
              key={`${room.ip}:${room.roomId}`}
              className="flex items-center justify-between gap-3 rounded-md border p-3"
            >
              <div className="flex min-w-0 flex-col">
                <span className="truncate font-medium">
                  {room.host}
                  <span className="ml-2 text-xs text-muted-foreground">
                    {hostnameFor(tailnet, room.ip)}
                  </span>
                </span>
                <span className="truncate font-mono text-xs text-muted-foreground">
                  {room.rom} · {room.players} player
                  {room.players === 1 ? "" : "s"} · {room.queue} queued
                </span>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <Badge
                  variant={room.phase === "playing" ? "default" : "secondary"}
                >
                  {room.phase === "playing" ? "playing" : "lobby"}
                </Badge>
                <Button
                  size="sm"
                  onClick={() => {
                    setJoinTarget(room);
                    setJoinSecret("");
                  }}
                  disabled={inRoom}
                >
                  Join
                </Button>
              </div>
            </div>
          ))}
          {rooms.length === 0 && !loading && !error && (
            <p className="py-6 text-center text-sm text-muted-foreground">
              Nobody is hosting right now.
            </p>
          )}
        </div>
      </CardContent>

      <Dialog open={hostOpen} onOpenChange={setHostOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Host a room</DialogTitle>
            <DialogDescription>
              Opens a king-of-the-hill room discoverable by your tailnet peers.
              Share the secret so others can join.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4">
            <div className="grid gap-2">
              <Label htmlFor="hostRom">Game</Label>
              <Select value={hostRom} onValueChange={setHostRom}>
                <SelectTrigger id="hostRom">
                  <SelectValue placeholder="Select a ROM" />
                </SelectTrigger>
                <SelectContent>
                  {roms.map((rom) => (
                    <SelectItem key={rom.path} value={rom.shortName}>
                      {rom.shortName}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-2">
              <Label htmlFor="hostSecret">Secret (optional)</Label>
              <Input
                id="hostSecret"
                value={hostSecret}
                placeholder="Leave blank to generate one"
                onChange={(event) => setHostSecret(event.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setHostOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => void submitHost()} disabled={busy || !hostRom}>
              {busy ? "Hosting…" : "Host room"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={joinTarget !== null}
        onOpenChange={(open) => !open && setJoinTarget(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Join {joinTarget?.host}&apos;s room</DialogTitle>
            <DialogDescription>
              {joinTarget?.rom} on {joinTarget?.ip}
              {joinTarget?.secretRequired ? " · secret required" : ""}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-2">
            <Label htmlFor="joinSecret">Room secret</Label>
            <Input
              id="joinSecret"
              value={joinSecret}
              placeholder="Ask the host for the secret"
              onChange={(event) => setJoinSecret(event.target.value)}
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setJoinTarget(null)}>
              Cancel
            </Button>
            <Button onClick={() => void submitJoin()} disabled={joinBusy}>
              {joinBusy ? "Joining…" : "Join room"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}
