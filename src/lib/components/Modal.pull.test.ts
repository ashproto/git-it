// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { mount, tick, unmount } from "svelte";
import { dialogs } from "../dialogs.svelte";
import Modal from "./Modal.svelte";

afterEach(() => {
  if (dialogs.state.kind === "pull") dialogs.resolvePull(false);
  document.body.replaceChildren();
});

function mountModal() {
  const target = document.createElement("div");
  document.body.appendChild(target);
  return { target, modal: mount(Modal, { target }) };
}

function openPull(upstream: string | null) {
  return dialogs.confirmPull({
    branch: "main",
    upstream,
    upstreamGone: false,
    ahead: 2,
    behind: 3,
    strategy: "rebase",
    autostash: true,
  });
}

function keydown(el: Element | null | undefined, key: string) {
  el?.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
}

describe("pull dialog", () => {
  it("preselects the saved strategy and autostash, then resolves the edited choice", async () => {
    const { target, modal } = mountModal();
    const pending = openPull("origin/main");
    await tick();

    expect(target.querySelector("h3")?.textContent?.trim()).toBe("Pull origin/main → main");
    expect(target.querySelector(".pull-status")?.textContent?.trim()).toBe(
      "main is 2 ahead, 3 behind origin/main as of the last fetch.",
    );
    const radios = [...target.querySelectorAll<HTMLInputElement>(".strategy-opt input")];
    expect(radios.map((r) => r.value)).toEqual(["merge", "rebase", "ffOnly"]);
    expect(radios.map((r) => r.checked)).toEqual([false, true, false]);
    expect(target.querySelector<HTMLInputElement>(".pull-autostash input")?.checked).toBe(true);
    expect(target.querySelector<HTMLInputElement>(".pull-dont-ask input")?.checked).toBe(false);
    const pull = target.querySelector<HTMLButtonElement>(".actions button.primary");
    expect(pull?.disabled).toBe(false);
    expect(document.activeElement).toBe(pull);

    radios[2].click();
    target.querySelector<HTMLInputElement>(".pull-dont-ask input")?.click();
    await tick();
    expect(radios.map((r) => r.checked)).toEqual([false, false, true]);
    pull?.click();

    await expect(pending).resolves.toEqual({ strategy: "ffOnly", autostash: true, dontAsk: true });
    await unmount(modal);
  });

  it("pulls on Enter and cancels on Escape", async () => {
    const { target, modal } = mountModal();
    const first = openPull("origin/main");
    await tick();
    keydown(target.querySelector(".actions button.primary"), "Enter");
    await expect(first).resolves.toEqual({ strategy: "rebase", autostash: true, dontAsk: false });

    const second = openPull("origin/main");
    await tick();
    keydown(target.querySelector(".actions button.primary"), "Escape");
    await expect(second).resolves.toBeNull();
    await unmount(modal);
  });

  it("disables Pull and explains when the branch has no upstream", async () => {
    const { target, modal } = mountModal();
    const pending = openPull(null);
    await tick();

    expect(target.querySelector("h3")?.textContent?.trim()).toBe("Pull");
    const status = target.querySelector(".pull-status");
    expect(status?.textContent?.trim()).toBe(
      "main has no upstream branch — push it first to set one.",
    );
    expect(status?.classList.contains("blocked")).toBe(true);
    const pull = target.querySelector<HTMLButtonElement>(".actions button.primary");
    expect(pull?.disabled).toBe(true);
    const cancel = target.querySelector<HTMLButtonElement>(".actions button");
    expect(document.activeElement).toBe(cancel);

    keydown(cancel, "Enter");
    expect(dialogs.state.kind).toBe("pull"); // Enter cannot pull either
    keydown(cancel, "Escape");
    await expect(pending).resolves.toBeNull();
    await unmount(modal);
  });

  it("disables Pull and explains when the upstream no longer exists", async () => {
    const { target, modal } = mountModal();
    const pending = dialogs.confirmPull({
      branch: "main",
      upstream: "origin/main",
      upstreamGone: true,
      ahead: 0,
      behind: 0,
      strategy: "rebase",
      autostash: true,
    });
    await tick();

    const status = target.querySelector(".pull-status");
    expect(status?.textContent?.trim()).toBe(
      "origin/main no longer exists as of the last fetch — there's nothing to pull.",
    );
    expect(status?.classList.contains("blocked")).toBe(true);
    const pull = target.querySelector<HTMLButtonElement>(".actions button.primary");
    expect(pull?.disabled).toBe(true);
    const cancel = target.querySelector<HTMLButtonElement>(".actions button");
    expect(document.activeElement).toBe(cancel);

    keydown(cancel, "Escape");
    await expect(pending).resolves.toBeNull();
    await unmount(modal);
  });
});
