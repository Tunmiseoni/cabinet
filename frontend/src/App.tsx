import { useEffect, useMemo, useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";
import { LaunchCard } from "@/components/LaunchCard";
import { LobbyCard } from "@/components/LobbyCard";
import { MatchView } from "@/components/MatchView";
import { PeersCard } from "@/components/PeersCard";
import { RomsCard } from "@/components/RomsCard";
import { SettingsDialog } from "@/components/SettingsDialog";
import { Toaster } from "@/components/ui/sonner";
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
} from "@/lib/api";
import { healthWarning } from "@/lib/health";
import { useAsyncTask, useTauriEvent } from "@/lib/hooks";
import { useInvoke } from "@/lib/query";
import {
  MATCH_EVENT,
  type Config,
  type MatchRole,
  type MatchState,
  type PeerHealth,
} from "@/lib/types";
import { AlertTriangle, RefreshCw, Settings, Wifi } from "lucide-react";

function App() {
  const [appError, setAppError] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showLobby, setShowLobby] = useState(false);

  const launchAction = useAsyncTask(setAppError);
  const settingsAction = useAsyncTask(setAppError);

  const configQuery = useInvoke("config", getConfig);
  const config = configQuery.data;

  const pollSeconds = Math.max(2, config?.pollIntervalSecs ?? 10);
  const peersQuery = useInvoke("peers", listPeers, {
    pollMs: pollSeconds * 1000,
  });
  const romsQuery = useInvoke("roms", listRoms);
  const launcherQuery = useInvoke("launcher", launcherInfo);
  const matchQuery = useInvoke("match", matchStatus);
  const match = matchQuery.data;

  useTauriEvent<MatchState>(MATCH_EVENT, (next) => matchQuery.mutate(next));

  const onlineIps = useMemo(
    () =>
      (peersQuery.data?.peers ?? [])
        .filter((peer) => peer.online)
        .map((peer) => peer.ip),
    [peersQuery.data],
  );
  const healthQuery = useInvoke(
    `health:${onlineIps.join(",")}`,
    () => peersHealth(onlineIps),
    { enabled: onlineIps.length > 0 },
  );
  const health = useMemo<Record<string, PeerHealth>>(
    () =>
      Object.fromEntries(
        (healthQuery.data ?? []).map((result) => [result.ip, result]),
      ),
    [healthQuery.data],
  );

  const running = match?.status === "running";
  const cabinetActive = Boolean(config?.cabinetMode) && running && !showLobby;
  const rttWarnMs = config?.rttWarnMs ?? 150;

  useEffect(() => {
    if (!running) setShowLobby(false);
  }, [running]);

  const refreshAll = () => {
    void peersQuery.refresh();
    void romsQuery.refresh();
    void launcherQuery.refresh();
  };

  const handleSaveConfig = (next: Config) =>
    settingsAction.run(async () => {
      configQuery.mutate(await saveConfig(next));
      await Promise.all([
        peersQuery.refresh(),
        romsQuery.refresh(),
        launcherQuery.refresh(),
      ]);
    });

  const handleLaunch = (
    rom: string,
    peerIp: string,
    role: MatchRole,
    dev: boolean,
    force: boolean,
  ) =>
    launchAction.run(async () => {
      matchQuery.mutate(await launchMatch({ rom, peerIp, role, dev, force }));
    });

  const handleStop = () =>
    launchAction.run(async () => {
      matchQuery.mutate(await stopMatch());
    });

  const handleLaunchDevPair = (rom: string) =>
    launchAction.run(async () => {
      matchQuery.mutate(await launchDevPair(rom));
    });

  const warnings = (peersQuery.data?.peers ?? [])
    .filter((peer) => peer.online)
    .map((peer) => healthWarning(peer.hostname, health[peer.ip], rttWarnMs))
    .filter((warning): warning is string => warning !== null);

  const errorMessage = [
    peersQuery.error,
    romsQuery.error,
    launcherQuery.error,
    appError,
  ]
    .filter(Boolean)
    .join(" · ");

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
              {peersQuery.data?.selfPeer?.ip
                ? ` · ${peersQuery.data.selfPeer.ip}`
                : ""}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={refreshAll}>
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

        {errorMessage && (
          <Alert variant="destructive">
            <AlertTriangle className="size-4" />
            <AlertTitle>Something went wrong</AlertTitle>
            <AlertDescription className="break-words">
              {errorMessage}
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

        <LobbyCard
          config={config}
          romIndex={romsQuery.data}
          provider={launcherQuery.data}
          match={match}
          onMatch={(state) => matchQuery.mutate(state)}
          onError={setAppError}
        />

        {config?.developerMode && (
          <LaunchCard
            config={config}
            tailnet={peersQuery.data}
            health={health}
            romIndex={romsQuery.data}
            provider={launcherQuery.data}
            match={match}
            busy={launchAction.busy}
            onLaunch={handleLaunch}
            onLaunchDevPair={handleLaunchDevPair}
            onStop={handleStop}
          />
        )}

        <div className="grid min-h-0 flex-1 gap-6 md:grid-cols-2">
          <PeersCard
            tailnet={peersQuery.data}
            loading={peersQuery.loading}
            error={peersQuery.error}
            health={health}
            healthLoading={healthQuery.loading}
            rttWarnMs={rttWarnMs}
          />
          <RomsCard
            romIndex={romsQuery.data}
            loading={romsQuery.loading}
            error={romsQuery.error}
          />
        </div>

        <SettingsDialog
          open={settingsOpen}
          onOpenChange={setSettingsOpen}
          config={config}
          onSave={handleSaveConfig}
        />

        <Toaster position="bottom-right" />
      </div>
    </TooltipProvider>
  );
}

export default App;
