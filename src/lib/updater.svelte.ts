// In-app auto-updater flow. Desktop-only; a no-op in the browser/dev build.
// Progress lands on the StatusBar (busyOp + status), while download/restart
// decisions use the app's modal dialog system.
import { appState } from "./store.svelte";
import { dialogs } from "./dialogs.svelte";
import { anyOverlayOpen } from "./overlays";
import { api } from "./api";
import type { DownloadEvent, UpdateInfo } from "./types";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
const isDev = import.meta.env.DEV;

let checking = false;
let presenting = false;
let lastPromptedVersion: string | null = null;
let pendingAutomaticUpdate: UpdateInfo | null = null;
let pendingPromptTimer: ReturnType<typeof setTimeout> | null = null;
let pollTimer: ReturnType<typeof setInterval> | null = null;
let lastAutomaticCheckAt = 0;
let preparationPromise: Promise<void> | null = null;

const AUTOMATIC_CHECK_INTERVAL_MS = 30 * 60 * 1000;

/** Manual "Check for Updates" (Settings button / macOS menu). */
export async function manualCheckForUpdates(): Promise<void> {
  if (!isTauri() || isDev) return;
  await prepareUpdater();
  await checkForUpdates("manual");
}

/** Auto-check on launch — desktop + non-dev, gated on the autoUpdateCheck setting. */
export async function startupUpdateCheck(): Promise<void> {
  if (!isTauri() || isDev) return;
  // Register polling before the first await. A slow settings-store read must not
  // prevent this launch from ever installing its background timer.
  startBackgroundPolling();
  await prepareUpdater();
  if (!appState.autoUpdateCheck) return;
  await runAutomaticCheck("startup", true);
}

/** Catch up after sleep/minimization, where WebKit may throttle interval timers. */
export async function updateCheckOnActivate(): Promise<void> {
  if (!isTauri() || isDev) return;
  await runPreparedAutomaticCheck("activation");
}

function prepareUpdater(): Promise<void> {
  if (!preparationPromise) {
    preparationPromise = (async () => {
      await appState.waitForUpdateSettingsHydration();
      // Seed only after persisted preferences are known. Every automatic entry
      // point awaits this same promise, so focus/timer checks cannot race startup.
      await seedChannelFromBuild();
    })();
  }
  return preparationPromise;
}

// A beta (pre-release) build tracks the beta channel by default on first run so
// it receives the rolling `next` pre-releases. Only acts when the user hasn't
// chosen a channel yet (no persisted value); never overrides a stored choice.
async function seedChannelFromBuild(): Promise<void> {
  try {
    if ((await appState.getPersistedUpdateChannel()) !== null) return;
    const { getVersion } = await import("@tauri-apps/api/app");
    const v = await getVersion();
    if (typeof v === "string" && v.includes("-")) {
      appState.setUpdateChannel("beta");
    }
  } catch {
    /* non-fatal — falls back to the stable default */
  }
}

function startBackgroundPolling(): void {
  if (pollTimer || isDev) return;
  pollTimer = setInterval(() => {
    void runPreparedAutomaticCheck("background");
  }, AUTOMATIC_CHECK_INTERVAL_MS);
}

async function runPreparedAutomaticCheck(
  source: "background" | "activation",
): Promise<void> {
  await prepareUpdater();
  if (!appState.autoUpdateCheck) return;
  await tryPresentPendingAutomaticUpdate();
  await runAutomaticCheck(source);
}

async function runAutomaticCheck(
  source: "startup" | "background" | "activation",
  force = false,
): Promise<void> {
  if (!appState.autoUpdateCheck) return;
  const now = Date.now();
  if (!force && now - lastAutomaticCheckAt < AUTOMATIC_CHECK_INTERVAL_MS) return;
  lastAutomaticCheckAt = now;
  await checkForUpdates(source);
}

async function checkForUpdates(
  source: "manual" | "startup" | "background" | "activation",
): Promise<void> {
  if (!isTauri() || isDev) return;
  if (checking || presenting) {
    if (source === "manual") appState.status = "Already checking for updates…";
    return;
  }

  checking = true;
  const manual = source === "manual";
  let available: UpdateInfo | null = null;
  if (manual) {
    appState.setBusyOp("Checking for updates");
    appState.status = "Checking for updates…";
  }
  try {
    available = await api.checkUpdateOnChannel(appState.updateChannel);
    if (!available && manual) appState.status = "You are on the latest version.";
  } catch (err) {
    reportUpdaterError(err);
  } finally {
    checking = false;
    if (manual) appState.setBusyOp(null);
  }

  if (!available) return;
  if (manual) {
    if (pendingAutomaticUpdate?.version === available.version) pendingAutomaticUpdate = null;
    await presentUpdate(available);
  } else {
    queueAutomaticUpdate(available);
  }
}

function queueAutomaticUpdate(update: UpdateInfo): void {
  if (update.version === lastPromptedVersion || update.version === pendingAutomaticUpdate?.version) {
    return;
  }
  pendingAutomaticUpdate = update;
  void tryPresentPendingAutomaticUpdate();
}

async function tryPresentPendingAutomaticUpdate(): Promise<void> {
  if (!appState.autoUpdateCheck) {
    pendingAutomaticUpdate = null;
    return;
  }
  // `anyOverlayOpen()` rather than just `dialogs.state`: an automatic check that lands while
  // Settings, Manage Repository, amend/rebase, branch-colour or a GitHub action is open would
  // otherwise mount the update dialog — which sits at a higher z-index — straight over the
  // workflow the user is in the middle of. overlays.ts is the one place that knows every
  // overlay singleton, so asking it there keeps this from drifting as new ones are added.
  if (!pendingAutomaticUpdate || checking || presenting || anyOverlayOpen()) {
    schedulePendingPromptRetry();
    return;
  }
  const update = pendingAutomaticUpdate;
  pendingAutomaticUpdate = null;
  await presentUpdate(update);
}

function schedulePendingPromptRetry(): void {
  if (!pendingAutomaticUpdate || pendingPromptTimer) return;
  pendingPromptTimer = setTimeout(() => {
    pendingPromptTimer = null;
    void tryPresentPendingAutomaticUpdate();
  }, 1000);
}

async function presentUpdate(update: UpdateInfo): Promise<void> {
  if (presenting) return;
  presenting = true;
  lastPromptedVersion = update.version;
  try {
    const notes = (update.notes ?? "").trim();
    const proceed = await dialogs.confirm({
      title: `Update available — ${update.version}`,
      message: notes
        ? `Git It ${update.version} is available.\n\n${notes}`
        : `Git It ${update.version} is available. Download it now?`,
      messageFormat: notes ? "markdown" : "text",
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
    reportUpdaterError(err);
  } finally {
    presenting = false;
    appState.setBusyOp(null);
    schedulePendingPromptRetry();
  }
}

function reportUpdaterError(err: unknown): void {
  const raw = err instanceof Error ? err.message : String(err);
  const sig = /signature|verify|verification|invalid/i.test(raw);
  appState.status = sig
    ? "Updater rejected the update (signature verification failed). The artifact may be unsigned or corrupted."
    : `Updater error: ${raw}`;
}
