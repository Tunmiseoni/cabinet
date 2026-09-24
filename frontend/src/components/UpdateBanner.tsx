import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { progressLabel, progressPercent, type Updater } from "@/lib/updater";
import { Download } from "lucide-react";

interface UpdateBannerProps {
  updater: Updater;
}

export function UpdateBanner({ updater }: UpdateBannerProps) {
  const { update, installing, progress } = updater;

  if (installing) {
    return (
      <Alert>
        <Download className="size-4" />
        <AlertTitle>Installing update…</AlertTitle>
        <AlertDescription className="grid gap-2">
          <Progress value={progressPercent(progress)} />
          <span className="text-xs text-muted-foreground">
            {progressLabel(progress)}
          </span>
        </AlertDescription>
      </Alert>
    );
  }

  if (!update) return null;

  return (
    <Alert>
      <Download className="size-4" />
      <AlertTitle>Update available — The Cabinet {update.version}</AlertTitle>
      <AlertDescription className="flex items-center justify-between gap-4">
        <span>The update installs in place and restarts the app.</span>
        <Button size="sm" onClick={() => void updater.install()}>
          Install and restart
        </Button>
      </AlertDescription>
    </Alert>
  );
}
