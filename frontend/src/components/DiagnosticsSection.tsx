import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { collectDiagnostics, openLogsDir } from "@/lib/api";

interface DiagnosticsSectionProps {
  verboseLogging: boolean;
  onVerboseLoggingChange: (value: boolean) => void;
}

export function DiagnosticsSection({
  verboseLogging,
  onVerboseLoggingChange,
}: DiagnosticsSectionProps) {
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  async function handleOpenLogs() {
    try {
      const dir = await openLogsDir();
      setStatus(`Opened ${dir}`);
    } catch (err) {
      setStatus(String(err));
    }
  }

  async function handleCollectDiagnostics() {
    setBusy(true);
    try {
      const result = await collectDiagnostics();
      setStatus(`Wrote ${result.path}`);
    } catch (err) {
      setStatus(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid gap-3">
      <div className="flex items-center justify-between gap-4">
        <div className="grid gap-1">
          <Label htmlFor="verboseLogging">Verbose logging</Label>
          <p className="text-xs text-muted-foreground">
            Raise the app log to debug level. Takes effect after a restart.
          </p>
        </div>
        <input
          id="verboseLogging"
          type="checkbox"
          className="size-4 shrink-0 accent-primary"
          checked={verboseLogging}
          onChange={(event) => onVerboseLoggingChange(event.target.checked)}
        />
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="outline" size="sm" onClick={() => void handleOpenLogs()}>
          Open logs folder
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => void handleCollectDiagnostics()}
        >
          {busy ? "Collecting…" : "Collect diagnostics"}
        </Button>
      </div>
      {status && (
        <p className="break-all text-xs text-muted-foreground">{status}</p>
      )}
    </div>
  );
}
