import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage } from "../services/api";

const REFRESH_MS = 30_000;

export interface DataState<T> {
  data: T | undefined;
  error: string | null;
  reload: () => void;
}

/**
 * Loads data and keeps it current. The tracker writes in batches, so pages
 * refresh on a timer as well as when the backend reports a change. Refreshing
 * keeps the previous data on screen, so charts do not flash.
 */
export function useData<T>(load: () => Promise<T>, dependencies: unknown[]): DataState<T> {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<string | null>(null);
  const latest = useRef(0);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const stableLoad = useCallback(load, dependencies);

  const reload = useCallback(() => {
    const request = ++latest.current;
    stableLoad().then(
      (result) => {
        if (request === latest.current) {
          setData(result);
          setError(null);
        }
      },
      (failure: unknown) => {
        if (request === latest.current) setError(errorMessage(failure));
      },
    );
  }, [stableLoad]);

  useEffect(() => {
    reload();
    const timer = window.setInterval(() => {
      if (!document.hidden) reload();
    }, REFRESH_MS);
    const unlisten = listen("data-changed", reload);
    return () => {
      window.clearInterval(timer);
      void unlisten.then((stop) => stop());
    };
  }, [reload]);

  return { data, error, reload };
}
