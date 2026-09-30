// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import type { ReflogEntry, StashEntry } from "../types";
import StashPanel from "./StashPanel.svelte";
import ReflogPanel from "./ReflogPanel.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
vi.mock("svelte/transition", async (original) => ({ ...await original<typeof import("svelte/transition")>(), slide: () => ({ duration: 0 }) }));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
// A whole task: every queued microtask has run, including the ones Svelte's dev build adds
// around each `await`, so only a retry that is really still pending can show as busy.
const nextTask = () => new Promise((resolve) => setTimeout(resolve));
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/history";
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
// Each row mounts its own concrete component through `open`, so no `mount` call receives a
// union of two component types (their props differ: ReflogPanel has `bare`).
const panels = [
  {
    name: "stashes",
    open: () => mount(StashPanel, { target }),
    method: "stashList" as const,
    what: "the stashes",
    empty: "No stashes.",
    pending: "Loading stashes…",
    // One real row, checked against its type, then widened to fit both panels' spies.
    entries: [{ index: 0, message: "WIP on main: parked work", sha: "a" }] satisfies StashEntry[] as never[],
    row: "WIP on main: parked work",
  },
  {
    name: "reflog",
    open: () => mount(ReflogPanel, { target }),
    method: "reflog" as const,
    what: "the reflog",
    empty: "No reflog entries yet.",
    pending: "Loading reflog entries…",
    entries: [{ sha: "a", short: "abc1234", selector: "HEAD@{0}", subject: "commit: parked work" }] satisfies ReflogEntry[] as never[],
    row: "commit: parked work",
  },
];
it.each(panels)("retries $name without showing a fake empty state", async ({ open, method, what, empty }) => {
  const load = vi.spyOn(api, method).mockRejectedValueOnce(new Error("fatal: cannot read")).mockResolvedValue([]);
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain(`Couldn't load ${what}: cannot read`));
  expect(target.textContent).not.toContain(empty);
  expect(errors.pending).toBe(0);
  const retry = Array.from(target.querySelectorAll("button")).find((b) => b.textContent === "Retry")!;
  retry.click();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  expect(load).toHaveBeenCalledTimes(2);
  expect(target.textContent).not.toContain("Couldn't load");
});
it.each(panels)("shows a $name retry running when it fails the same way again", async ({ open, method, what }) => {
  let fail!: (e: unknown) => void;
  const load = vi
    .spyOn(api, method)
    .mockRejectedValueOnce(new Error("fatal: cannot read"))
    .mockImplementationOnce(() => new Promise<never[]>((_, reject) => { fail = reject; }));
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain(`Couldn't load ${what}: cannot read`));
  const retry = () => target.querySelector<HTMLButtonElement>(".load-error button")!;
  retry().click();
  await nextTask();
  expect(retry().disabled).toBe(true);
  expect(retry().textContent).toBe("Retrying…");
  expect(target.textContent).toContain(`Couldn't load ${what}: cannot read`);
  fail(new Error("fatal: cannot read"));
  await vi.waitFor(() => expect(retry().disabled).toBe(false));
  expect(retry().textContent).toBe("Retry");
  expect(target.textContent).toContain(`Couldn't load ${what}: cannot read`);
  expect(load).toHaveBeenCalledTimes(2);
});
it.each(panels)("shows a $name load that rejects with null as a failure", async ({ open, method, what, empty }) => {
  vi.spyOn(api, method).mockRejectedValue(null);
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await vi.waitFor(() =>
    expect(target.textContent).toContain(`Couldn't load ${what}: The operation failed without an error message.`),
  );
  expect(target.textContent).not.toContain(empty);
});
it.each(panels)("keeps pending $name distinct from successful empty", async ({ open, method, empty, pending }) => {
  let finish!: () => void;
  vi.spyOn(api, method).mockImplementation(() => new Promise<never[]>((resolve) => { finish = () => resolve([]); }));
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await tick();
  expect(target.textContent).toContain(pending);
  expect(target.textContent).not.toContain(empty);
  finish();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
});
it.each(panels)("keeps the current $name list on screen while a refresh runs", async ({ open, method, entries, row }) => {
  let finish!: () => void;
  const load = vi
    .spyOn(api, method)
    .mockResolvedValueOnce(entries)
    .mockImplementationOnce(() => new Promise<never[]>((resolve) => { finish = () => resolve(entries); }));
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain(row));
  // A working-copy / undo refresh re-runs the loader; the list must not flash "Loading".
  appState.setWorkingChanges([]);
  appState.setLastUndo({ branch: "main", sha: "a", label: "Reset" });
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  expect(target.textContent).toContain(row);
  expect(target.textContent).not.toContain("Loading");
  finish();
  await vi.waitFor(() => expect(target.textContent).toContain(row));
});
it.each(panels)("ignores an obsolete $name failure after repository switch", async ({ open, method, empty }) => {
  let reject!: (e: unknown) => void;
  const load = vi.spyOn(api, method).mockImplementationOnce(() => new Promise<never[]>((_, fail) => { reject = fail; })).mockResolvedValue([]);
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await tick();
  appState.repo = "/tmp/new-history";
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  reject(new Error("obsolete failure"));
  await tick();
  await Promise.resolve();
  expect(target.textContent).not.toContain("obsolete failure");
});
it.each(panels)("drops an obsolete $name success after a repository switch", async ({ open, method, empty, entries, row }) => {
  let finish!: () => void;
  const load = vi
    .spyOn(api, method)
    .mockImplementationOnce(() => new Promise<never[]>((resolve) => { finish = () => resolve(entries); }))
    .mockResolvedValue([]);
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await tick();
  appState.repo = "/tmp/new-history";
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  finish();
  await tick();
  await Promise.resolve();
  expect(target.textContent).not.toContain(row);
  expect(target.textContent).toContain(empty);
});
it.each(panels)("logs a $name failure once while refreshes repeat it", async ({ open, method, what, empty }) => {
  appState.clearLog();
  const load = vi
    .spyOn(api, method)
    .mockRejectedValueOnce(new Error("fatal: cannot read"))
    .mockRejectedValueOnce(new Error("fatal: cannot read"))
    .mockResolvedValueOnce([])
    .mockRejectedValueOnce(new Error("fatal: cannot read"));
  const logged = () => appState.logLines.filter((line) => line === `[load] Couldn't load ${what}\nfatal: cannot read`);
  // A working-copy / undo refresh re-runs the loader.
  const refresh = () => {
    appState.setWorkingChanges([]);
    appState.setLastUndo({ branch: "main", sha: "a", label: "Reset" });
  };
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await vi.waitFor(() => expect(logged()).toHaveLength(1));
  refresh();
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await nextTask();
  expect(logged()).toHaveLength(1);
  target.querySelector<HTMLButtonElement>(".load-error button")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  // Failing again after a success is news: it's logged again.
  refresh();
  await vi.waitFor(() => expect(logged()).toHaveLength(2));
  expect(load).toHaveBeenCalledTimes(4);
});
