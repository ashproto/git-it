// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { mount, unmount } from "svelte";
import { errors } from "../errors.svelte";
import LoadError from "./LoadError.svelte";
let dispose: (() => Promise<void>) | undefined;
afterEach(async () => {
  await dispose?.();
  dispose = undefined;
  errors.reset();
  document.body.replaceChildren();
});
it("shows only the first summarized headline line, selectable, with Retry", () => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const retry = vi.fn();
  const panel = mount(LoadError, { target, props: { what: "the stashes", error: new Error("From example\nfatal: cannot read\n\tfile.txt\nhint: try later"), onRetry: retry } });
  dispose = () => unmount(panel);
  expect(target.querySelector(".selectable")?.textContent).toBe("Couldn't load the stashes: cannot read");
  expect(target.textContent).not.toContain("file.txt");
  target.querySelector<HTMLButtonElement>("button")!.click();
  expect(retry).toHaveBeenCalledOnce();
  expect(errors.pending).toBe(0);
});
it.each([null, undefined])("renders an empty rejection (%s) as a real failure", (error) => {
  const target = document.createElement("div");
  document.body.appendChild(target);
  const panel = mount(LoadError, { target, props: { what: "the diff", error, onRetry: () => {} } });
  dispose = () => unmount(panel);
  expect(target.textContent).toContain("Couldn't load the diff: The operation failed without an error message.");
});
