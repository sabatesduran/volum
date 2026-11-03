import { create } from "zustand";
import type { DownloadEvent, Update } from "@tauri-apps/plugin-updater";
import { isTauri } from "../../lib/tauri/api";

export type UpdatePhase = "idle" | "checking" | "available" | "downloading" | "downloaded" | "installing" | "up-to-date" | "preview" | "error";

export interface AvailableUpdate {
  version: string;
  date?: string;
  body?: string;
}

interface UpdateState {
  phase: UpdatePhase;
  update?: AvailableUpdate;
  downloadedBytes: number;
  totalBytes?: number;
  downloadReady: boolean;
  error?: string;
  checkedAt?: number;
  dismissedVersion?: string;
  dismissNotification: () => void;
}

let activeUpdate: Update | undefined;
let checkPromise: Promise<void> | undefined;

export const useUpdateStore = create<UpdateState>((set) => ({
  phase: "idle",
  downloadedBytes: 0,
  downloadReady: false,
  dismissNotification: () => set((state) => ({ dismissedVersion: state.update?.version }))
}));

function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}

export function updatePercent(downloadedBytes: number, totalBytes?: number): number | undefined {
  if (!totalBytes || totalBytes <= 0) return undefined;
  return Math.min(100, Math.round(downloadedBytes / totalBytes * 100));
}

export async function checkForUpdates(manual = false): Promise<void> {
  const phase = useUpdateStore.getState().phase;
  if (activeUpdate && ["available", "downloading", "downloaded", "installing", "error"].includes(phase)) return;
  if (checkPromise) return checkPromise;
  if (!isTauri()) {
    if (manual) useUpdateStore.setState({ phase: "preview", error: undefined, checkedAt: Date.now() });
    return;
  }

  checkPromise = (async () => {
    useUpdateStore.setState({ phase: "checking", error: undefined });
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check({ timeout: 20_000 });
      if (!update) {
        if (activeUpdate) await activeUpdate.close().catch(() => undefined);
        activeUpdate = undefined;
        useUpdateStore.setState({
          phase: manual ? "up-to-date" : "idle",
          update: undefined,
          downloadedBytes: 0,
          totalBytes: undefined,
          downloadReady: false,
          checkedAt: Date.now()
        });
        return;
      }
      if (activeUpdate && activeUpdate !== update) await activeUpdate.close().catch(() => undefined);
      activeUpdate = update;
      useUpdateStore.setState({
        phase: "available",
        update: { version: update.version, date: update.date, body: update.body },
        downloadedBytes: 0,
        totalBytes: undefined,
        downloadReady: false,
        error: undefined,
        checkedAt: Date.now()
      });
    } catch (reason) {
      useUpdateStore.setState({
        phase: manual ? "error" : "idle",
        error: manual ? errorMessage(reason) : undefined,
        checkedAt: Date.now()
      });
    } finally {
      checkPromise = undefined;
    }
  })();
  return checkPromise;
}

export async function downloadUpdate(): Promise<void> {
  if (!activeUpdate) return;
  useUpdateStore.setState({ phase: "downloading", downloadedBytes: 0, totalBytes: undefined, downloadReady: false, error: undefined, dismissedVersion: undefined });
  try {
    await activeUpdate.download((event: DownloadEvent) => {
      if (event.event === "Started") {
        useUpdateStore.setState({ downloadedBytes: 0, totalBytes: event.data.contentLength });
      } else if (event.event === "Progress") {
        useUpdateStore.setState((state) => ({ downloadedBytes: state.downloadedBytes + event.data.chunkLength }));
      } else if (event.event === "Finished") {
        useUpdateStore.setState((state) => ({ downloadedBytes: state.totalBytes ?? state.downloadedBytes }));
      }
    });
    useUpdateStore.setState({ phase: "downloaded", downloadReady: true });
  } catch (reason) {
    useUpdateStore.setState({ phase: "error", error: errorMessage(reason) });
  }
}

export async function restartToUpdate(): Promise<void> {
  if (!activeUpdate) return;
  useUpdateStore.setState({ phase: "installing", error: undefined });
  try {
    await activeUpdate.install({ restartAfterInstall: false });
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  } catch (reason) {
    useUpdateStore.setState({ phase: "error", error: errorMessage(reason) });
  }
}
