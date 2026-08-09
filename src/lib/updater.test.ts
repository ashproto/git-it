// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => {
  const appState = {
    updateChannel: "stable" as "stable" | "beta",
    autoUpdateCheck: true,
    status: "Ready",
    waitForUpdateSettingsHydration: vi.fn<() => Promise<void>>(),
    getPersistedUpdateChannel: vi.fn<() => Promise<"stable" | "beta" | null>>(),
    setUpdateChannel: vi.fn<(channel: "stable" | "beta") => void>(),
    setBusyOp: vi.fn<(label: string | null) => void>(),
  };
  const dialogState = { kind: "none" };
  return {
    appState,
    dialogState,
    confirm: vi.fn(),
    checkUpdateOnChannel: vi.fn(),
    installPendingUpdate: vi.fn(),
    getVersion: vi.fn(),
  };
});

vi.mock("./store.svelte", () => ({ appState: mocks.appState }));
vi.mock("./dialogs.svelte", () => ({
  dialogs: { state: mocks.dialogState, confirm: mocks.confirm },
}));
vi.mock("./api", () => ({
  api: {
    checkUpdateOnChannel: mocks.checkUpdateOnChannel,
    installPendingUpdate: mocks.installPendingUpdate,
  },
}));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: mocks.getVersion }));

async function loadUpdater() {
  vi.stubEnv("DEV", false);
  vi.resetModules();
  return import("./updater.svelte");
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-07-17T16:00:00-07:00"));
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  mocks.appState.updateChannel = "stable";
  mocks.appState.autoUpdateCheck = true;
  mocks.appState.status = "Ready";
  mocks.dialogState.kind = "none";
  mocks.appState.waitForUpdateSettingsHydration.mockReset().mockResolvedValue();
  mocks.appState.getPersistedUpdateChannel.mockReset().mockResolvedValue("beta");
  mocks.appState.setUpdateChannel.mockReset().mockImplementation((channel) => {
    mocks.appState.updateChannel = channel;
  });
  mocks.appState.setBusyOp.mockReset();
  mocks.confirm.mockReset().mockResolvedValue(false);
  mocks.checkUpdateOnChannel.mockReset().mockResolvedValue(null);
  mocks.installPendingUpdate.mockReset().mockResolvedValue(undefined);
  mocks.getVersion.mockReset().mockResolvedValue("0.3.0-next.20");
});

afterEach(() => {
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.unstubAllEnvs();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("automatic updater lifecycle", () => {
  it("registers polling before hydration and checks the hydrated beta channel", async () => {
    let finishHydration!: () => void;
    mocks.appState.waitForUpdateSettingsHydration.mockImplementation(
      () => new Promise<void>((resolve) => {
        finishHydration = () => {
          mocks.appState.updateChannel = "beta";
          resolve();
        };
      }),
    );
    const updater = await loadUpdater();

    const startup = updater.startupUpdateCheck();
    expect(vi.getTimerCount()).toBe(1);
    expect(mocks.checkUpdateOnChannel).not.toHaveBeenCalled();
    // A focus event during hydration must join the same readiness barrier, not
    // run an early check against the in-memory stable default.
    const activation = updater.updateCheckOnActivate();
    finishHydration();
    await Promise.all([startup, activation]);

    expect(mocks.checkUpdateOnChannel).toHaveBeenCalledOnce();
    expect(mocks.checkUpdateOnChannel).toHaveBeenCalledWith("beta");
  });

  it("proactively prompts once per version and opts release notes into markdown", async () => {
    mocks.checkUpdateOnChannel.mockResolvedValue({
      version: "0.3.0-next.21",
      currentVersion: "0.3.0-next.20",
      notes: "## Fixes\n\n- **UI** — Render notes",
    });
    const updater = await loadUpdater();

    await updater.startupUpdateCheck();
    await vi.runAllTicks();

    expect(mocks.confirm).toHaveBeenCalledOnce();
    expect(mocks.confirm).toHaveBeenCalledWith(expect.objectContaining({
      title: "Update available — 0.3.0-next.21",
      messageFormat: "markdown",
      confirmLabel: "Download",
    }));

    await vi.advanceTimersByTimeAsync(30 * 60 * 1000);
    expect(mocks.checkUpdateOnChannel).toHaveBeenCalledTimes(2);
    expect(mocks.confirm).toHaveBeenCalledOnce();
  });

  it("defers an automatic prompt until another dialog has closed", async () => {
    mocks.dialogState.kind = "prompt";
    mocks.checkUpdateOnChannel.mockResolvedValue({
      version: "0.3.0-next.21",
      currentVersion: "0.3.0-next.20",
      notes: null,
    });
    const updater = await loadUpdater();

    await updater.startupUpdateCheck();
    expect(mocks.confirm).not.toHaveBeenCalled();

    mocks.dialogState.kind = "none";
    await vi.advanceTimersByTimeAsync(1000);
    expect(mocks.confirm).toHaveBeenCalledOnce();
  });

  // A manual check replaces the backend's pending-update slot. If the UI keeps the queued
  // automatic update anyway, closing the overlay later presents a version the backend no longer
  // has — Download then fails with "no pending update", or fetches something other than what the
  // dialog named. Only an exact version match used to clear the queue.
  it("drops a queued automatic update when a manual check finds nothing", async () => {
    mocks.dialogState.kind = "prompt"; // an overlay is up, so the automatic prompt queues
    mocks.checkUpdateOnChannel.mockResolvedValue({
      version: "0.3.0-next.21",
      currentVersion: "0.3.0-next.20",
      notes: null,
    });
    const updater = await loadUpdater();
    await updater.startupUpdateCheck();
    expect(mocks.confirm).not.toHaveBeenCalled();

    // User switches channel in Settings and checks by hand; this time there is nothing.
    mocks.checkUpdateOnChannel.mockResolvedValue(null);
    await updater.manualCheckForUpdates();

    mocks.dialogState.kind = "none";
    await vi.advanceTimersByTimeAsync(1000);
    expect(mocks.confirm).not.toHaveBeenCalled();
  });

  it("drops a queued automatic update when a manual check finds a different version", async () => {
    mocks.dialogState.kind = "prompt";
    mocks.checkUpdateOnChannel.mockResolvedValue({
      version: "0.3.0-next.21",
      currentVersion: "0.3.0-next.20",
      notes: null,
    });
    const updater = await loadUpdater();
    await updater.startupUpdateCheck();
    expect(mocks.confirm).not.toHaveBeenCalled();

    mocks.checkUpdateOnChannel.mockResolvedValue({
      version: "0.4.0",
      currentVersion: "0.3.0-next.20",
      notes: null,
    });
    await updater.manualCheckForUpdates();
    // The manual check presents its own find immediately.
    expect(mocks.confirm).toHaveBeenCalledOnce();

    // Closing the overlay must NOT then present the superseded next.21 as well.
    mocks.dialogState.kind = "none";
    await vi.advanceTimersByTimeAsync(1000);
    expect(mocks.confirm).toHaveBeenCalledOnce();
  });

  it("uses activation as a catch-up check after a throttled timer window", async () => {
    const updater = await loadUpdater();
    await updater.startupUpdateCheck();
    expect(mocks.checkUpdateOnChannel).toHaveBeenCalledOnce();

    vi.setSystemTime(new Date("2026-07-17T16:31:00-07:00"));
    await updater.updateCheckOnActivate();
    expect(mocks.checkUpdateOnChannel).toHaveBeenCalledTimes(2);
  });
});
