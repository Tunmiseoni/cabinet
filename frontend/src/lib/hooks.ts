import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export function useTauriEvent<T>(
  event: string,
  handler: (payload: T) => void,
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let dispose: UnlistenFn | undefined;
    let cancelled = false;
    listen<T>(event, (message) => handlerRef.current(message.payload)).then(
      (unlisten) => {
        if (cancelled) unlisten();
        else dispose = unlisten;
      },
    );
    return () => {
      cancelled = true;
      dispose?.();
    };
  }, [event]);
}

export function usePolling(callback: () => void, intervalMs: number): void {
  const callbackRef = useRef(callback);
  callbackRef.current = callback;

  useEffect(() => {
    const timer = setInterval(
      () => callbackRef.current(),
      Math.max(intervalMs, 0),
    );
    return () => clearInterval(timer);
  }, [intervalMs]);
}

export function useAsyncTask(onError: (message: string | null) => void) {
  const [busy, setBusy] = useState(false);
  const run = useCallback(
    async (task: () => Promise<void>) => {
      setBusy(true);
      try {
        await task();
        onError(null);
      } catch (err) {
        onError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [onError],
  );
  return { busy, run };
}
