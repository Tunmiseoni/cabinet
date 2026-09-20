import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";
import { LaunchCard } from "@/components/LaunchCard";
import { LifetimeCard } from "@/components/LifetimeCard";
import { MatchView } from "@/components/MatchView";
import { PeersCard } from "@/components/PeersCard";
import { RoomCard } from "@/components/RoomCard";
import { RoomsCard } from "@/components/RoomsCard";
import { RomsCard } from "@/components/RomsCard";
import { SettingsDialog } from "@/components/SettingsDialog";
import {
  enableOverlay,
  getConfig,
  getScores,
  hostRoom,
  joinRoom,
  launcherInfo,
  launchDevPair,
  launchMatch,
  leaveRoom,
  listPeers,
  listRooms,
  listRoms,
  matchStatus,
  overlayStatus,
  peersHealth,
  reportRoomResult,
  resetScores,
  roomEnqueue,
  roomLeaveQueue,
  roomSecret,
  roomState,
  setConfig as saveConfig,
  stopMatch,
  type Config,
  type DiscoveredRoom,
  type MatchRole,
  type MatchState,
  type OverlayStatus,
  type PeerHealth,
  type ProviderInfo,
  type RoomState,
  type RomIndex,
  type ScoreSnapshot,
  type Tailnet,
  MATCH_EVENT,
  ROOM_EVENT,
  SCORES_EVENT,
} from "@/lib/api";
import { healthWarning } from "@/lib/health";
import { AlertTriangle, RefreshCw, Settings, Wifi } from "lucide-react";

