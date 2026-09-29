// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import ConflictView from "./ConflictView.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
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
it("leaves Continue to git's conflict count when the details fail", async () => {
  setStatus(0);
  vi.spyOn(api, "conflictDetails").mockRejectedValue(new Error("fatal: index locked"));
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the conflict details:"));
  expect(target.querySelector<HTMLButtonElement>(".primary")!.disabled).toBe(false);
});
it("discards a failed request after the operation ends", async () => {
  setStatus(1);
  let reject!: (e: unknown) => void;
  vi.spyOn(api, "conflictDetails").mockImplementation(
    () => new Promise((_, fail) => {
      reject = fail;
    }),
  );
  const panel = mount(ConflictView, { target });
  dispose = () => unmount(panel);
  await tick();
  appState.setRepoStatus(null);
  await tick();
  reject(new Error("obsolete"));
  await tick();
  expect(target.querySelector("section")).toBeNull();
  expect(errors.pending).toBe(0);
});
