// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ check: vi.fn(), install: vi.fn(), confirm: vi.fn(), relaunch: vi.fn() }));
vi.mock("./api", () => ({ api: { checkUpdateOnChannel: mocks.check, installPendingUpdate: mocks.install } }));
vi.mock("./dialogs.svelte", () => ({ dialogs: { confirm: mocks.confirm } }));
vi.mock("./overlays", () => ({ anyOverlayOpen: () => false }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: mocks.relaunch }));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let updater: typeof import("./updater.svelte");
let appState: (typeof import("./store.svelte"))["appState"];
let errors: (typeof import("./errors.svelte"))["errors"];
const update = { version: "0.4.0", currentVersion: "0.3.0", notes: null };
beforeEach(async () => {
  // Real timers let dependency optimizer rebundling settle during the dynamic imports.
  vi.useRealTimers();
  vi.stubEnv("DEV", false);
  vi.resetModules();
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  mocks.check.mockReset().mockResolvedValue(null);
  mocks.install.mockReset().mockResolvedValue(undefined);
  mocks.confirm.mockReset().mockResolvedValue(false);
  mocks.relaunch.mockReset().mockResolvedValue(undefined);
  ({ appState } = await import("./store.svelte"));
  vi.spyOn(appState, "waitForUpdateSettingsHydration").mockResolvedValue(undefined);
  vi.spyOn(appState, "getPersistedUpdateChannel").mockResolvedValue("stable");
  vi.spyOn(appState, "autoUpdateCheck", "get").mockReturnValue(true);
  ({ errors } = await import("./errors.svelte"));
  errors.reset();
  updater = await import("./updater.svelte");
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-09-26T12:00:00Z"));
});
afterEach(() => {
  errors.reset();
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("reports manual check failure with a popup", async () => {
  mocks.check.mockRejectedValue(new Error("fatal: network unavailable"));
  await updater.manualCheckForUpdates();
  expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't check for updates", headline: "network unavailable" });
  expect(errors.pending).toBe(1);
  expect(appState.statusIsError).toBe(true);
  expect(appState.busyOp).toBeNull();
});
it("reports a check failure as it is, not as a signature failure", async () => {
  // A check only fetches and parses the manifest; signatures are verified when installing.
  mocks.check.mockRejectedValue(new Error("invalid peer certificate: UnknownIssuer"));
  await updater.manualCheckForUpdates();
  expect(errors.current).toMatchObject({
    kind: "error",
    title: "Couldn't check for updates",
    headline: "invalid peer certificate: UnknownIssuer",
    output: "invalid peer certificate: UnknownIssuer",
  });
});
it.each(["startup", "background", "activation"])("keeps %s check errors quiet but accessible", async (source) => {
  if (source === "background") {
    await updater.startupUpdateCheck();
    mocks.check.mockRejectedValue(new Error("fatal: offline"));
    await vi.advanceTimersByTimeAsync(30 * 60 * 1000);
  } else {
    mocks.check.mockRejectedValue(new Error("fatal: offline"));
    if (source === "startup") await updater.startupUpdateCheck();
    else await updater.updateCheckOnActivate();
  }
  expect(errors.current).toBeNull();
  expect(errors.pending).toBe(0);
  expect(errors.last).toMatchObject({ title: "Couldn't check for updates", headline: "offline" });
  expect(appState.statusIsError).toBe(true);
  expect(appState.logLines.join("\n")).toContain("offline");
});
it("notices only manual overlapping checks", async () => {
  let finish!: (value: null) => void;
  mocks.check.mockImplementation(() => new Promise<null>((resolve) => { finish = resolve; }));
  const first = updater.manualCheckForUpdates();
  await vi.waitFor(() => expect(mocks.check).toHaveBeenCalledOnce());
  await updater.updateCheckOnActivate();
  expect(errors.pending).toBe(0);
  await updater.manualCheckForUpdates();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Can't check for updates yet", headline: "Already checking for updates…" });
  expect(appState.statusIsError).toBe(false);
  finish(null);
  await first;
});
it("reports download/install failure and preserves signature guidance and raw output", async () => {
  mocks.check.mockResolvedValue(update);
  mocks.confirm.mockResolvedValue(true);
  mocks.install.mockRejectedValue(new Error("signature verification failed: artifact detail"));
  await updater.manualCheckForUpdates();
  expect(errors.current?.title).toBe("Couldn't download or install the update");
  expect(errors.current?.headline).toContain("Updater rejected the update");
  expect(errors.current?.output).toContain("signature verification failed: artifact detail");
  expect(errors.pending).toBe(1);
  expect(appState.busyOp).toBeNull();
});
it("reports relaunch rejection", async () => {
  mocks.check.mockResolvedValue(update);
  mocks.confirm.mockResolvedValue(true);
  mocks.relaunch.mockRejectedValue(new Error("fatal: relaunch denied"));
  await updater.manualCheckForUpdates();
  expect(errors.current).toMatchObject({ title: "Couldn't restart to install the update", headline: "relaunch denied" });
  expect(errors.pending).toBe(1);
});
it("reports a relaunch watchdog once and only logs a later rejection", async () => {
  let reject!: (e: unknown) => void;
  mocks.check.mockResolvedValue(update);
  mocks.confirm.mockResolvedValue(true);
  mocks.relaunch.mockImplementation(() => new Promise<void>((_, fail) => { reject = fail; }));
  const pending = updater.manualCheckForUpdates();
  await vi.waitFor(() => expect(mocks.relaunch).toHaveBeenCalledOnce());
  await vi.advanceTimersByTimeAsync(10000);
  expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't restart to install the update", headline: "Update install did not start. Verify the app is properly signed/notarized." });
  reject(new Error("late rejection"));
  await pending;
  expect(errors.pending).toBe(1);
  expect(appState.logLines).toContain("[updater] relaunch failed after the watchdog: late rejection");
  expect(appState.busyOp).toBeNull();
});
it("does not report user postponement as an error", async () => {
  mocks.check.mockResolvedValue(update);
  await updater.manualCheckForUpdates();
  expect(errors.pending).toBe(0);
  expect(appState.status).toBe("Update download postponed.");
});
