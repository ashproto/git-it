// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { api } from "../api";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import RepoList from "./RepoList.svelte";
import RepoTabs from "./RepoTabs.svelte";

vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: vi.fn(), set: vi.fn(), save: vi.fn() })),
}));
const components = [["RepoList", RepoList], ["RepoTabs", RepoTabs]] as const;
let dispose: (() => Promise<void>) | undefined;
let target: HTMLDivElement;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  appState.openRepo("/tmp/recent");
  appState.closeRepo("/tmp/recent");
  target = document.createElement("div");
  document.body.appendChild(target);
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.forgetRepo("/tmp/recent");
  document.body.replaceChildren();
  vi.restoreAllMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

// RepoTabs lists recents in its + menu; RepoList lists them inline.
async function clickRecent(Component: typeof RepoList | typeof RepoTabs): Promise<void> {
  if (Component === RepoTabs) {
    target.querySelector<HTMLButtonElement>(".add-btn")!.click();
    await tick();
  }
  const recent = target.querySelector<HTMLButtonElement>('button[title="/tmp/recent"]');
  expect(recent).not.toBeNull();
  recent!.click();
}

it.each(components)("%s reports rejected recent validation and notices non-repositories", async (_name, Component) => {
  const probe = vi.spyOn(api, "isGitRepo").mockRejectedValueOnce(new Error("fatal: permission denied")).mockResolvedValue(false);
  const panel = mount(Component, { target });
  dispose = () => unmount(panel);
  await tick();
  for (const kind of ["error", "notice"]) {
    await clickRecent(Component);
    await vi.waitFor(() => expect(errors.current?.kind).toBe(kind));
    expect(errors.pending).toBe(1);
    expect(errors.current?.title).toBe(kind === "error" ? "Couldn't open the repository" : "Not a git repository");
    expect(appState.statusIsError).toBe(kind === "error");
    errors.reset();
  }
  expect(probe).toHaveBeenCalledTimes(2);
  expect(appState.openRepos).not.toContain("/tmp/recent");
});

it.each(components)("%s opens a valid recent without a report", async (_name, Component) => {
  vi.spyOn(api, "isGitRepo").mockResolvedValue(true);
  const panel = mount(Component, { target });
  dispose = () => unmount(panel);
  await tick();
  await clickRecent(Component);
  await vi.waitFor(() => expect(appState.repo).toBe("/tmp/recent"));
  expect(errors.pending).toBe(0);
});
