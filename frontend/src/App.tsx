import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";
import { LaunchCard } from "@/components/LaunchCard";
import { LifetimeCard } from "@/components/LifetimeCard";
import { PeersCard } from "@/components/PeersCard";
import { RomsCard } from "@/components/RomsCard";
import { SettingsDialog } from "@/components/SettingsDialog";
import {
  getConfig,
  getScores,
  launcherInfo,
  launchDevPair,
  launchMatch,
  listPeers,
  listRoms,
  matchStatus,
  overlayStatus,
  peersHealth,
  resetScores,
  setConfig as saveConfig,
  stopMatch,
  type Config,
  type InstallInfo,
  type MatchState,
  type OverlayStatus,
  type PeerHealth,
  type RomIndex,
  type ScoreSnapshot,
  type Tailnet,
  MATCH_EVENT,
  SCORES_EVENT,
} from "@/lib/api";
import { healthWarning } from "@/lib/health";
import { AlertTriangle, RefreshCw, Settings, Wifi } from "lucide-react";

function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [tailnet, setTailnet] = useState<Tailnet | null>(null);
  const [romIndex, setRomIndex] = useState<RomIndex | null>(null);
  const [health, setHealth] = useState<Record<string, PeerHealth>>({});
  const [launcher, setLauncher] = useState<InstallInfo | null>(null);
  const [overlay, setOverlay] = useState<OverlayStatus | null>(null);
  const [scores, setScores] = useState<ScoreSnapshot | null>(null);
  const [match, setMatch] = useState<MatchState | null>(null);
  const [launchBusy, setLaunchBusy] = useState(false);
  const [peersLoading, setPeersLoading] = useState(true);
  const [romsLoading, setRomsLoading] = useState(true);
  const [healthLoading, setHealthLoading] = useState(false);
  const [peersError, setPeersError] = useState<string | null>(null);
  const [romsError, setRomsError] = useState<string | null>(null);
  const [appError, setAppError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const rttWarnMs = config?.rttWarnMs ?? 150;

  const refreshPeers = useCallback(async () => {
    setPeersLoading(true);
    try {
      const status = await listPeers();
      setTailnet(status);
      setPeersError(null);

      const onlineIps = status.peers
        .filter((peer) => peer.online)
        .map((peer) => peer.ip);
      if (onlineIps.length === 0) {
        setHealth({});
        return;
      }
      setHealthLoading(true);
      try {
        const results = await peersHealth(onlineIps);
        setHealth(
          Object.fromEntries(results.map((result) => [result.ip, result])),
        );
      } finally {
        setHealthLoading(false);
      }
    } catch (err) {
      setPeersError(String(err));
    } finally {
      setPeersLoading(false);
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

  const refreshScores = useCallback(async () => {
    try {
      setScores(await getScores());
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
    refreshScores();
    matchStatus()
      .then(setMatch)
      .catch(() => undefined);
  }, [refreshPeers, refreshRoms, refreshLauncher, refreshScores]);

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
    const seconds = config?.pollIntervalSecs ?? 10;
    const timer = setInterval(refreshPeers, Math.max(2, seconds) * 1000);
    return () => clearInterval(timer);
  }, [config?.pollIntervalSecs, refreshPeers]);

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
    side: number,
    dev: boolean,
  ) {
    setLaunchBusy(true);
    try {
      setMatch(await launchMatch({ rom, peerIp, side, dev }));
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

  const warnings = (tailnet?.peers ?? [])
    .filter((peer) => peer.online)
    .map((peer) => healthWarning(peer.hostname, health[peer.ip], rttWarnMs))
    .filter((warning): warning is string => warning !== null);

  return (
    <TooltipProvider>
      <div className="mx-auto flex min-h-screen max-w-5xl flex-col gap-6 p-6">
        <header className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">cabinet</h1>
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
          launcher={launcher}
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

        <LifetimeCard scores={scores} tailnet={tailnet} overlay={overlay} />

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
