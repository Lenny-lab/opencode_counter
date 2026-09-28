import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import type { DatabaseInfo, Snapshot, TodayStat } from "@/types";

/** Pushed by the Rust watcher whenever the database file changes. */
export const CHANGED_EVENT = "stats://changed";

/**
 * Both windows load the same bundle, so the entry point has to work out which
 * surface it is running as. The label comes from `tauri.conf.json`.
 */
export async function currentWindowLabel(): Promise<string> {
  try {
    return getCurrentWindow().label;
  } catch {
    // Running under plain `vite dev` in a browser, where there is no Tauri
    // runtime. Fall back to the dashboard so the UI is still explorable.
    return "dashboard";
  }
}

export function fetchDatabaseInfo(): Promise<DatabaseInfo> {
  return invoke<DatabaseInfo>("database_info");
}

export function fetchSnapshot(days: number | null): Promise<Snapshot> {
  return invoke<Snapshot>("get_snapshot", { days });
}

export function fetchToday(): Promise<TodayStat> {
  return invoke<TodayStat>("get_today");
}

export function openDashboard(): Promise<void> {
  return invoke<void>("open_dashboard");
}

export function hidePopover(): Promise<void> {
  return invoke<void>("hide_popover");
}

export function hideDashboard(): Promise<void> {
  return invoke<void>("hide_dashboard");
}

export function quitApp(): Promise<void> {
  return invoke<void>("quit");
}

/** Subscribe to database-change notifications. Returns an unsubscribe fn. */
export function onDatabaseChanged(handler: () => void): Promise<UnlistenFn> {
  return listen(CHANGED_EVENT, () => handler());
}
