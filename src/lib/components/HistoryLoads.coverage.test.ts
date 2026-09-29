// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import StashPanel from "./StashPanel.svelte";
import ReflogPanel from "./ReflogPanel.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
vi.mock("svelte/transition", async (original) => ({ ...await original<typeof import("svelte/transition")>(), slide: () => ({ duration: 0 }) }));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
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
  },
  {
    name: "reflog",
    open: () => mount(ReflogPanel, { target }),
    method: "reflog" as const,
    what: "the reflog",
    empty: "No reflog entries.",
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
it.each(panels)("keeps pending $name distinct from successful empty", async ({ open, method, empty }) => {
  let finish!: () => void;
  vi.spyOn(api, method).mockImplementation(() => new Promise<never[]>((resolve) => { finish = () => resolve([]); }));
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await tick();
  expect(target.textContent).not.toContain(empty);
  finish();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
});
it.each(panels)("keeps the current $name list on screen while a refresh runs", async ({ open, method, empty }) => {
  let finish!: () => void;
  const load = vi
    .spyOn(api, method)
    .mockResolvedValueOnce([])
    .mockImplementationOnce(() => new Promise<never[]>((resolve) => { finish = () => resolve([]); }));
  const panel = open();
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>(".toggle")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  // A working-copy / undo refresh re-runs the loader; the list must not flash "Loading".
  appState.setWorkingChanges([]);
  appState.setLastUndo({ branch: "main", sha: "a", label: "Reset" });
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  expect(target.textContent).toContain(empty);
  expect(target.textContent).not.toContain("Loading");
  finish();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
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
