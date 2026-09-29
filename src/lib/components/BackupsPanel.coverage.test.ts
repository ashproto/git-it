// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import BackupsPanel from "./BackupsPanel.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
let target: HTMLDivElement;
let dispose: (() => Promise<void>) | undefined;
// A whole task: every queued microtask has run, including the ones Svelte's dev build adds
// around each `await`, so only a retry that is really still pending can show as busy.
const nextTask = () => new Promise((resolve) => setTimeout(resolve));
function button(text: string): HTMLButtonElement {
  const found = Array.from(target.querySelectorAll("button")).find((b) => b.textContent?.trim() === text);
  if (!found) throw new Error(`Missing button: ${text}`);
  return found;
}
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/backups";
  vi.spyOn(api, "listBundles").mockResolvedValue([{ name: "backup.bundle", path: "/tmp/backup.bundle", size: 12 }]);
  vi.spyOn(api, "listSafetyRefs").mockResolvedValue([{ kind: "original", name: "refs/original/main" }]);
  vi.stubGlobal("confirm", vi.fn(() => true));
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
it.each([
  ["bundles", "Couldn't load the bundles:", "No bundles yet."],
  ["safety", "Couldn't load the safety refs:", "No safety refs"],
])("keeps failed %s lists inline and retries to successful empty", async (kind, prefix, empty) => {
  if (kind === "bundles") vi.mocked(api.listBundles).mockRejectedValueOnce(new Error("fatal: denied")).mockResolvedValue([]);
  else vi.mocked(api.listSafetyRefs).mockRejectedValueOnce(new Error("fatal: denied")).mockResolvedValue([]);
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await tick();
  if (kind === "safety") button("Safety refs").click();
  await vi.waitFor(() => expect(target.textContent).toContain(`${prefix} denied`));
  expect(target.textContent).not.toContain(empty);
  expect(errors.pending).toBe(0);
  button("Retry").click();
  await vi.waitFor(() => expect(target.textContent).toContain(empty));
  expect(target.textContent).not.toContain(prefix);
});
it.each([
  ["bundles", "Couldn't load the bundles: denied"],
  ["safety", "Couldn't load the safety refs: denied"],
])("shows a %s retry running when it fails the same way again", async (kind, message) => {
  let fail!: (e: unknown) => void;
  const pending = () => new Promise<never[]>((_, reject) => { fail = reject; });
  if (kind === "bundles") vi.mocked(api.listBundles).mockRejectedValueOnce(new Error("fatal: denied")).mockImplementationOnce(pending);
  else vi.mocked(api.listSafetyRefs).mockRejectedValueOnce(new Error("fatal: denied")).mockImplementationOnce(pending);
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await tick();
  if (kind === "safety") button("Safety refs").click();
  await vi.waitFor(() => expect(target.textContent).toContain(message));
  const retry = () => target.querySelector<HTMLButtonElement>(".load-error button")!;
  retry().click();
  await nextTask();
  expect(retry().disabled).toBe(true);
  expect(retry().textContent).toBe("Retrying…");
  expect(target.textContent).toContain(message);
  fail(new Error("fatal: denied"));
  await vi.waitFor(() => expect(retry().disabled).toBe(false));
  expect(retry().textContent).toBe("Retry");
  expect(target.textContent).toContain(message);
  expect(kind === "bundles" ? api.listBundles : api.listSafetyRefs).toHaveBeenCalledTimes(2);
});
it.each([
  ["create", "Create bundle now", "Couldn't create the bundle"],
  ["delete", "Delete", "Couldn't delete the bundle"],
  ["fetch", "Fetch refs", "Couldn't fetch from the bundle"],
  ["selected", "Delete selected", "Couldn't delete the safety refs"],
  ["all", "Delete all", "Couldn't delete all safety refs"],
])("reports %s action once", async (kind, label, title) => {
  const failure = new Error("fatal: operation denied");
  vi.spyOn(api, "createBundle").mockRejectedValue(failure);
  vi.spyOn(api, "deleteBundle").mockRejectedValue(failure);
  vi.spyOn(api, "fetchBundle").mockRejectedValue(failure);
  vi.spyOn(api, "deleteRefs").mockRejectedValue(failure);
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.querySelector(".row-btn")).not.toBeNull());
  if (kind === "selected" || kind === "all") {
    button("Safety refs").click();
    await vi.waitFor(() => expect(target.textContent).toContain("refs/original/main"));
  }
  if (kind !== "create") {
    target.querySelector<HTMLButtonElement>(".row-btn")!.click();
    await tick();
  }
  button(label).click();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title, headline: "operation denied" }));
  expect(errors.pending).toBe(1);
  expect(appState.statusIsError).toBe(true);
});
it("keeps a post-create list failure inline rather than reporting creation twice", async () => {
  vi.spyOn(api, "createBundle").mockResolvedValue("/tmp/new.bundle");
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.querySelector(".row-btn")).not.toBeNull());
  vi.mocked(api.listBundles).mockRejectedValue(new Error("fatal: list unavailable"));
  button("Create bundle now").click();
  await vi.waitFor(() => expect(target.textContent).toContain("Couldn't load the bundles: list unavailable"));
  expect(errors.pending).toBe(0);
});
it("keeps the current bundle list on screen while a refresh runs", async () => {
  let finish!: () => void;
  vi.mocked(api.listBundles)
    .mockResolvedValueOnce([{ name: "backup.bundle", path: "/tmp/backup.bundle", size: 12 }])
    .mockImplementationOnce(() => new Promise<never[]>((resolve) => { finish = () => resolve([]); }));
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(target.textContent).toContain("backup.bundle"));
  button("Refresh").click();
  await tick();
  expect(target.textContent).toContain("backup.bundle");
  expect(target.textContent).not.toContain("Loading bundles");
  finish();
  await vi.waitFor(() => expect(target.textContent).toContain("No bundles yet."));
});
it("loads the active safety list again after a repository switch", async () => {
  const panel = mount(BackupsPanel, { target, props: { bare: true } });
  dispose = () => unmount(panel);
  await tick();
  button("Safety refs").click();
  await vi.waitFor(() => expect(target.textContent).toContain("refs/original/main"));
  vi.mocked(api.listSafetyRefs).mockResolvedValue([]);
  appState.repo = "/tmp/other-backups";
  await vi.waitFor(() => expect(api.listSafetyRefs).toHaveBeenLastCalledWith("/tmp/other-backups"));
  await vi.waitFor(() => expect(target.textContent).toContain("No safety refs"));
  expect(target.textContent).not.toContain("refs/original/main");
});
