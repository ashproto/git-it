// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { mount, tick, unmount } from "svelte";
import { errors, reportError } from "../errors.svelte";
import { appState } from "../store.svelte";
import StatusBar from "./StatusBar.svelte";

let component: ReturnType<typeof mount> | null = null;

function render(): HTMLElement {
  const target = document.createElement("div");
  document.body.appendChild(target);
  component = mount(StatusBar, { target });
  return target;
}

afterEach(() => {
  if (component) unmount(component);
  component = null;
  errors.reset();
  appState.status = "Ready";
  document.body.replaceChildren();
});

describe("StatusBar error state", () => {
  it("shows an ordinary message as plain text", async () => {
    appState.status = "Fetch — done.";
    const target = render();
    await tick();
    expect(target.querySelector("button.status-error")).toBeNull();
    expect(target.querySelector(".status-text")?.textContent).toBe("Fetch — done.");
  });

  it("turns a failure into a button that reopens the error", async () => {
    const target = render();
    reportError("Pull failed", "fatal: Need to specify how to reconcile divergent branches.");
    errors.dismiss();
    await tick();
    const button = target.querySelector<HTMLButtonElement>("button.status-error");
    expect(button?.textContent).toContain(
      "Pull failed: Need to specify how to reconcile divergent branches.",
    );
    expect(button?.title).toBe("Show error details");
    button?.click();
    expect(errors.current?.title).toBe("Pull failed");
  });

  it("drops the error state on the next ordinary message", async () => {
    const target = render();
    reportError("Pull failed", "fatal: boom");
    await tick();
    expect(target.querySelector("button.status-error")).not.toBeNull();
    appState.status = "Fetch — done.";
    await tick();
    expect(target.querySelector("button.status-error")).toBeNull();
    expect(target.querySelector(".status-text")?.textContent).toBe("Fetch — done.");
  });
});
