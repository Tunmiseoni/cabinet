import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { RoomView } from "@/components/RoomView";
import {
  lobbyDiscover,
  lobbyJoin,
  lobbyRoom,
  lobbyStart,
  lobbyStatus,
  lobbyStop,
  matchStatus,
  stopMatch,
} from "@/lib/api";
import { useAsyncTask } from "@/lib/hooks";
import { useInvoke } from "@/lib/query";
import type {
  Config,
  MatchState,
  ProviderInfo,
  Room,
  RomIndex,
} from "@/lib/types";
import { DoorOpen, Server, Square, Users } from "lucide-react";

const FIRST_TO = [1, 2, 3];

type HostSeat = "1" | "2" | "spectate";

function hostSeatValue(seat: HostSeat): number | null {
  return seat === "spectate" ? null : Number(seat);
}

function roomSummary(room: Room): string {
  return `${room.rom} · ${room.phase} · ${room.players}/2 players`;
}

interface LobbyCardProps {
  config: Config | null;
  romIndex: RomIndex | null;
  provider: ProviderInfo | null;
  match: MatchState | null;
  onMatch: (state: MatchState) => void;
  onError: (message: string | null) => void;
}

export function LobbyCard({
  config,
  romIndex,
  provider,
  match,
  onMatch,
  onError,
}: LobbyCardProps) {
  const [rom, setRom] = useState("");
  const [firstTo, setFirstTo] = useState("2");
  const [hostSeat, setHostSeat] = useState<HostSeat>("1");
  const [manualHost, setManualHost] = useState("");
  const [manualRom, setManualRom] = useState("");

  const action = useAsyncTask(onError);

  const roomQuery = useInvoke("lobby-room", lobbyStatus, { pollMs: 2000 });
  const hosting = roomQuery.data;
  const roomsQuery = useInvoke("lobby-rooms", lobbyDiscover, {
    pollMs: 5000,
    enabled: !hosting,
  });
  const liveRoomQuery = useInvoke("lobby-live-room", lobbyRoom, {
    pollMs: 2000,
    enabled: !hosting,
  });

  const roms = romIndex?.roms ?? [];
  const rooms = roomsQuery.data ?? [];
  const running = match?.status === "running";
  const retroarch = provider?.kind === "retroarch";

  useEffect(() => {
    if (!rom && roms.length > 0) setRom(roms[0].shortName);
  }, [rom, roms]);

  const busy = action.busy;

  async function host() {
    const room = await lobbyStart({
      rom,
      firstTo: Number(firstTo),
      hostSeat: hostSeatValue(hostSeat),
    });
    roomQuery.mutate(room);
    onMatch(await matchStatus());
  }

  async function stopHosting() {
    await lobbyStop();
    roomQuery.mutate(null);
    onMatch(await matchStatus());
  }

  async function stopSession() {
    try {
      await lobbyStop();
    } catch {
      await stopMatch();
    }
    roomQuery.mutate(null);
    onMatch(await matchStatus());
  }

  // A single Join: the backend seats you in a free player slot, or makes you a spectator when
  // the room is full (the set-end rotation promotes the longest-waiting spectator).
  async function join(host: string, rom?: string) {
    const outcome = await lobbyJoin({
      host,
      rom: rom && rom.trim() !== "" ? rom : null,
    });
    onMatch(outcome.state);
  }

  const manualReady = manualHost.trim() !== "";

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Users className="size-4" />
          Lobby
        </CardTitle>
        <CardDescription>
          {retroarch
            ? "Host a RetroArch room, or join one on the tailnet."
            : "The lobby needs the RetroArch provider."}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-6">
        {running && (
          <RoomView room={hosting ?? liveRoomQuery.data} match={match} />
        )}

        {running && (
          <section className="flex flex-wrap items-center justify-between gap-3 rounded-md border p-3">
            <div className="grid gap-1">
              <span className="text-sm font-medium">Match running</span>
              <span className="text-xs text-muted-foreground">
                {match?.rom}
                {match?.peerIp ? ` · ${match.peerIp}` : ""}
              </span>
            </div>
            <Button
              variant="destructive"
              size="sm"
              disabled={busy}
              onClick={() => void action.run(stopSession)}
            >
              <Square className="size-4" />
              Stop match
            </Button>
          </section>
        )}

        <section className="grid gap-3">
          <h3 className="flex items-center gap-2 text-sm font-medium">
            <Server className="size-4" />
            Hosting
          </h3>
          {hosting ? (
            <div className="flex flex-wrap items-center justify-between gap-3 rounded-md border p-3">
              <div className="grid gap-1">
                <span className="text-sm font-medium">
                  {hosting.hostHandle} · {roomSummary(hosting)}
                </span>
                <span className="text-xs text-muted-foreground">
                  Joiners connect to{" "}
                  {config?.defaultPeerIp ?? "this host"}. Share the address if discovery
                  is blocked.
                </span>
              </div>
              <Button
                variant="destructive"
                size="sm"
                disabled={busy}
                onClick={() => void action.run(stopHosting)}
              >
                Stop hosting
              </Button>
            </div>
          ) : (
            <div className="grid gap-3 sm:grid-cols-[1fr_auto_auto_auto]">
              <div className="grid gap-2">
                <Label>ROM</Label>
                <Select value={rom} onValueChange={setRom} disabled={running || busy}>
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
                <Label>First to</Label>
                <Select value={firstTo} onValueChange={setFirstTo} disabled={busy}>
                  <SelectTrigger className="w-20">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {FIRST_TO.map((n) => (
                      <SelectItem key={n} value={String(n)}>
                        {n}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="grid gap-2">
                <Label>Play as</Label>
                <Select
                  value={hostSeat}
                  onValueChange={(value) => setHostSeat(value as HostSeat)}
                  disabled={running || busy}
                >
                  <SelectTrigger className="w-36">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="1">Player 1</SelectItem>
                    <SelectItem value="2">Player 2</SelectItem>
                    <SelectItem value="spectate">Spectate (table)</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex items-end">
                <Button
                  disabled={!retroarch || rom === "" || running || busy}
                  onClick={() => void action.run(host)}
                >
                  <Server className="size-4" />
                  Host room
                </Button>
              </div>
            </div>
          )}
        </section>

        {!hosting && (
          <section className="grid gap-3">
            <h3 className="flex items-center gap-2 text-sm font-medium">
              <DoorOpen className="size-4" />
              Rooms
            </h3>
            {rooms.length === 0 ? (
              <p className="text-xs text-muted-foreground">
                {roomsQuery.loading ? "Searching the tailnet…" : "No rooms found."}
              </p>
            ) : (
              <ul className="grid gap-2">
                {rooms.map((entry) => (
                  <li
                    key={entry.room.roomId}
                    className="flex flex-wrap items-center justify-between gap-3 rounded-md border p-3"
                  >
                    <div className="grid gap-1">
                      <span className="text-sm font-medium">
                        {entry.room.hostHandle} · {roomSummary(entry.room)}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {entry.hostHostname} · {entry.hostIp}
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <Badge variant="secondary">first to {entry.room.firstTo}</Badge>
                      {entry.room.players >= 2 && (
                        <Badge variant="outline">full · join as spectator</Badge>
                      )}
                      <Button
                        size="sm"
                        disabled={running || busy}
                        onClick={() => void action.run(() => join(entry.hostIp))}
                      >
                        Join
                      </Button>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}

        {!hosting && (
          <section className="grid gap-3">
            <h3 className="text-sm font-medium">Join by address</h3>
            <div className="grid gap-3 sm:grid-cols-[1fr_1fr_auto]">
              <div className="grid gap-2">
                <Label>Host</Label>
                <Input
                  value={manualHost}
                  placeholder="100.64.0.2"
                  disabled={running || busy}
                  onChange={(event) => setManualHost(event.target.value)}
                />
              </div>
              <div className="grid gap-2">
                <Label>ROM</Label>
                <Input
                  value={manualRom}
                  placeholder="sfiii3nr1"
                  disabled={running || busy}
                  onChange={(event) => setManualRom(event.target.value)}
                />
              </div>
              <div className="flex items-end">
                <Button
                  disabled={!retroarch || !manualReady || running || busy}
                  onClick={() =>
                    void action.run(() => join(manualHost.trim(), manualRom))
                  }
                >
                  Join
                </Button>
              </div>
            </div>
          </section>
        )}
      </CardContent>
    </Card>
  );
}
