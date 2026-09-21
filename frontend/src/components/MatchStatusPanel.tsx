import { Button } from "@/components/ui/button";
import type { CabinetStatus } from "@/lib/types";

interface MatchStatusPanelProps {
  error: string | null;
  status: CabinetStatus | null;
  supported: boolean;
  timedOut: boolean;
  message: string | null;
  onRetry: () => void;
  onGrant: () => void;
}

export function MatchStatusPanel({
  error,
  status,
  supported,
  timedOut,
  message,
  onRetry,
  onGrant,
}: MatchStatusPanelProps) {
  return (
    <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 p-6 text-center">
      {error ? (
        <>
          <p className="max-w-md text-sm text-amber-400">{error}</p>
          <p className="max-w-md text-xs text-neutral-500">
            The app backend may be stale. Restart with scripts/dev.sh, then Retry.
          </p>
          <Button size="sm" onClick={onRetry}>
            Retry
          </Button>
        </>
      ) : !status ? (
        <p className="text-sm text-neutral-300">Checking Cabinet mode…</p>
      ) : !supported ? (
        <p className="max-w-md text-sm text-neutral-300">
          Cabinet mode is not supported on this platform yet — the game is in its
          own window.
        </p>
      ) : timedOut ? (
        <>
          <p className="max-w-md text-sm text-neutral-300">
            No emulator window found yet. The game may have opened in its own
            window.
          </p>
          <Button size="sm" onClick={onRetry}>
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
              <p className="max-w-md text-xs text-neutral-400">{status.detail}</p>
              <Button size="sm" onClick={onGrant}>
                Grant Accessibility
              </Button>
            </>
          )}
        </>
      )}
      {message && <p className="max-w-md text-xs text-amber-400">{message}</p>}
    </div>
  );
}
