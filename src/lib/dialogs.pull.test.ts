import { describe, expect, it } from "vitest";
import { dialogs } from "./dialogs.svelte";

function openPull(upstream: string | null) {
  return dialogs.confirmPull({
    branch: "main",
    upstream,
    upstreamGone: false,
    ahead: 2,
    behind: 3,
    strategy: "merge",
    autostash: false,
    offerDontAsk: true,
  });
}

describe("pull options dialog", () => {
  it("opens preselected from the saved defaults and resolves the edited choice", async () => {
    const pending = openPull("origin/main");

    expect(dialogs.state.kind).toBe("pull");
    if (dialogs.state.kind !== "pull") throw new Error("pull dialog did not open");
    expect(dialogs.state.branch).toBe("main");
    expect(dialogs.state.upstream).toBe("origin/main");
    expect(dialogs.state.ahead).toBe(2);
    expect(dialogs.state.behind).toBe(3);
    expect(dialogs.state.strategy).toBe("merge");
    expect(dialogs.state.autostash).toBe(false);
    expect(dialogs.state.offerDontAsk).toBe(true);
    expect(dialogs.state.dontAsk).toBe(false);

    dialogs.setPullStrategy("rebase");
    dialogs.setPullAutostash(true);
    dialogs.setPullDontAsk(true);
    dialogs.resolvePull(true);

    await expect(pending).resolves.toEqual({ strategy: "rebase", autostash: true, dontAsk: true });
    expect(dialogs.state.kind).toBe("none");
  });

  it("resolves null on cancel", async () => {
    const pending = openPull("origin/main");
    dialogs.setPullStrategy("ffOnly");
    dialogs.resolvePull(false);
    await expect(pending).resolves.toBeNull();
    expect(dialogs.state.kind).toBe("none");
  });

  it("refuses to pull when the branch has no upstream", async () => {
    const pending = openPull(null);
    dialogs.resolvePull(true);
    expect(dialogs.state.kind).toBe("pull"); // still open — nothing to pull from
    dialogs.resolvePull(false);
    await expect(pending).resolves.toBeNull();
  });

  it("refuses to pull when the upstream is gone", async () => {
    const pending = dialogs.confirmPull({
      branch: "main",
      upstream: "origin/main",
      upstreamGone: true,
      ahead: 0,
      behind: 0,
      strategy: "merge",
      autostash: false,
      offerDontAsk: true,
    });
    dialogs.resolvePull(true);
    expect(dialogs.state.kind).toBe("pull"); // still open — the upstream no longer exists
    dialogs.resolvePull(false);
    await expect(pending).resolves.toBeNull();
  });

  it("refuses to pull with a detached HEAD", async () => {
    const pending = dialogs.confirmPull({
      branch: null,
      upstream: null,
      upstreamGone: false,
      ahead: 0,
      behind: 0,
      strategy: "merge",
      autostash: false,
      offerDontAsk: true,
    });
    dialogs.resolvePull(true);
    expect(dialogs.state.kind).toBe("pull");
    dialogs.resolvePull(false);
    await expect(pending).resolves.toBeNull();
  });

  it("is settled as cancelled when another dialog replaces it", async () => {
    const pending = openPull("origin/main");
    const other = dialogs.confirm({ title: "Other", message: "Replaces the pull dialog" });
    await expect(pending).resolves.toBeNull();
    dialogs.resolveConfirm(false);
    await other;
  });
});
