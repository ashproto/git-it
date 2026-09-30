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
    source:
      upstream === null
        ? { kind: "none" }
        : { kind: "tracked", upstream, gone: false, ahead: 2, behind: 3 },
    strategy: "rebase",
    autostash: true,
    offerDontAsk: true,
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

  it("offers no 'Don't ask again' when asking is already off", async () => {
    // An ⌥-click with asking off still opens the options, but ticking it would change nothing.
    const { target, modal } = mountModal();
    const pending = dialogs.confirmPull({
      branch: "main",
      source: { kind: "tracked", upstream: "origin/main", gone: false, ahead: 2, behind: 3 },
      strategy: "rebase",
      autostash: true,
      offerDontAsk: false,
    });
    await tick();

    expect(target.querySelector(".pull-autostash")).not.toBeNull();
    expect(target.querySelector(".pull-dont-ask")).toBeNull();
    expect(target.textContent).not.toContain("Don't ask again");
    expect(target.textContent).not.toContain("Change this in Settings");

    target.querySelector<HTMLButtonElement>(".actions button.primary")?.click();
    await expect(pending).resolves.toEqual({ strategy: "rebase", autostash: true, dontAsk: false });
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

  it("keeps Enter working after a click moves focus to the dialog itself", async () => {
    // macOS WebKit doesn't focus a clicked button, radio, checkbox or label: focus goes to the
    // nearest focusable ancestor instead. The dialog is that ancestor (tabindex -1), so the
    // next key still starts inside the overlay rather than on <body>.
    const { target, modal } = mountModal();
    const pending = openPull("origin/main");
    await tick();
    const dialog = target.querySelector<HTMLElement>(".dialog.pull");
    expect(dialog?.getAttribute("tabindex")).toBe("-1");
    dialog?.focus();
    expect(document.activeElement).toBe(dialog);

    keydown(dialog, "Enter");
    await expect(pending).resolves.toEqual({ strategy: "rebase", autostash: true, dontAsk: false });
    await unmount(modal);
  });

  it("cancels on a backdrop press but not on a press inside the dialog", async () => {
    const { target, modal } = mountModal();
    const pending = openPull("origin/main");
    await tick();
    target.querySelector(".dialog.pull")?.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    expect(dialogs.state.kind).toBe("pull");
    target.querySelector(".overlay")?.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    await expect(pending).resolves.toBeNull();
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

  it("disables Pull and its options and explains when the upstream no longer exists", async () => {
    const { target, modal } = mountModal();
    const pending = dialogs.confirmPull({
      branch: "main",
      source: { kind: "tracked", upstream: "origin/main", gone: true, ahead: 0, behind: 0 },
      strategy: "rebase",
      autostash: true,
      offerDontAsk: true,
    });
    await tick();

    const status = target.querySelector(".pull-status");
    expect(status?.textContent?.trim()).toBe(
      "origin/main no longer exists as of the last fetch — there's nothing to pull.",
    );
    expect(status?.classList.contains("blocked")).toBe(true);
    // The status line describes the dialog, so a screen reader announces why Pull is off.
    expect(status?.id).toBeTruthy();
    expect(target.querySelector(".dialog.pull")?.getAttribute("aria-describedby")).toBe(status?.id);
    const pull = target.querySelector<HTMLButtonElement>(".actions button.primary");
    expect(pull?.disabled).toBe(true);
    // Options that can't take effect must not look usable.
    const options = target.querySelectorAll<HTMLInputElement>(
      ".strategy-opt input, .pull-autostash input, .pull-dont-ask input",
    );
    expect([...options].map((o) => o.disabled)).toEqual([true, true, true, true, true]);
    const cancel = target.querySelector<HTMLButtonElement>(".actions button");
    expect(document.activeElement).toBe(cancel);

    keydown(cancel, "Escape");
    await expect(pending).resolves.toBeNull();
    await unmount(modal);
  });

  it("disables Pull and explains when HEAD is detached", async () => {
    const { target, modal } = mountModal();
    const pending = dialogs.confirmPull({
      branch: null,
      source: { kind: "none" },
      strategy: "rebase",
      autostash: true,
      offerDontAsk: true,
    });
    await tick();

    const status = target.querySelector(".pull-status");
    expect(status?.textContent?.trim()).toBe("No branch is checked out.");
    expect(status?.classList.contains("blocked")).toBe(true);
    expect(target.querySelector<HTMLButtonElement>(".actions button.primary")?.disabled).toBe(true);

    keydown(target.querySelector(".actions button"), "Escape");
    await expect(pending).resolves.toBeNull();
    await unmount(modal);
  });

  it("enables Pull for a merge source that no tracking ref mirrors, without counts", async () => {
    // A fork's pull request checked out through gh: git pulls refs/pull/7/head from origin, and
    // nothing local mirrors it to count ahead/behind against.
    const { target, modal } = mountModal();
    const pending = dialogs.confirmPull({
      branch: "pr-7",
      source: { kind: "configured", source: "origin refs/pull/7/head" },
      strategy: "rebase",
      autostash: true,
      offerDontAsk: true,
    });
    await tick();

    expect(target.querySelector("h3")?.textContent?.trim()).toBe(
      "Pull origin refs/pull/7/head → pr-7",
    );
    const status = target.querySelector(".pull-status");
    expect(status?.textContent?.trim()).toBe(
      "pr-7 pulls from origin refs/pull/7/head, as configured — ahead/behind isn't known.",
    );
    expect(status?.classList.contains("blocked")).toBe(false);
    const options = target.querySelectorAll<HTMLInputElement>(
      ".strategy-opt input, .pull-autostash input, .pull-dont-ask input",
    );
    expect([...options].map((o) => o.disabled)).toEqual([false, false, false, false, false]);
    const pull = target.querySelector<HTMLButtonElement>(".actions button.primary");
    expect(pull?.disabled).toBe(false);
    expect(document.activeElement).toBe(pull);

    keydown(pull, "Enter");
    await expect(pending).resolves.toEqual({ strategy: "rebase", autostash: true, dontAsk: false });
    await unmount(modal);
  });
});
