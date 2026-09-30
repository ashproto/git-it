// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { SvelteMap } from "svelte/reactivity";
import { errors } from "../errors.svelte";
import { appState } from "../store.svelte";
import LoadError from "./LoadError.svelte";
let dispose: (() => Promise<void>) | undefined;
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  appState.clearLog();
  document.body.replaceChildren();
  vi.restoreAllMocks();
});
it("shows only the first summarized headline line, selectable, with Retry", () => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const retry = vi.fn();
  const panel = mount(LoadError, { target, props: { what: "the stashes", error: new Error("From example\nfatal: cannot read\n\tfile.txt\nhint: try later"), onRetry: retry } });
  dispose = () => unmount(panel);
  expect(target.querySelector(".selectable")?.textContent).toBe("Couldn't load the stashes: cannot read");
  expect(target.textContent).not.toContain("file.txt");
  expect(target.querySelector('[role="status"]')?.getAttribute("aria-live")).toBe("polite");
  const button = target.querySelector<HTMLButtonElement>("button")!;
  // Names what it retries: a broken repository fails several panels at once.
  expect(button.getAttribute("aria-label")).toBe("Retry loading the stashes");
  button.click();
  expect(retry).toHaveBeenCalledOnce();
  // No arguments: the click event is not passed on.
  expect(retry).toHaveBeenCalledWith();
  expect(errors.pending).toBe(0);
});
it.each([null, undefined])("renders an empty rejection (%s) as a real failure", (error) => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(LoadError, { target, props: { what: "the diff", error, onRetry: () => {} } });
  dispose = () => unmount(panel);
  expect(target.textContent).toContain("Couldn't load the diff: The operation failed without an error message.");
});
// A whole task: every queued microtask has run, including the ones Svelte's dev build adds
// around each `await`, so only a retry that is really still pending can show as busy.
const nextTask = () => new Promise((resolve) => setTimeout(resolve));
function mountRetry(onRetry: () => Promise<unknown>): HTMLButtonElement {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(LoadError, { target, props: { what: "the stashes", error: new Error("fatal: cannot read"), onRetry } });
  dispose = () => unmount(panel);
  return target.querySelector<HTMLButtonElement>("button")!;
}
it("disables Retry and reads Retrying… until the retry settles", async () => {
  let finish!: () => void;
  const button = mountRetry(() => new Promise<void>((resolve) => { finish = resolve; }));
  button.click();
  await nextTask();
  expect(button.disabled).toBe(true);
  expect(button.textContent).toBe("Retrying…");
  finish();
  await vi.waitFor(() => expect(button.disabled).toBe(false));
  expect(button.textContent).toBe("Retry");
});
it("re-enables Retry when the retry rejects", async () => {
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  const failure = new Error("loader bug");
  let fail!: (e: unknown) => void;
  const button = mountRetry(() => new Promise<void>((_, reject) => { fail = reject; }));
  button.click();
  await nextTask();
  expect(button.disabled).toBe(true);
  fail(failure);
  await vi.waitFor(() => expect(button.disabled).toBe(false));
  expect(button.textContent).toBe("Retry");
  // Logged, not left as an unhandled rejection.
  expect(warn).toHaveBeenCalledWith("[gte] retry loading the stashes failed", failure);
});
// Mounts with a failure the test can swap, the way a panel hands over each failed load.
function mountFailure(error: unknown): (next: unknown) => void {
  const failure = new SvelteMap<"error", unknown>([["error", error]]);
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(LoadError, {
    target,
    props: {
      what: "the stashes",
      get error() {
        return failure.get("error");
      },
      onRetry: () => {},
    },
  });
  dispose = () => unmount(panel);
  flushSync();
  return (next) => {
    failure.set("error", next);
    flushSync();
  };
}
const loadLines = () => appState.logLines.filter((line) => line.startsWith("[load] "));
it("logs the whole failure to the Output log", () => {
  const raw = "From example\nfatal: cannot read\n\tfile.txt\nhint: try later";
  mountFailure(new Error(raw));
  expect(loadLines()).toEqual([`[load] Couldn't load the stashes\n${raw}`]);
});
it("doesn't log a refresh that fails the same way again", () => {
  const fail = mountFailure(new Error("fatal: cannot read"));
  fail(new Error("fatal: cannot read"));
  fail(new Error("fatal: cannot read"));
  expect(loadLines()).toEqual(["[load] Couldn't load the stashes\nfatal: cannot read"]);
});
it("logs a refresh that fails differently", () => {
  const fail = mountFailure(new Error("fatal: cannot read"));
  fail(new Error("fatal: permission denied"));
  expect(loadLines()).toEqual([
    "[load] Couldn't load the stashes\nfatal: cannot read",
    "[load] Couldn't load the stashes\nfatal: permission denied",
  ]);
});
it("logs the same failure again when mounted for a new failure", async () => {
  // A panel unmounts this on a successful load, so a later failure mounts it afresh.
  mountFailure(new Error("fatal: cannot read"));
  await dispose?.();
  mountFailure(new Error("fatal: cannot read"));
  expect(loadLines()).toHaveLength(2);
});
it("logs without re-running on its own write", () => {
  const warn = vi.spyOn(console, "warn");
  const error = vi.spyOn(console, "error");
  expect(() => mountFailure(new Error("fatal: cannot read"))).not.toThrow();
  expect(loadLines()).toHaveLength(1);
  expect(warn).not.toHaveBeenCalled();
  expect(error).not.toHaveBeenCalled();
});
