import { useCallback, useEffect, useRef, useState } from "react";
import { check as checkForUpdate, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export interface Updater {
  update: Update | null;
  checking: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  error: string | null;
  upToDate: boolean;
  check: () => Promise<void>;
  install: () => Promise<void>;
}

export function progressPercent(progress: UpdateProgress | null): number {
  if (!progress || !progress.total) return 0;
  return Math.min(100, Math.round((progress.downloaded / progress.total) * 100));
}

export function progressLabel(progress: UpdateProgress | null): string {
  if (!progress) return "Starting…";
  const mb = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (!progress.total) return `Downloaded ${mb(progress.downloaded)}`;
  return `Downloaded ${mb(progress.downloaded)} of ${mb(progress.total)}`;
}

export function useUpdater(): Updater {
  const [update, setUpdate] = useState<Update | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [upToDate, setUpToDate] = useState(false);
  const installingRef = useRef(false);

  const check = useCallback(async () => {
    setChecking(true);
    setError(null);
    try {
      const found = await checkForUpdate();
      setUpdate(found);
      setUpToDate(found === null);
    } catch (err) {
      setError(String(err));
    } finally {
      setChecking(false);
    }
  }, []);

  const install = useCallback(async () => {
    if (!update || installingRef.current) return;
    installingRef.current = true;
    setInstalling(true);
    setError(null);
    setProgress({ downloaded: 0, total: null });
    try {
      let downloaded = 0;
      let total: number | null = null;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? null;
          setProgress({ downloaded, total });
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setProgress({ downloaded, total });
        } else {
          setProgress({ downloaded: total ?? downloaded, total });
        }
      });
      await relaunch();
    } catch (err) {
      setError(String(err));
    } finally {
      installingRef.current = false;
      setInstalling(false);
      setProgress(null);
    }
  }, [update]);

  useEffect(() => {
    if (import.meta.env.DEV) return;
    void check();
  }, [check]);

  return {
    update,
    checking,
    installing,
    progress,
    error,
    upToDate,
    check,
    install,
  };
}
