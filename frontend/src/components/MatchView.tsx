import { useCallback, useEffect, useRef, useState } from "react";
import {
  PhysicalPosition,
  PhysicalSize,
  currentMonitor,
  getCurrentWindow,
} from "@tauri-apps/api/window";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { PeerHealthBadge } from "@/components/PeerHealthBadge";
import {
  cabinetPlace,
  cabinetRelease,
  cabinetRequestPermission,
  cabinetStatus,
  type CabinetRect,
  type CabinetStatus,
  type MatchState,
} from "@/lib/api";
import { X } from "lucide-react";

const PERMISSION_MARKER = "accessibility-permission-required";
const REASSERT_MS = 2000;
const FIND_TIMEOUT_MS = 25_000;

interface MatchViewProps {
  match: MatchState;
  rttWarnMs: number;
  onShowLobby: () => void;
}

function describeResult(match: MatchState): string | null {
  const result = match.result;
  if (!result) return null;
  const score = `${result.p1Score ?? "?"}–${result.p2Score ?? "?"}`;
  if (result.winnerSide === null) return `draw · ${score}`;
  const winner =
    result.winnerSide === 0 ? (result.p1Name ?? "P1") : (result.p2Name ?? "P2");
  return `${winner} wins · ${score}`;
}

interface ShellState {
  size: PhysicalSize;
  position: PhysicalPosition;
  decorated: boolean;
}

