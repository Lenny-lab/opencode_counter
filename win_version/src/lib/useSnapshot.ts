import { useCallback, useEffect, useRef, useState } from "react";

import { fetchDatabaseInfo, fetchSnapshot, onDatabaseChanged } from "@/lib/ipc";
import type { DatabaseInfo, Snapshot } from "@/types";

export interface SnapshotState {
  snapshot: Snapshot | null;
  info: DatabaseInfo | null;
  loading: boolean;
  /** True only for the very first load, so refreshes do not blank the UI. */
  initial: boolean;
  error: string | null;
  reload: () => void;
}

/**
 * Loads a snapshot and keeps it current.
 *
 * `pollMs` is the fallback for when the file watcher misses something. The
 * macOS build used 45 seconds for the dashboard and none for the popover, which
 * only loads on open; the watcher event does the real work in both cases.
 */
export function useSnapshot(days: number | null, pollMs = 0): SnapshotState {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [info, setInfo] = useState<DatabaseInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [initial, setInitial] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Guards against a slow response for a range the user has already left.
  const request = useRef(0);
  // Read inside `load` but deliberately not a dependency: `load` writes the
  // info it probes, so depending on it would re-fire the effect that called
  // `load` and spin forever.
  const known = useRef<DatabaseInfo | null>(null);

  const load = useCallback(async () => {
    const ticket = ++request.current;
    setLoading(true);
    try {
      const next = await fetchSnapshot(days);
      if (ticket !== request.current) return;
      setSnapshot(next);
      setError(null);
      // The database may have appeared, moved, or been removed since last time.
      known.current = await fetchDatabaseInfo();
      setInfo(known.current);
    } catch (err) {
      if (ticket !== request.current) return;
      setError(err instanceof Error ? err.message : String(err));
      if (known.current === null) {
        known.current = await fetchDatabaseInfo().catch(() => null);
        setInfo(known.current);
      }
    } finally {
      if (ticket === request.current) {
        setLoading(false);
        setInitial(false);
      }
    }
  }, [days]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let dispose: (() => void) | undefined;
    let cancelled = false;
    void onDatabaseChanged(() => {
      if (!cancelled) void load();
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else dispose = unlisten;
    });
    return () => {
      cancelled = true;
      dispose?.();
    };
  }, [load]);

  useEffect(() => {
    if (pollMs <= 0) return;
    const timer = window.setInterval(() => void load(), pollMs);
    return () => window.clearInterval(timer);
  }, [load, pollMs]);

  return { snapshot, info, loading, initial, error, reload: () => void load() };
}
