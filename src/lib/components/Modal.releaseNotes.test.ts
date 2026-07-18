// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { mount, tick, unmount } from "svelte";
import { dialogs } from "../dialogs.svelte";
import Modal from "./Modal.svelte";

afterEach(() => {
  if (dialogs.state.kind === "confirm") dialogs.resolveConfirm(false);
  document.body.replaceChildren();
});

describe("confirmation message rendering", () => {
  it("renders opted-in updater notes as structured, scrollable markdown", async () => {
    const target = document.createElement("div");
    document.body.appendChild(target);
    const modal = mount(Modal, { target });
    const pending = dialogs.confirm({
      title: "Update available — 0.3.0-next.20",
      message: [
        "Git It 0.3.0-next.20 is available.",
        "",
        "## Git It 0.3.0-next.20",
        "",
        "### ✨ New features",
        "",
        "- **General** — Add worktree visibility",
        "- **UI** — Render release notes",
      ].join("\n"),
      messageFormat: "markdown",
      confirmLabel: "Download",
    });
    await tick();

    const notes = target.querySelector<HTMLElement>("[data-testid='release-notes']");
    expect(notes).not.toBeNull();
    expect(notes?.querySelector("h2")?.textContent).toBe("Git It 0.3.0-next.20");
    expect(notes?.querySelector("h3")?.textContent).toContain("New features");
    expect(notes?.querySelectorAll("li")).toHaveLength(2);
    expect(notes?.querySelector("strong")?.textContent).toBe("General");
    expect(notes?.textContent).not.toContain("## Git It");

    dialogs.resolveConfirm(false);
    await pending;
    await unmount(modal);
  });

  it("keeps ordinary confirmation messages literal by default", async () => {
    const target = document.createElement("div");
    document.body.appendChild(target);
    const modal = mount(Modal, { target });
    const pending = dialogs.confirm({
      title: "Delete branch",
      message: "Delete **feature/_literal_**?",
    });
    await tick();

    expect(target.querySelector("[data-testid='release-notes']")).toBeNull();
    expect(target.querySelector(".msg")?.textContent).toBe("Delete **feature/_literal_**?");
    expect(target.querySelector(".msg strong")).toBeNull();

    dialogs.resolveConfirm(false);
    await pending;
    await unmount(modal);
  });
});