function sameJson(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [tailnet, setTailnet] = useState<Tailnet | null>(null);
  const [romIndex, setRomIndex] = useState<RomIndex | null>(null);
  const [health, setHealth] = useState<Record<string, PeerHealth>>({});
  const [launcher, setLauncher] = useState<ProviderInfo | null>(null);
  const [overlay, setOverlay] = useState<OverlayStatus | null>(null);
  const [overlayBusy, setOverlayBusy] = useState(false);
  const [scores, setScores] = useState<ScoreSnapshot | null>(null);
  const [rooms, setRooms] = useState<DiscoveredRoom[]>([]);
  const [roomsLoading, setRoomsLoading] = useState(true);
  const [room, setRoom] = useState<RoomState | null>(null);
  const [roomSecretValue, setRoomSecretValue] = useState<string | null>(null);
  const [roomBusy, setRoomBusy] = useState(false);
  const [match, setMatch] = useState<MatchState | null>(null);
  const [launchBusy, setLaunchBusy] = useState(false);
  const [peersLoading, setPeersLoading] = useState(true);
  const [romsLoading, setRomsLoading] = useState(true);
  const [healthLoading, setHealthLoading] = useState(false);
  const [peersError, setPeersError] = useState<string | null>(null);
  const [romsError, setRomsError] = useState<string | null>(null);
  const [appError, setAppError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showLobby, setShowLobby] = useState(false);

  const running = match?.status === "running";
  const cabinetActive = Boolean(config?.cabinetMode) && running && !showLobby;

  useEffect(() => {
    if (!running) setShowLobby(false);
  }, [running]);

  const rttWarnMs = config?.rttWarnMs ?? 150;

  const refreshPeers = useCallback(async (silent = false) => {
    if (!silent) setPeersLoading(true);
    try {
      const status = await listPeers();
      setTailnet((prev) => (sameJson(prev, status) ? prev : status));
      setPeersError(null);

      const onlineIps = status.peers
        .filter((peer) => peer.online)
        .map((peer) => peer.ip);
      if (onlineIps.length === 0) {
        setHealth((prev) => (Object.keys(prev).length === 0 ? prev : {}));
        return;
      }
      if (!silent) setHealthLoading(true);
      try {
        const results = await peersHealth(onlineIps);
        const next = Object.fromEntries(
          results.map((result) => [result.ip, result]),
        );
        setHealth((prev) => (sameJson(prev, next) ? prev : next));
      } finally {
        if (!silent) setHealthLoading(false);
      }
    } catch (err) {
      setPeersError(String(err));
    } finally {
      if (!silent) setPeersLoading(false);
    }
  }, []);

  const refreshRoms = useCallback(async () => {
    setRomsLoading(true);
    try {
      setRomIndex(await listRoms());
      setRomsError(null);
    } catch (err) {
      setRomsError(String(err));
    } finally {
      setRomsLoading(false);
    }
  }, []);

  const refreshLauncher = useCallback(async () => {
    try {
      setLauncher(await launcherInfo());
      setOverlay(await overlayStatus());
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    }
  }, []);

  const handleEnableOverlay = useCallback(async () => {
    setOverlayBusy(true);
    try {
      setOverlay(await enableOverlay());
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setOverlayBusy(false);
    }
  }, []);

  const refreshScores = useCallback(async () => {
    try {
      setScores(await getScores());
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    }
  }, []);

  const refreshRooms = useCallback(async (silent = false) => {
    if (!silent) setRoomsLoading(true);
    try {
      const next = await listRooms();
      setRooms((prev) => (sameJson(prev, next) ? prev : next));
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      if (!silent) setRoomsLoading(false);
    }
  }, []);

  useEffect(() => {
    getConfig()
      .then(setConfig)
      .catch((err) => setAppError(String(err)));
    refreshPeers();
    refreshRoms();
    refreshLauncher();
    refreshScores();
    refreshRooms();
    roomState()
      .then(setRoom)
      .catch(() => undefined);
    roomSecret()
      .then(setRoomSecretValue)
      .catch(() => undefined);
    matchStatus()
      .then(setMatch)
      .catch(() => undefined);
  }, [
    refreshPeers,
    refreshRoms,
    refreshLauncher,
    refreshScores,
    refreshRooms,
  ]);

  useEffect(() => {
    const unlisten = listen<MatchState>(MATCH_EVENT, (event) =>
      setMatch(event.payload),
    );
    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<ScoreSnapshot>(SCORES_EVENT, (event) =>
      setScores(event.payload),
    );
    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<RoomState | null>(ROOM_EVENT, (event) => {
      setRoom(event.payload);
      if (event.payload) {
        roomSecret()
          .then(setRoomSecretValue)
          .catch(() => undefined);
      } else {
        setRoomSecretValue(null);
      }
    });
    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    const seconds = config?.pollIntervalSecs ?? 10;
    const timer = setInterval(() => {
      refreshPeers(true);
      refreshRooms(true);
    }, Math.max(2, seconds) * 1000);
    return () => clearInterval(timer);
  }, [config?.pollIntervalSecs, refreshPeers, refreshRooms]);

  async function handleSaveConfig(next: Config) {
    try {
      setConfig(await saveConfig(next));
      await refreshPeers();
      await refreshRoms();
      await refreshLauncher();
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    }
  }

  async function handleLaunch(
    rom: string,
    peerIp: string,
    role: MatchRole,
    dev: boolean,
    force: boolean,
  ) {
    setLaunchBusy(true);
    try {
      setMatch(await launchMatch({ rom, peerIp, role, dev, force }));
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setLaunchBusy(false);
    }
  }

  async function handleStop() {
    setLaunchBusy(true);
    try {
      setMatch(await stopMatch());
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setLaunchBusy(false);
    }
  }

  async function handleLaunchDevPair(rom: string) {
    setLaunchBusy(true);
    try {
      setMatch(await launchDevPair(rom));
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setLaunchBusy(false);
    }
  }

  async function handleResetScores() {
    try {
      setScores(await resetScores());
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    }
  }

  async function handleHostRoom(rom: string, secret: string | null) {
    setRoomBusy(true);
    try {
      const state = await hostRoom(rom, secret);
      setRoom(state);
      setRoomSecretValue(await roomSecret());
      await refreshRooms();
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setRoomBusy(false);
    }
  }

  async function handleJoinRoom(target: DiscoveredRoom, secret: string | null) {
    setRoomBusy(true);
    try {
      setRoom(await joinRoom(target.ip, target.roomId, secret));
      setRoomSecretValue(null);
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setRoomBusy(false);
    }
  }

  async function handleLeaveRoom() {
    setRoomBusy(true);
    try {
      await leaveRoom();
      setRoom(null);
      setRoomSecretValue(null);
      await refreshRooms();
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setRoomBusy(false);
    }
  }

  async function handleRoomEnqueue() {
    setRoomBusy(true);
    try {
      await roomEnqueue();
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setRoomBusy(false);
    }
  }

  async function handleRoomLeaveQueue() {
    setRoomBusy(true);
    try {
      await roomLeaveQueue();
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setRoomBusy(false);
    }
  }

  async function handleReportRoomResult(won: boolean) {
    if (!room?.currentMatch) return;
    setRoomBusy(true);
    try {
      await reportRoomResult(room.currentMatch.matchId, won);
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    } finally {
      setRoomBusy(false);
    }
  }

  const warnings = (tailnet?.peers ?? [])
    .filter((peer) => peer.online)
    .map((peer) => healthWarning(peer.hostname, health[peer.ip], rttWarnMs))
    .filter((warning): warning is string => warning !== null);

  if (cabinetActive && match) {
    return (
      <TooltipProvider>
        <MatchView
          match={match}
          rttWarnMs={rttWarnMs}
          onShowLobby={() => setShowLobby(true)}
        />
      </TooltipProvider>
    );
  }

  return (
    <TooltipProvider>
      <div className="mx-auto flex min-h-screen max-w-5xl flex-col gap-6 p-6">
        {running && config?.cabinetMode && (
          <Alert>
            <AlertTitle>Match in progress</AlertTitle>
            <AlertDescription className="flex items-center justify-between gap-4">
              <span>The emulator is hosted in Cabinet mode.</span>
              <Button size="sm" onClick={() => setShowLobby(false)}>
                Return to Cabinet
              </Button>
            </AlertDescription>
          </Alert>
        )}
        <header className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">The Cabinet</h1>
            <p className="text-sm text-muted-foreground">
              Tailnet FightCade lobby
              {tailnet?.selfPeer?.ip ? ` · ${tailnet.selfPeer.ip}` : ""}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                refreshPeers();
                refreshRoms();
                refreshLauncher();
                refreshScores();
                refreshRooms();
              }}
            >
              <RefreshCw className="size-4" />
              Refresh
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setSettingsOpen(true)}
            >
              <Settings className="size-4" />
              Settings
            </Button>
          </div>
        </header>

        {(peersError || romsError || appError) && (
          <Alert variant="destructive">
            <AlertTriangle className="size-4" />
            <AlertTitle>Something went wrong</AlertTitle>
            <AlertDescription className="break-words">
              {[peersError, romsError, appError].filter(Boolean).join(" · ")}
            </AlertDescription>
          </Alert>
        )}

        {warnings.length > 0 && (
          <Alert>
            <Wifi className="size-4" />
            <AlertTitle>Connection health</AlertTitle>
            <AlertDescription>
              <ul className="list-disc space-y-1 pl-4">
                {warnings.map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            </AlertDescription>
          </Alert>
        )}

        <LaunchCard
          config={config}
          tailnet={tailnet}
          health={health}
          romIndex={romIndex}
          provider={launcher}
          match={match}
          busy={launchBusy}
          onLaunch={handleLaunch}
          onLaunchDevPair={handleLaunchDevPair}
          onStop={handleStop}
        />

        <div className="grid min-h-0 flex-1 gap-6 md:grid-cols-2">
          <PeersCard
            tailnet={tailnet}
            loading={peersLoading}
            error={peersError}
            health={health}
            healthLoading={healthLoading}
            rttWarnMs={rttWarnMs}
          />
          <RomsCard romIndex={romIndex} loading={romsLoading} error={romsError} />
        </div>

        {room && (
          <RoomCard
            room={room}
            selfNodeId={tailnet?.selfPeer?.nodeId ?? null}
            secret={roomSecretValue}
            busy={roomBusy}
            onEnqueue={handleRoomEnqueue}
            onLeaveQueue={handleRoomLeaveQueue}
            onLeave={handleLeaveRoom}
            onReport={handleReportRoomResult}
          />
        )}

        <RoomsCard
          rooms={rooms}
          loading={roomsLoading}
          error={null}
          inRoom={room !== null}
          romIndex={romIndex}
          tailnet={tailnet}
          onRefresh={refreshRooms}
          onHost={handleHostRoom}
          onJoin={handleJoinRoom}
        />

        <LifetimeCard
          scores={scores}
          tailnet={tailnet}
          overlay={overlay}
          onEnableOverlay={handleEnableOverlay}
          enablingOverlay={overlayBusy}
        />

        <SettingsDialog
          open={settingsOpen}
          onOpenChange={setSettingsOpen}
          config={config}
          onSave={handleSaveConfig}
          onResetScores={handleResetScores}
        />
      </div>
    </TooltipProvider>
  );
}

export default App;
