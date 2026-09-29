// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import { contextMenu } from "../contextMenu.svelte";
import { SAMPLE_GRAPH } from "../graph/sample";
import WorkingCopyView from "./WorkingCopyView.svelte";
import WorktreePanel from "./WorktreePanel.svelte";
import GraphHistory from "./GraphHistory.svelte";
import PrCommitsTab from "./github/PrCommitsTab.svelte";

vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn(), revealItemInDir: vi.fn() }));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  // DiffView reads window.matchMedia at init and jsdom has none; a right-click selects the
  // file, its (empty) diff loads, and DiffView mounts.
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  appState.repo = "/tmp/repo";
  appState.setRepoLoading(false);
  appState.setGraphCommits(SAMPLE_GRAPH.slice(0, 1));
  appState.setFileTreeView(false);
  vi.spyOn(api, "diff").mockResolvedValue("");
  vi.spyOn(api, "appsForFile").mockResolvedValue([{ name: "TextEdit", path: "/Applications/TextEdit.app" }]);
  vi.mocked(openPath).mockReset().mockRejectedValue(new Error("fatal: launch denied"));
  vi.mocked(revealItemInDir).mockReset().mockRejectedValue(new Error("fatal: launch denied"));
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockRejectedValue(new Error("fatal: copy denied")) } });
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  contextMenu.close();
  errors.reset();
  appState.repo = "";
  document.body.replaceChildren();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  if (clipboard) Object.defineProperty(navigator, "clipboard", clipboard);
  else Reflect.deleteProperty(navigator, "clipboard");
});

it.each([
  ["Open", false, "Couldn't open the file"],
  ["Open With", false, "Couldn't open the file"],
  ["Show in Finder", false, "Couldn't show in Finder"],
  ["Show in Finder", true, "Couldn't show in Finder"],
] as const)("reports %s failure (deleted=%s)", async (label, deleted, title) => {
  appState.setWorkingChanges([{ path: "f.txt", status: deleted ? "deleted" : "modified", staged: false, unstaged: true, untracked: false, conflicted: false }]);
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector(".file-row")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  await vi.waitFor(() => expect(contextMenu.open).toBe(true));
  const item = contextMenu.items.find((item) => item.label === label)!;
  (label === "Open With" ? item.submenu![0] : item).action!();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title, headline: "launch denied" }));
  expect(errors.pending).toBe(1);
  if (label === "Open With") expect(openPath).toHaveBeenCalledWith("/tmp/repo/f.txt", "/Applications/TextEdit.app");
  if (label === "Show in Finder" && deleted) expect(openPath).toHaveBeenCalledWith("/tmp/repo");
  if (label === "Show in Finder" && !deleted) expect(revealItemInDir).toHaveBeenCalledWith("/tmp/repo/f.txt");
});
it("keeps failed app discovery as silent submenu omission", async () => {
  vi.mocked(api.appsForFile).mockRejectedValue(new Error("discovery failed"));
  appState.setWorkingChanges([{ path: "f.txt", status: "modified", staged: false, unstaged: true, untracked: false, conflicted: false }]);
  const panel = mount(WorkingCopyView, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector(".file-row")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  await vi.waitFor(() => expect(contextMenu.open).toBe(true));
  expect(contextMenu.items.some((item) => item.label === "Open With")).toBe(false);
  expect(errors.pending).toBe(0);
});
it("reports Open Worktree Folder failure", async () => {
  const panel = mount(WorktreePanel, { target, props: { worktrees: [{ path: "/tmp/linked", head: "a", branch: "feature", isMain: false, isCurrent: false, detached: false, bare: false, locked: false, lockedReason: null, prunable: false, prunableReason: null, status: null }] } });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector(".worktree")!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  contextMenu.items.find((item) => item.label === "Open Worktree Folder")!.action!();
  await vi.waitFor(() => expect(errors.current?.title).toBe("Couldn't open the worktree folder"));
});
it.each([false, true])("reports graph copy failure, missing clipboard=%s", async (missing) => {
  if (missing) Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
  const panel = mount(GraphHistory, { target });
  dispose = () => unmount(panel);
  await tick();
  target.querySelector('[id^="gc-row-"]')!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  contextMenu.items.find((item) => item.label === "Copy SHA")!.action!();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't copy the SHA", headline: missing ? "Clipboard is unavailable." : "copy denied" }));
});
it.each([false, true])("reports PR copy failure without success feedback, missing clipboard=%s", async (missing) => {
  if (missing) Object.defineProperty(navigator, "clipboard", { configurable: true, value: undefined });
  const panel = mount(PrCommitsTab, { target, props: { commits: [{ oid: "a".repeat(40), message: "Subject", author: "A", committedDate: "2026-09-26T00:00:00Z" }] } });
  dispose = () => unmount(panel);
  target.querySelector<HTMLButtonElement>('[title="Copy full SHA"]')!.click();
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't copy the SHA", headline: missing ? "Clipboard is unavailable." : "copy denied" }));
  expect(target.textContent).not.toContain("✓ copied");
});
