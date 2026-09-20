import { useCallback, useEffect, useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";
import { LaunchCard } from "@/components/LaunchCard";
import { MatchView } from "@/components/MatchView";
import { PeersCard } from "@/components/PeersCard";
import { RomsCard } from "@/components/RomsCard";
import { SettingsDialog } from "@/components/SettingsDialog";
import {
  getConfig,
  launcherInfo,
  launchDevPair,
  launchMatch,
  listPeers,
  listRoms,
  matchStatus,
  peersHealth,
  setConfig as saveConfig,
  stopMatch,
  type Config,
  type MatchRole,
  type MatchState,
  type PeerHealth,
  type ProviderInfo,
  type RomIndex,
  type Tailnet,
  MATCH_EVENT,
} from "@/lib/api";
import { healthWarning } from "@/lib/health";
import { useAsyncTask, usePolling, useTauriEvent } from "@/lib/hooks";
import { sameJson } from "@/lib/utils";
import { AlertTriangle, RefreshCw, Settings, Wifi } from "lucide-react";

function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [tailnet, setTailnet] = useState<Tailnet | null>(null);
  const [romIndex, setRomIndex] = useState<RomIndex | null>(null);
  const [health, setHealth] = useState<Record<string, PeerHealth>>({});
  const [launcher, setLauncher] = useState<ProviderInfo | null>(null);
  const [match, setMatch] = useState<MatchState | null>(null);
  const [peersLoading, setPeersLoading] = useState(true);
  const [romsLoading, setRomsLoading] = useState(true);
  const [healthLoading, setHealthLoading] = useState(false);
  const [peersError, setPeersError] = useState<string | null>(null);
  const [romsError, setRomsError] = useState<string | null>(null);
  const [appError, setAppError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showLobby, setShowLobby] = useState(false);

  const launchAction = useAsyncTask(setAppError);
  const settingsAction = useAsyncTask(setAppError);

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
      setAppError(null);
    } catch (err) {
      setAppError(String(err));
    }
  }, []);

  useEffect(() => {
    getConfig()
      .then(setConfig)
      .catch((err) => setAppError(String(err)));
    refreshPeers();
    refreshRoms();
    refreshLauncher();
    matchStatus()
      .then(setMatch)
      .catch(() => undefined);
  }, [refreshPeers, refreshRoms, refreshLauncher]);

  useTauriEvent<MatchState>(MATCH_EVENT, setMatch);

  const pollSeconds = Math.max(2, config?.pollIntervalSecs ?? 10);
  usePolling(() => {
    refreshPeers(true);
  }, pollSeconds * 1000);

  const handleSaveConfig = (next: Config) =>
    settingsAction.run(async () => {
      setConfig(await saveConfig(next));
      await refreshPeers();
      await refreshRoms();
      await refreshLauncher();
    });

  const handleLaunch = (
    rom: string,
    peerIp: string,
    role: MatchRole,
    dev: boolean,
    force: boolean,
  ) =>
    launchAction.run(async () => {
      setMatch(await launchMatch({ rom, peerIp, role, dev, force }));
    });

  const handleStop = () =>
    launchAction.run(async () => {
      setMatch(await stopMatch());
    });

  const handleLaunchDevPair = (rom: string) =>
    launchAction.run(async () => {
      setMatch(await launchDevPair(rom));
    });

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
              Tailnet FightCade launcher
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
          busy={launchAction.busy}
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

        <SettingsDialog
          open={settingsOpen}
          onOpenChange={setSettingsOpen}
          config={config}
          onSave={handleSaveConfig}
        />
      </div>
    </TooltipProvider>
  );
}

export default App;
