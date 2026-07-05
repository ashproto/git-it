// In-app auto-updater flow. Desktop-only; a no-op in the browser/dev build.
// Mirrors Resume-Designer's state machine, re-surfaced through Git It's own
// surfaces: progress lands on the StatusBar (busyOp + status) and the
// download/restart prompts use dialogs.confirm. No toast library, and no
// pre-relaunch durability gate (Git It only persists settings/view-state
// fire-and-forget — there is no unsaved user document to protect).
import { appState } from "./store.svelte";
import { dialogs } from "./dialogs.svelte";
import { api } from "./api";
import type { DownloadEvent } from "./types";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
const isDev = import.meta.env.DEV;

let checking = false;
let lastBackgroundVersion: string | null = null;
let pollTimer: ReturnType<typeof setInterval> | null = null;

/** Manual "Check for Updates" (Settings button / macOS menu). */
export async function manualCheckForUpdates(): Promise<void> {
  await checkForUpdates("manual");
}

/** Auto-check on launch — desktop + non-dev, gated on the autoUpdateCheck setting. */
export async function startupUpdateCheck(): Promise<void> {
  if (!isTauri() || isDev) return;
  startBackgroundPolling();
  if (!appState.autoUpdateCheck) return;
  await checkForUpdates("startup");
}

// Poll for updates every 30 minutes while the app is open. Notify-only, respects
// the auto-check setting live, and never runs in dev.
function startBackgroundPolling(): void {
  if (pollTimer || isDev) return;
  const THIRTY_MIN = 30 * 60 * 1000;
  pollTimer = setInterval(() => {
    if (!appState.autoUpdateCheck) return;
    void checkForUpdates("background");
  }, THIRTY_MIN);
}

async function checkForUpdates(source: "manual" | "startup" | "background"): Promise<void> {
  if (!isTauri() || isDev) return;
  if (checking) return;
  checking = true;
  const manual = source === "manual";
  if (manual) {
    appState.setBusyOp("Checking for updates");
    appState.status = "Checking for updates…";
  }
  try {
    const update = await api.checkUpdateOnChannel(appState.updateChannel);
    if (!update) {
      if (manual) appState.status = "You are on the latest version.";
      return;
    }
    // Background poll: one prompt per new version, no nagging.
    if (source === "background") {
      if (update.version === lastBackgroundVersion) return;
      lastBackgroundVersion = update.version;
    }
    const notes = (update.notes ?? "").trim();
    const proceed = await dialogs.confirm({
      title: `Update available — ${update.version}`,
      message: notes
        ? `Git It ${update.version} is available.\n\n${notes}`
        : `Git It ${update.version} is available. Download it now?`,
      confirmLabel: "Download",
    });
    if (!proceed) {
      appState.status = "Update download postponed.";
      return;
    }

    appState.setBusyOp("Downloading update");
    let total = 0;
    let downloaded = 0;
    await api.installPendingUpdate((e: DownloadEvent) => {
      if (e.event === "Started") {
        total = e.data.contentLength ?? 0;
      } else if (e.event === "Progress") {
        downloaded += e.data.chunkLength ?? 0;
        const pct = total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0;
        appState.setBusyOp(`Downloading update ${pct}%`);
        appState.status = `Downloading update… ${pct}%`;
      } else if (e.event === "Finished") {
        appState.status = `Version ${update.version} is ready to install.`;
      }
    });

    const restart = await dialogs.confirm({
      title: "Update ready",
      message: `Git It ${update.version} has been downloaded. Restart now to apply it?`,
      confirmLabel: "Restart Now",
    });
    if (!restart) {
      appState.status = "Update downloaded. Restart later to finish installing.";
      return;
    }
    appState.setBusyOp("Restarting to install update");
    // 10s watchdog: if the relaunch-into-installer step never starts, surface a
    // signing/notarization hint instead of hanging silently.
    const guard = setTimeout(() => {
      appState.status =
        "Update install did not start. Verify the app is properly signed/notarized.";
    }, 10000);
    try {
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } finally {
      clearTimeout(guard);
    }
  } catch (err) {
    const raw = err instanceof Error ? err.message : String(err);
    const sig = /signature|verify|verification|invalid/i.test(raw);
    appState.status = sig
      ? "Updater rejected the update (signature verification failed). The artifact may be unsigned or corrupted."
      : `Updater error: ${raw}`;
  } finally {
    checking = false;
    appState.setBusyOp(null);
  }
}
