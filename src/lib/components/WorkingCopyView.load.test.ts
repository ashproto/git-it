// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import WorkingCopyView from "./WorkingCopyView.svelte";
vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn(), revealItemInDir: vi.fn() }));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
const file = { path: "f.txt", status: "modified", staged: false, unstaged: true, untracked: false, conflicted: false };
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  // DiffView reads window.matchMedia at init and jsdom has none; it mounts once a diff loads.
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  appState.repo = "/tmp/diff";
  appState.setWorkingChanges([file]);
  appState.setSelectedFile({ path: "f.txt", section: "unstaged" });
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
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("shows the summarized failure with Retry, then permits a successful empty diff", async () => {
  const load = vi.spyOn(api, "diff").mockRejectedValueOnce(new Error("From origin\nfatal: cannot read file")).mockResolvedValue("");
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the diff: cannot read file"));
  expect(target.querySelector(".diff-pane")!.textContent).not.toContain("No changes.");
  expect(errors.pending).toBe(0);
  target.querySelector<HTMLButtonElement>(".load-error button")!.click();
  await vi.waitFor(() => expect(target.querySelector(".diff-pane")!.textContent).toContain("No changes."));
  expect(load).toHaveBeenCalledTimes(2);
  expect(load).toHaveBeenLastCalledWith("/tmp/diff", "f.txt", false, false, appState.effectiveDiffContext);
});
it("does not show an empty diff while a request is pending", async () => {
  let finish!: () => void;
  vi.spyOn(api, "diff").mockImplementation(() => new Promise((resolve) => { finish = () => resolve(""); }));
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await tick();
  expect(target.querySelector(".diff-pane")!.textContent).toContain("Loading diff…");
  expect(target.querySelector(".diff-pane")!.textContent).not.toContain("No changes.");
  finish();
  await vi.waitFor(() => expect(target.querySelector(".diff-pane")!.textContent).toContain("No changes."));
});
it("ignores another repository's failure for the same selected path", async () => {
  let reject!: (e: unknown) => void;
  const load = vi.spyOn(api, "diff").mockImplementationOnce(() => new Promise((_, fail) => { reject = fail; })).mockResolvedValue("");
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await tick();
  appState.repo = "/tmp/other-diff";
  appState.setWorkingChanges([file]);
  appState.setSelectedFile({ path: "f.txt", section: "unstaged" });
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await vi.waitFor(() => expect(target.querySelector(".diff-pane")!.textContent).toContain("No changes."));
  reject(new Error("obsolete error"));
  await tick();
  await Promise.resolve();
  expect(target.textContent).not.toContain("obsolete error");
});
