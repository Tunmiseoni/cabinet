import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  progressLabel,
  progressPercent,
  type Updater,
} from "@/lib/updater";

interface UpdateSectionProps {
  updater: Updater;
}

export function UpdateSection({ updater }: UpdateSectionProps) {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);

  const { update, checking, installing, progress, error, upToDate } = updater;

  return (
    <div className="grid gap-3">
      <div className="grid gap-1">
        <Label>Application update</Label>
        <p className="text-xs text-muted-foreground">
          {version ? `You are running The Cabinet ${version}. ` : ""}
          Updates install in place, so there is nothing to re-download.
        </p>
      </div>

      {installing ? (
        <div className="grid gap-2">
          <Progress value={progressPercent(progress)} />
          <p className="text-xs text-muted-foreground">
            {progressLabel(progress)}
          </p>
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={checking}
            onClick={() => void updater.check()}
          >
            {checking ? "Checking…" : "Check for updates"}
          </Button>
          {update && (
            <Button size="sm" onClick={() => void updater.install()}>
              Install {update.version} and restart
            </Button>
          )}
        </div>
      )}

      {update && !installing && (
        <div className="grid gap-1">
          <p className="text-xs text-muted-foreground">
            Version {update.version} is available.
          </p>
          {update.body && (
            <p className="whitespace-pre-line text-xs text-muted-foreground">
              {update.body}
            </p>
          )}
        </div>
      )}
      {!update && upToDate && !checking && (
        <p className="text-xs text-muted-foreground">
          You are on the latest version.
        </p>
      )}
      {error && <p className="break-all text-xs text-destructive">{error}</p>}
    </div>
  );
}
