// View-model transition tests for the Local Changes ↔ Commit Timeline screens.
//
// `appState` is the singleton exported from the `.svelte.ts` runes store; the
// view-model here is the `activeView` ("timeline" | "changes") $state plus the
// helpers that flip it (setActiveView / setWorkingCopySelected / setCurrent) and
// the back-compat `workingCopySelected` getter. The store has no $effect, and all
// browser globals it touches are typeof-guarded, so it imports and runs as plain
// logic under Vitest (the svelte plugin in vitest.config.ts compiles the runes).
//
// Focus of this suite: the G1 regression fix — entering Local Changes must clear
// BOTH the focused commit (currentSha) and the multi-select Set, so a previously
// selected commit doesn't stay highlighted behind the working-copy view.
import { beforeEach, describe, expect, it } from "vitest";
import { appState } from "./store.svelte";
import { SAMPLE_GRAPH } from "./graph/sample";

// The store is a process-wide singleton, so reset the view-model to a known
// baseline before each test. Setting repo to "" clears any per-repo state a prior
// test left behind (the repo-switch test below opens one); the explicit resets
// then guarantee the baseline even on the first run, where repo is already "" and
// the setter no-ops. setActiveView("timeline") does NOT clear currentSha/selected
// (only "changes" does), so clear those explicitly.
beforeEach(() => {
  appState.repo = "";
  appState.setActiveView("timeline");
  appState.setCurrent(null);
  appState.selected = new Set();
});

describe("Local Changes view-model", () => {
  it("setWorkingCopySelected(true) enters changes and clears focus + selection (G1)", () => {
    // Arrange a focused commit and a non-empty multi-select to prove they're cleared.
    appState.setCurrent("deadbeefcafe");
    appState.selected = new Set(["deadbeefcafe", "0123456789ab"]);
    expect(appState.currentSha).toBe("deadbeefcafe");
    expect(appState.selected.size).toBe(2);

    appState.setWorkingCopySelected(true);

    expect(appState.activeView).toBe("changes");
    expect(appState.currentSha).toBeNull();
    expect(appState.selected.size).toBe(0);
  });

  it("setWorkingCopySelected(false) returns to the timeline", () => {
    appState.setWorkingCopySelected(true);
    expect(appState.activeView).toBe("changes");

    appState.setWorkingCopySelected(false);

    expect(appState.activeView).toBe("timeline");
  });

  it("setActiveView('changes') clears focus + selection just like the working-copy toggle", () => {
    appState.setCurrent("abc123");
    appState.selected = new Set(["abc123"]);

    appState.setActiveView("changes");

    expect(appState.activeView).toBe("changes");
    expect(appState.currentSha).toBeNull();
    expect(appState.selected.size).toBe(0);
  });

  it("setActiveView('timeline') leaves the selection untouched", () => {
    appState.selected = new Set(["sha-a", "sha-b"]);

    appState.setActiveView("timeline");

    expect(appState.activeView).toBe("timeline");
    expect(appState.selected.size).toBe(2);
    expect(appState.selected.has("sha-a")).toBe(true);
    expect(appState.selected.has("sha-b")).toBe(true);
  });

  it("setCurrent(sha) navigates back to the timeline and focuses that commit", () => {
    appState.setActiveView("changes");
    expect(appState.activeView).toBe("changes");

    appState.setCurrent("feedface");

    expect(appState.currentSha).toBe("feedface");
    expect(appState.activeView).toBe("timeline");
  });

  it("setCurrent(null) clears the focus but does NOT change the active view", () => {
    appState.setActiveView("changes");

    appState.setCurrent(null);

    expect(appState.currentSha).toBeNull();
    // Staying in Local Changes: a null focus must not yank the user to the graph.
    expect(appState.activeView).toBe("changes");
  });

  it("workingCopySelected getter is the back-compat shim for activeView === 'changes'", () => {
    appState.setActiveView("changes");
    expect(appState.workingCopySelected).toBe(true);

    appState.setActiveView("timeline");
    expect(appState.workingCopySelected).toBe(false);

    // And the working-copy setter round-trips through the same shim.
    appState.setWorkingCopySelected(true);
    expect(appState.workingCopySelected).toBe(true);
  });

  it("applyGraphRefresh (live refresh) preserves focus/selection/queued-edits for surviving commits", () => {
    appState.setGraphCommits(SAMPLE_GRAPH);
    const s0 = SAMPLE_GRAPH[0].sha;
    const s1 = SAMPLE_GRAPH[1].sha;
    appState.setCurrent(s0);
    appState.selected = new Set([s0, s1]);
    appState.setNewDate(s0, new Date("2020-01-01T00:00:00Z"));
    appState.setNewDate("goneSha000000", new Date("2020-01-02T00:00:00Z"));

    // A background/focus refresh whose history no longer contains s1.
    appState.applyGraphRefresh(SAMPLE_GRAPH.filter((c) => c.sha !== s1));

    expect(appState.currentSha).toBe(s0); // open commit still exists → preserved
    expect(appState.selected.has(s0)).toBe(true);
    expect(appState.selected.has(s1)).toBe(false); // pruned (no longer present)
    expect(appState.newDates.has(s0)).toBe(true); // queued edit kept
    expect(appState.newDates.has("goneSha000000")).toBe(false); // pruned
  });

  it("applyGraphRefresh clears the focus only when the open commit is gone", () => {
    appState.setGraphCommits(SAMPLE_GRAPH);
    appState.setCurrent(SAMPLE_GRAPH[0].sha);
    appState.applyGraphRefresh(SAMPLE_GRAPH.filter((c) => c.sha !== SAMPLE_GRAPH[0].sha));
    expect(appState.currentSha).toBeNull();
  });

  it("contrast: setGraphCommits (the repo-switch reset) still clears focus/selection/edits", () => {
    appState.setGraphCommits(SAMPLE_GRAPH);
    appState.setCurrent(SAMPLE_GRAPH[0].sha);
    appState.selected = new Set([SAMPLE_GRAPH[0].sha]);
    appState.setNewDate(SAMPLE_GRAPH[0].sha, new Date("2020-01-01T00:00:00Z"));

    appState.setGraphCommits(SAMPLE_GRAPH);

    expect(appState.currentSha).toBeNull();
    expect(appState.selected.size).toBe(0);
    expect(appState.newDates.size).toBe(0);
  });

  it("switching repositories resets the active view back to the timeline", () => {
    // Enter Local Changes with a live selection so the repo switch has real
    // per-repo state to reset. (currentSha can't be non-null here — entering
    // "changes" clears it — so its reset on switch is covered by the setCurrent
    // tests above, not asserted here where it would be vacuously already-null.)
    appState.setActiveView("changes");
    appState.selected = new Set(["abc123"]);
    expect(appState.activeView).toBe("changes");

    // The reset only runs when the path actually changes — pick a value distinct
    // from the current repo so the `v !== repo` guard fires regardless of test order.
    const target = appState.repo === "/tmp/gitit-test-a" ? "/tmp/gitit-test-b" : "/tmp/gitit-test-a";
    appState.repo = target;

    expect(appState.activeView).toBe("timeline");
    // The repo setter also clears the per-repo multi-select.
    expect(appState.selected.size).toBe(0);
  });
});
