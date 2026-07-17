// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { mount, tick, unmount } from "svelte";
import { dialogs } from "../dialogs.svelte";
import type { WorktreeInfo } from "../types";
import Modal from "./Modal.svelte";

function cleanLinkedWorktree(): WorktreeInfo {
  return {
    path: "/tmp/git-it-linked-worktree",
    head: "a".repeat(40),
    branch: "feature",
    isMain: false,
    isCurrent: false,
    detached: false,
    bare: false,
    locked: false,
    lockedReason: null,
    prunable: false,
    prunableReason: null,
    status: {
      head: { sha: "a".repeat(40), branch: "feature", detached: false },
      staged: 0,
      unstaged: 0,
      untracked: 0,
      conflicted: 0,
      operation: null,
    },
  };
}

afterEach(() => {
  if (dialogs.state.kind === "branchDelete") dialogs.resolveBranchDelete(false);
  document.body.replaceChildren();
});

describe("branch-delete dialog focus", () => {
  it("keeps focus on an option after it is toggled", async () => {
    const target = document.createElement("div");
    document.body.appendChild(target);
    const modal = mount(Modal, { target });
    const pending = dialogs.confirmBranchDelete({
      branch: "feature",
      upstream: "origin/feature",
      worktree: cleanLinkedWorktree(),
    });
    await tick();

    const cancel = target.querySelector<HTMLButtonElement>(".actions button");
    expect(document.activeElement).toBe(cancel);
    const removeWorktree = target.querySelector<HTMLInputElement>(".worktree-opt-in input");
    if (!removeWorktree) throw new Error("worktree opt-in checkbox did not render");

    removeWorktree.focus();
    removeWorktree.checked = true;
    removeWorktree.dispatchEvent(new Event("change", { bubbles: true }));
    await tick();

    expect(document.activeElement).toBe(removeWorktree);
    dialogs.resolveBranchDelete(false);
    await pending;
    await unmount(modal);
  });
});