export function MatchView({ match, rttWarnMs, onShowLobby }: MatchViewProps) {
  const [status, setStatus] = useState<CabinetStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [windowId, setWindowId] = useState<number | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [broadcast, setBroadcast] = useState(false);
  const [timedOut, setTimedOut] = useState(false);
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const windowIdRef = useRef<number | null>(null);
  const shellRef = useRef<ShellState | null>(null);

  windowIdRef.current = windowId;

  const measure = useCallback(async (): Promise<CabinetRect | null> => {
    const element = viewportRef.current;
    if (!element) return null;
    const box = element.getBoundingClientRect();
    const win = getCurrentWindow();
    const [scale, inner] = await Promise.all([
      win.scaleFactor(),
      win.innerPosition(),
    ]);
    return {
      x: inner.x / scale + box.left,
      y: inner.y / scale + box.top,
      width: box.width,
      height: box.height,
    };
  }, []);

  const place = useCallback(async () => {
    const id = windowIdRef.current;
    if (id === null) return;
    const rect = await measure();
    if (!rect) return;
    try {
      await cabinetPlace(id, rect);
      setMessage(null);
    } catch (err) {
      const text = String(err);
      if (text.includes(PERMISSION_MARKER)) {
        setMessage("Accessibility permission is required to host the game here.");
      } else if (text.includes("not part of the current match")) {
        setWindowId(null);
        setMessage(null);
      } else {
        setMessage(text);
      }
    }
  }, [measure]);

  const attach = useCallback(async () => {
    try {
      const next = await cabinetStatus();
      setStatus(next);
      setError(null);
      const matchWindows = next.windows;
      setWindowId((current) => {
        if (current !== null && matchWindows.some((w) => w.id === current)) {
          return current;
        }
        if (
          current !== null &&
          !matchWindows.some((w) => w.id === current)
        ) {
          return null;
        }
        if (
          next.supported &&
          next.permission === "granted" &&
          matchWindows.length > 0
        ) {
          return matchWindows[0].id;
        }
        return null;
      });
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    void attach();
    const timer = setInterval(() => void attach(), 1500);
    return () => clearInterval(timer);
  }, [attach]);

  useEffect(() => {
    if (windowId !== null || error) return;
    const timer = setTimeout(() => {
      if (windowIdRef.current === null) setTimedOut(true);
    }, FIND_TIMEOUT_MS);
    return () => clearTimeout(timer);
  }, [windowId, error]);

  useEffect(() => {
    if (windowId === null) return;
    void place();
    const timer = setInterval(() => void place(), REASSERT_MS);
    const element = viewportRef.current;
    const observer = element ? new ResizeObserver(() => void place()) : null;
    if (element && observer) observer.observe(element);
    return () => {
      clearInterval(timer);
      observer?.disconnect();
    };
  }, [windowId, place]);

  useEffect(() => {
    if (windowId === null) return;
    const win = getCurrentWindow();
    let active = true;
    (async () => {
      const [size, position, decorated, monitor] = await Promise.all([
        win.outerSize(),
        win.outerPosition(),
        win.isDecorated(),
        currentMonitor(),
      ]);
      if (!active) return;
      shellRef.current = { size, position, decorated };
      if (monitor) {
        await win.setDecorations(false).catch(() => undefined);
        await win.setPosition(monitor.position).catch(() => undefined);
        await win.setSize(monitor.size).catch(() => undefined);
        if (active) setBroadcast(true);
      }
    })().catch(() => undefined);
    return () => {
      active = false;
      const id = windowIdRef.current;
      if (id !== null) void cabinetRelease(id);
      const shell = shellRef.current;
      if (shell) {
        void win.setDecorations(shell.decorated).catch(() => undefined);
        void win.setSize(shell.size).catch(() => undefined);
        void win.setPosition(shell.position).catch(() => undefined);
        shellRef.current = null;
      }
      setBroadcast(false);
    };
  }, [windowId]);

  async function grant() {
    try {
      const trusted = await cabinetRequestPermission();
      setMessage(
        trusted
          ? null
          : "Grant Accessibility in System Settings → Privacy & Security → Accessibility.",
      );
      await attach();
    } catch (err) {
      setError(String(err));
    }
  }

  const instances = match.instances
    .map((instance) => `${instance.sideLabel}@${instance.port}`)
    .join(" + ");
  const result = describeResult(match);
  const supported = status?.supported ?? false;

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-neutral-950 text-neutral-100">
      <header className="flex items-center justify-between gap-4 border-b border-neutral-800 px-6 py-3">
        <div className="flex items-center gap-3">
          <span className="text-lg font-semibold tracking-tight">The Cabinet</span>
          <Badge variant="secondary">{match.rom ?? "match"}</Badge>
          {!match.dev && match.peerIp && (
            <span className="font-mono text-xs text-neutral-400">{match.peerIp}</span>
          )}
          <span className="font-mono text-xs text-neutral-400">{instances}</span>
        </div>
        <div className="flex items-center gap-3">
          <PeerHealthBadge
            health={match.peerHealth ?? undefined}
            rttWarnMs={rttWarnMs}
            loading={false}
          />
          {result && <span className="text-sm font-medium">{result}</span>}
          <Button variant="outline" size="sm" onClick={onShowLobby}>
            <X className="size-4" />
            Lobby
          </Button>
        </div>
      </header>

      <main className="relative flex min-h-0 flex-1 items-center justify-center p-4">
        <div
          ref={viewportRef}
          className="relative h-full w-full rounded-lg border border-dashed border-neutral-700 bg-black"
        >
          {windowId === null && (
            <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 p-6 text-center">
              {error ? (
                <>
                  <p className="max-w-md text-sm text-amber-400">{error}</p>
                  <p className="max-w-md text-xs text-neutral-500">
                    The app backend may be stale. Restart with scripts/dev.sh, then Retry.
                  </p>
                  <Button size="sm" onClick={() => void attach()}>
                    Retry
                  </Button>
                </>
              ) : !status ? (
                <p className="text-sm text-neutral-300">Checking Cabinet mode…</p>
              ) : !supported ? (
                <p className="max-w-md text-sm text-neutral-300">
                  Cabinet mode is not supported on this platform yet — the game is
                  in its own window.
                </p>
              ) : timedOut ? (
                <>
                  <p className="max-w-md text-sm text-neutral-300">
                    No emulator window found yet. The game may have opened in its own
                    window.
                  </p>
                  <Button size="sm" onClick={() => void attach()}>
                    Check again
                  </Button>
                </>
              ) : (
                <>
                  <p className="max-w-md text-sm text-neutral-300">
                    Waiting for the emulator window…
                  </p>
                  {status.permission !== "granted" && (
                    <>
                      <p className="max-w-md text-xs text-neutral-400">
                        {status.detail}
                      </p>
                      <Button size="sm" onClick={() => void grant()}>
                        Grant Accessibility
                      </Button>
                    </>
                  )}
                </>
              )}
              {message && (
                <p className="max-w-md text-xs text-amber-400">{message}</p>
              )}
            </div>
          )}
        </div>
      </main>

      <footer className="flex items-center justify-between gap-4 border-t border-neutral-800 px-6 py-2 text-xs text-neutral-500">
        <span>
          {broadcast ? "Cabinet mode" : "Cabinet mode (windowed)"}
          {windowId !== null ? ` · window ${windowId}` : ""}
        </span>
        <span className="truncate">
          {status
            ? `${status.platform} · ${status.mode} · ${status.permission}${
                status.ownerPids.length > 0
                  ? ` · pids ${status.ownerPids.join(",")}`
                  : ""
              }`
            : "no status"}
        </span>
      </footer>
    </div>
  );
}
