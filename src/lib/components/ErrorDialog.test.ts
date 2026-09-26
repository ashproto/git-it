// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import { errors, reportError, reportNotice } from "../errors.svelte";
import ErrorDialog from "./ErrorDialog.svelte";

const DIVERGENT = [
  "From /tmp/origin",
  "   d728e86..b362e35  main       -> origin/main",
  "hint: You have divergent branches and need to specify how to reconcile them.",
  "hint:   git config pull.rebase false  # merge",
  "fatal: Need to specify how to reconcile divergent branches.",
].join("\n");

let component: ReturnType<typeof mount> | null = null;

function render(): HTMLElement {
  const target = document.createElement("div");
  document.body.appendChild(target);
  component = mount(ErrorDialog, { target });
  return target;
}

afterEach(() => {
  if (component) unmount(component);
  component = null;
  errors.reset();
  Reflect.deleteProperty(navigator, "clipboard");
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

describe("ErrorDialog", () => {
  it("renders nothing until something is reported", async () => {
    const target = render();
    await tick();
    expect(target.querySelector("[role=alertdialog]")).toBeNull();
  });

  it("leads with git's real error and shows its hints", async () => {
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    expect(target.querySelector("h3")?.textContent).toBe("Pull failed");
    expect(target.querySelector(".headline-text")?.textContent).toBe(
      "Need to specify how to reconcile divergent branches.",
    );
    expect(target.querySelector(".hints")?.textContent).toContain("git config pull.rebase false");
    expect(target.querySelector(".glyph")?.textContent).toBe("⚠");
  });

  it("keeps the raw output behind a disclosure", async () => {
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    expect(target.querySelector(".output")).toBeNull();
    target.querySelector<HTMLButtonElement>(".disclosure")?.click();
    await tick();
    expect(target.querySelector(".output")?.textContent).toContain("From /tmp/origin");
  });

  it("offers no disclosure when the output is only the headline", async () => {
    const target = render();
    reportError("Push failed", "A remote operation is already in progress.");
    await tick();
    expect(target.querySelector(".disclosure")).toBeNull();
  });

  it("focuses OK and advances to the next queued report", async () => {
    const target = render();
    reportError("Pull failed", DIVERGENT);
    reportError("Push failed", "error: failed to push some refs to 'origin'");
    await tick();
    expect(target.querySelector(".count")?.textContent).toBe("1 of 2");
    const ok = target.querySelector<HTMLButtonElement>("button.primary");
    expect(document.activeElement).toBe(ok);
    ok?.click();
    await tick();
    expect(target.querySelector("h3")?.textContent).toBe("Push failed");
    expect(target.querySelector(".count")).toBeNull();
  });

  it("closes on Escape and returns focus to where it was", async () => {
    const opener = document.createElement("button");
    document.body.appendChild(opener);
    opener.focus();
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    target
      .querySelector(".overlay")
      ?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await tick();
    expect(errors.current).toBeNull();
    expect(target.querySelector("[role=alertdialog]")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("keeps its keys from reaching window-level shortcuts", async () => {
    const target = render();
    const onWindowKey = vi.fn();
    window.addEventListener("keydown", onWindowKey);
    reportError("Pull failed", DIVERGENT);
    await tick();
    target
      .querySelector(".overlay")
      ?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    window.removeEventListener("keydown", onWindowKey);
    expect(onWindowKey).not.toHaveBeenCalled();
  });

  it("shows a notice with an info glyph and no copy button", async () => {
    const target = render();
    reportNotice("Can't reword this commit", "Can't reword a merge commit here.");
    await tick();
    expect(target.querySelector(".glyph")?.textContent).toBe("ⓘ");
    expect(target.querySelector(".copy")).toBeNull();
  });

  it("copies the title, headline and raw output", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    const copy = target.querySelector<HTMLButtonElement>(".copy");
    copy?.click();
    await vi.waitFor(() => expect(copy?.textContent).toBe("Copied"));
    expect(writeText).toHaveBeenCalledWith(
      `Pull failed\nNeed to specify how to reconcile divergent branches.\n\n${DIVERGENT}`,
    );
  });

  it("says so when copying is unavailable", async () => {
    Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
    const target = render();
    reportError("Pull failed", DIVERGENT);
    await tick();
    const copy = target.querySelector<HTMLButtonElement>(".copy");
    copy?.click();
    await vi.waitFor(() => expect(copy?.textContent).toBe("Copy failed"));
  });
});
