// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import type { Ref, WorktreeInfo } from "../types";
import { contextMenu } from "../contextMenu.svelte";

vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn() }));
vi.mock("../store.svelte", () => ({
  appState: { openRepo: vi.fn(), setActiveView: vi.fn() },
}));
vi.mock("../dialogs.svelte", () => ({
  dialogs: { confirm: vi.fn().mockResolvedValue(false) },
}));
vi.mock("../gitActions", () => ({
  gitActions: { removeWorktree: vi.fn() },
}));

import WorktreePanel from "./WorktreePanel.svelte";

function worktree(overrides: Partial<WorktreeInfo> = {}): WorktreeInfo {
  return {
    path: "/tmp/linked",
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
      staged: 1,
      unstaged: 0,
      untracked: 0,
      conflicted: 0,
      operation: null,
    },
    ...overrides,
  };
}

afterEach(() => {
  contextMenu.close();
  document.body.replaceChildren();
});

describe("linked worktree panel", () => {
  it("hides the primary checkout and opens an app-owned management menu", async () => {
    const target = document.createElement("div");
    document.body.appendChild(target);
    const refs: Ref[] = [
      { name: "feature", kind: "local", target_sha: "a", upstream: "origin/feature", ahead: 2, behind: 0 },
      { name: "origin/feature", kind: "remote", target_sha: "a", upstream: null, ahead: 0, behind: 0 },
    ];
    const panel = mount(WorktreePanel, {
      target,
      props: {
        worktrees: [worktree({ path: "/tmp/main", branch: "main", isMain: true }), worktree()],
        refs,
      },
    });
    await tick();

    const rows = target.querySelectorAll(".worktree");
    expect(rows).toHaveLength(1);
    const event = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: 30,
      clientY: 40,
    });
    rows[0].dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(contextMenu.open).toBe(true);
    expect(contextMenu.items.map((item) => item.label)).toEqual(expect.arrayContaining([
      "Open Worktree in Git It",
      "View Local Changes",
      "Open Worktree Folder",
      "Remove Worktree…",
    ]));
    expect(contextMenu.items.some((item) => item.label === "↑2 to push")).toBe(true);

    await unmount(panel);
  });
});
