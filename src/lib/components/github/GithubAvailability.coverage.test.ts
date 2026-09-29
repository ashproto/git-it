// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../../api";
import { appState } from "../../store.svelte";
import { errors } from "../../errors.svelte";
import { githubState } from "../../githubState.svelte";
import type { GhAvailability } from "../../types";
import GithubView from "./GithubView.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let sequence = 0;
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
// A whole task: every queued microtask has run, including the ones Svelte's dev build adds
// around each `await`, so only a retry that is really still pending can show as busy.
const nextTask = () => new Promise((resolve) => setTimeout(resolve));
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = `/tmp/github-availability-${++sequence}`;
  appState.setActiveView("github");
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
it("shows the real failure through GithubView and retries without installation advice", async () => {
  const load = vi.spyOn(api, "githubAvailability").mockRejectedValueOnce({ kind: "Other", message: "fatal: gh could not start" }).mockResolvedValue({ kind: "NotAuthed" });
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the GitHub screen: gh could not start"));
  expect(githubState.availability).toBeNull();
  expect(githubState.availabilityError).toBe("fatal: gh could not start");
  expect(target.textContent).not.toContain("brew install gh");
  expect(target.querySelectorAll(".load-error button")).toHaveLength(1);
  expect(errors.pending).toBe(0);
  target.querySelector<HTMLButtonElement>(".load-error button")!.click();
  await vi.waitFor(() => expect(target.textContent).toContain("Sign in to GitHub"));
  expect(githubState.availabilityError).toBeNull();
  expect(load).toHaveBeenCalledTimes(2);
});
it("shows the availability retry running while it is pending", async () => {
  let fail!: (e: unknown) => void;
  const load = vi
    .spyOn(api, "githubAvailability")
    .mockRejectedValueOnce({ kind: "Other", message: "fatal: gh could not start" })
    .mockImplementationOnce(() => new Promise<GhAvailability>((_, reject) => { fail = reject; }));
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the GitHub screen: gh could not start"));
  const retry = () => target.querySelector<HTMLButtonElement>(".load-error button");
  retry()!.click();
  await nextTask();
  expect(retry()?.disabled).toBe(true);
  expect(retry()?.textContent).toBe("Retrying…");
  fail({ kind: "Other", message: "fatal: gh could not start" });
  await vi.waitFor(() => expect(retry()?.disabled).toBe(false));
  expect(retry()?.textContent).toBe("Retry");
  expect(target.textContent).toContain("Couldn't load the GitHub screen: gh could not start");
  expect(load).toHaveBeenCalledTimes(2);
});
it.each([
  [{ kind: "NotInstalled" }, "brew install gh"],
  [{ kind: "NotAuthed" }, "gh auth login"],
  [{ kind: "NoRemote" }, "No GitHub remote"],
] satisfies Array<[GhAvailability, string]>)("preserves true availability result %j", async (result, text) => {
  vi.spyOn(api, "githubAvailability").mockResolvedValue(result);
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain(text));
  expect(githubState.availabilityError).toBeNull();
  expect(target.textContent).not.toContain("Couldn't load");
});
it("keeps null rejection as an error and does not enter an automatic retry loop", async () => {
  const load = vi.spyOn(api, "githubAvailability").mockRejectedValue(null);
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the GitHub screen: The operation failed without an error message."));
  await tick();
  expect(load).toHaveBeenCalledOnce();
  expect(githubState.availabilityError).toBe("");
});
it("discards a failed probe for a previous repository", async () => {
  let reject!: (e: unknown) => void;
  const load = vi.spyOn(api, "githubAvailability").mockImplementationOnce(() => new Promise((_, fail) => { reject = fail; })).mockResolvedValue({ kind: "NoRemote" });
  const panel = mount(GithubView, { target });
  dispose = () => unmount(panel);
  await tick();
  appState.repo = `/tmp/github-availability-${++sequence}`;
  appState.setActiveView("github");
  await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  await vi.waitFor(() => expect(target.textContent).toContain("No GitHub remote"));
  reject(new Error("obsolete probe"));
  await tick();
  await Promise.resolve();
  expect(githubState.availabilityError).toBeNull();
  expect(target.textContent).not.toContain("obsolete probe");
});
