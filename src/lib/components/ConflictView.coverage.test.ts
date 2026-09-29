// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import type { ConflictEntry } from "../types";
import ConflictView from "./ConflictView.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
// A whole task: every queued microtask has run, including the ones Svelte's dev build adds
// around each `await`, so only a request that is really still pending can show as such.
const nextTask = () => new Promise((resolve) => setTimeout(resolve));
function setStatus(conflicted: number) {
  appState.setRepoStatus({
    head: { sha: "a", branch: "main", detached: false },
    staged: 0,
    unstaged: 0,
    untracked: 0,
    conflicted,
    operation: "merge",
  });
}
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/conflicts";
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.setRepoStatus(null);
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it.each([new Error("fatal: index locked"), null])(
  "shows a retryable load error instead of an empty list (%s)",
  async (error) => {
    setStatus(1);
    const load = vi
      .spyOn(api, "conflictDetails")
      .mockRejectedValueOnce(error)
      .mockResolvedValue([{ path: "f", kind: "both" }]);
    const panel = mount(ConflictView, { target });
    dispose = () => unmount(panel);
    await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the conflict details:"));
    // A background load failure is inline, never a popup.
    expect(errors.pending).toBe(0);
    Array.from(target.querySelectorAll("button"))
      .find((b) => b.textContent === "Retry")!
      .click();
    await vi.waitFor(() => expect(target.textContent).toContain("Use ours"));
    expect(target.textContent).not.toContain("Couldn't load the conflict details:");
    expect(load).toHaveBeenCalledTimes(2);
  },
);
it("shows Retry running while a failed details load retries", async () => {
  setStatus(1);
  let fail!: (e: unknown) => void;
  const load = vi
    .spyOn(api, "conflictDetails")
    .mockRejectedValueOnce(new Error("fatal: index locked"))
    .mockImplementationOnce(() => new Promise((_, reject) => { fail = reject; }));
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the conflict details: index locked"));
  const retry = () => target.querySelector<HTMLButtonElement>(".load-error button")!;
  retry().click();
  await nextTask();
  expect(retry().disabled).toBe(true);
  expect(retry().textContent).toBe("Retrying…");
  fail(new Error("fatal: index locked"));
  await vi.waitFor(() => expect(retry().disabled).toBe(false));
  expect(retry().textContent).toBe("Retry");
  expect(target.textContent).toContain("Couldn't load the conflict details: index locked");
  expect(load).toHaveBeenCalledTimes(2);
});
it("keeps a details error on screen while a refresh runs", async () => {
  setStatus(1);
  let finish!: (entries: ConflictEntry[]) => void;
  const load = vi
    .spyOn(api, "conflictDetails")
    .mockRejectedValueOnce(new Error("fatal: index locked"))
    .mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the conflict details: index locked"));
  // Every watcher tick assigns a fresh status, which refreshes the list.
  setStatus(1);
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await nextTask();
  expect(target.textContent).toContain("Couldn't load the conflict details: index locked");
  finish([{ path: "f", kind: "both" }]);
  await vi.waitFor(() => expect(target.textContent).toContain("Use ours"));
  expect(target.textContent).not.toContain("Couldn't load");
});
it("leaves the guidance and Continue to git's conflict count when the details fail", async () => {
  setStatus(0);
  const load = vi.spyOn(api, "conflictDetails").mockRejectedValue(new Error("fatal: index locked"));
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(load).toHaveBeenCalled());
  await nextTask();
  // Nothing is left to list, so the failure doesn't replace the guidance.
  expect(target.textContent).not.toContain("Couldn't load");
  expect(target.textContent).toContain("All conflicts resolved — continue to finish, or abort.");
  expect(target.querySelector<HTMLButtonElement>(".primary")!.disabled).toBe(false);
});
it("drops a superseded request within an operation", async () => {
  setStatus(1);
  const requests: { resolve: (entries: ConflictEntry[]) => void; reject: (e: unknown) => void }[] = [];
  const load = vi
    .spyOn(api, "conflictDetails")
    .mockImplementation(() => new Promise((resolve, reject) => { requests.push({ resolve, reject }); }));
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(1));
  // A watcher tick starts a second request before the first settles.
  setStatus(1);
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  requests[1].resolve([{ path: "f", kind: "both" }]);
  await vi.waitFor(() => expect(target.textContent).toContain("Use ours"));
  requests[0].reject(new Error("fatal: superseded"));
  await nextTask();
  expect(target.textContent).toContain("Use ours");
  expect(target.textContent).not.toContain("Couldn't load");
});
it("clears a details error when the operation ends", async () => {
  setStatus(1);
  vi.spyOn(api, "conflictDetails")
    .mockRejectedValueOnce(new Error("fatal: index locked"))
    .mockImplementation(() => new Promise(() => {}));
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the conflict details:"));
  appState.setRepoStatus(null);
  await tick();
  // The next operation's list is still loading; the old failure must not come back with it.
  setStatus(1);
  await nextTask();
  expect(target.textContent).toContain("Merge in progress");
  expect(target.textContent).not.toContain("Couldn't load");
});
it("drops a request that settles after its operation ended", async () => {
  setStatus(1);
  let finish!: (entries: ConflictEntry[]) => void;
  const load = vi
    .spyOn(api, "conflictDetails")
    .mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }))
    .mockImplementation(() => new Promise(() => {}));
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(1));
  appState.setRepoStatus(null);
  await tick();
  finish([{ path: "old", kind: "both" }]);
  await nextTask();
  // A new operation whose own list never arrives.
  setStatus(1);
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await nextTask();
  expect(target.textContent).toContain("Merge in progress");
  expect(target.textContent).not.toContain("old");
});
