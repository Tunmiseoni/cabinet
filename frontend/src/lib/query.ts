import { useCallback, useEffect, useRef, useState } from "react";
import { usePolling } from "./hooks";

function sameJson(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

const cache = new Map<string, unknown>();

interface InvokeOptions {
  enabled?: boolean;
  pollMs?: number;
}

export function useInvoke<T>(
  key: string,
  fetcher: () => Promise<T>,
  { enabled = true, pollMs }: InvokeOptions = {},
) {
  const [data, setData] = useState<T | null>(() => (cache.get(key) as T) ?? null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(() => enabled && !cache.has(key));

  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const load = useCallback(
    async (silent: boolean) => {
      if (!enabled) return;
      if (!silent) setLoading(true);
      try {
        const next = await fetcherRef.current();
        if (!sameJson(cache.get(key), next)) cache.set(key, next);
        setData((prev) => (sameJson(prev, next) ? prev : next));
        setError(null);
      } catch (err) {
        setError(String(err));
      } finally {
        if (!silent) setLoading(false);
      }
    },
    [key, enabled],
  );

  useEffect(() => {
    const cached = cache.has(key);
    setData((cache.get(key) as T) ?? null);
    setLoading(enabled && !cached);
    void load(cached);
  }, [key, enabled, load]);

  usePolling(() => void load(true), pollMs);

  const refresh = useCallback(() => load(false), [load]);
  const mutate = useCallback(
    (next: T) => {
      cache.set(key, next);
      setData(next);
    },
    [key],
  );

  return { data, error, loading, refresh, mutate };
}
