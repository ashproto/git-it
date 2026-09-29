// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { appState } from "../store.svelte";
import { errors } from "../errors.svelte";
import { gitActions } from "../gitActions";
import { SAMPLE_GRAPH } from "../graph/sample";
import CommitComposer from "./CommitComposer.svelte";
import CommitMessageEdit from "./CommitMessageEdit.svelte";

let dispose: (() => Promise<void>) | undefined;
beforeEach(() => {
  appState.setGraphCommits([{ ...SAMPLE_GRAPH[0], refs: [{ name: "main", kind: "local", is_head: true }] }]);
  appState.selected = new Set([SAMPLE_GRAPH[0].sha]);
  vi.spyOn(gitActions, "getCommitMessage").mockRejectedValue(new Error("fatal: missing object"));
});
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.setGraphCommits([]);
  document.body.replaceChildren();
  vi.restoreAllMocks();
});
it("reports amend loading failure, unticks, and keeps the user's draft", async () => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(CommitComposer, { target });
  dispose = () => unmount(panel);
  const title = target.querySelector<HTMLInputElement>(".title-box")!;
  title.value = "My draft";
  title.dispatchEvent(new Event("input", { bubbles: true }));
  await tick();
  const checkbox = target.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
  checkbox.checked = true;
  checkbox.dispatchEvent(new Event("change", { bubbles: true }));
  await vi.waitFor(() => expect(errors.current?.headline).toBe("missing object"));
  await tick();
  expect(errors.current?.title).toBe("Couldn't load the commit message");
  expect(errors.pending).toBe(1);
  expect(checkbox.checked).toBe(false);
  expect(title.value).toBe("My draft");
});
it("reports selected commit-message loading failure", async () => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(CommitMessageEdit, { target });
  dispose = () => unmount(panel);
  await vi.waitFor(() => expect(errors.current).toMatchObject({ kind: "error", title: "Couldn't load the commit message", headline: "missing object" }));
  expect(appState.statusIsError).toBe(true);
});
it("ignores an obsolete amend load failure after the user unticks", async () => {
  let reject!: (error: unknown) => void;
  vi.mocked(gitActions.getCommitMessage).mockImplementation(() => new Promise<string>((_, fail) => { reject = fail; }));
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(CommitComposer, { target });
  dispose = () => unmount(panel);
  const checkbox = target.querySelector<HTMLInputElement>('input[type="checkbox"]')!;
  checkbox.checked = true;
  checkbox.dispatchEvent(new Event("change", { bubbles: true }));
  await tick();
  checkbox.checked = false;
  checkbox.dispatchEvent(new Event("change", { bubbles: true }));
  reject(new Error("old failure"));
  await tick();
  await Promise.resolve();
  expect(errors.pending).toBe(0);
});
