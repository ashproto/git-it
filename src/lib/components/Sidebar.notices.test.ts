// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import { dialogs } from "../dialogs.svelte";
import { contextMenu } from "../contextMenu.svelte";
import { gitActions } from "../gitActions";
import { SAMPLE_GRAPH } from "../graph/sample";
import Sidebar from "./Sidebar.svelte";
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn() }));
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.repo = "/tmp/sidebar";
  appState.setGraphCommits([{ ...SAMPLE_GRAPH[0], refs: [{ name: "main", kind: "local", is_head: true }] }]);
  appState.setRefsDetailed([{ name: "main", kind: "local", target_sha: SAMPLE_GRAPH[0].sha, upstream: null, ahead: 0, behind: 0 }]);
  appState.setRemotes([]);
  appState.setWorktrees([]);
  appState.setRepoLoading(false);
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  dialogs.resolvePrompt(null);
  await dispose?.();
  dispose = undefined;
  contextMenu.close();
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});
it("notices loading refusal before opening a ref context menu", async () => {
  appState.setRepoLoading(true);
  const panel = mount(Sidebar, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector('button.ref[title="main"]')!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  expect(contextMenu.open).toBe(false);
  expect(errors.current).toMatchObject({ kind: "notice", title: "Repository still opening", headline: "Still opening the repository — try that again in a moment." });
  expect(appState.statusIsError).toBe(false);
});
it("notices changed repository after a rename prompt and makes no write", async () => {
  const rename = vi.spyOn(gitActions, "renameBranch");
  const panel = mount(Sidebar, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector('button.ref[title="main"]')!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  const pending = contextMenu.items.find((item) => item.label === "Rename…")!.action!();
  expect(dialogs.state.kind).toBe("prompt");
  appState.repo = "/tmp/other-sidebar";
  dialogs.resolvePrompt("renamed");
  await pending;
  expect(rename).not.toHaveBeenCalled();
  expect(errors.current).toMatchObject({ kind: "notice", title: "Repository changed", headline: "Repository changed while that dialog was open — nothing was done." });
  expect(errors.pending).toBe(1);
  expect(appState.status).toBe("Repository changed while that dialog was open — nothing was done.");
});
it("raises no notice for a tag double-click while the repository opens", async () => {
  const checkout = vi.spyOn(gitActions, "checkout").mockResolvedValue(true);
  appState.setRefsDetailed([
    { name: "main", kind: "local", target_sha: SAMPLE_GRAPH[0].sha, upstream: null, ahead: 0, behind: 0 },
    { name: "v1.0", kind: "tag", target_sha: SAMPLE_GRAPH[0].sha, upstream: null, ahead: 0, behind: 0 },
  ]);
  appState.setRepoLoading(true);
  const panel = mount(Sidebar, { target });
  dispose = () => unmount(panel);
  await tick();
  // Double-clicking a tag only jumps to it, so there is nothing to refuse.
  target.querySelector('button.ref[title="v1.0"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  expect(errors.pending).toBe(0);
  // A branch's double-click checks it out, so it is still refused.
  target.querySelector('button.ref[title="main"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  expect(errors.current).toMatchObject({ kind: "notice", title: "Repository still opening" });
  expect(checkout).not.toHaveBeenCalled();
});
