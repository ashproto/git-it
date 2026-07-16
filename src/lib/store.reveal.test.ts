// Arming tests for the Task 9 one-shot timeline "graph builds itself" reveal.
//
// `appState.revealing` gates the row-cascade + edge-draw animations. The
// load-bearing invariant is that the reveal is armed EXACTLY ONCE per repo-identity
// change (or a same-repo view switch, exercised in +page.svelte) and must NOT replay
// on an fswatch/window-focus refresh, on infinite-scroll paging, or on a same-repo
// git-op reload. The arming logic lives in the runes store (setGraphCommits +
// armReveal), which imports and runs as plain logic under Vitest — the reveal uses a
// plain seq-guarded setTimeout (NO $effect), so fake timers drive it deterministically.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { appState } from "./store.svelte";
import { SAMPLE_GRAPH } from "./graph/sample";
import type { GraphCommit } from "./types";

// A synthetic older page (fresh SHAs) so appendGraphCommits takes its real append
// path rather than the dedupe early-return.
const EXTRA: GraphCommit[] = [
  {
    sha: "ffff0000aaaa",
    parents: [],
    author_name: "A. Shah",
    author_email: "a@x.dev",
    author_date: "2026-06-12T09:00:00Z",
    committer_name: "A. Shah",
    committer_date: "2026-06-12T09:00:00Z",
    refs: [],
    subject: "older page commit",
    body: "",
  },
];

// The store is a process-wide singleton; reset the view-model baseline. NOTE: the
// repo setter never touches graphCommitsRepo, so each test primes it explicitly via
// setGraphCommits with distinct repo paths — arming is independent of test order.
beforeEach(() => {
  appState.repo = "";
  appState.setActiveView("timeline");
  appState.setCurrent(null);
  appState.selected = new Set();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("Timeline reveal arming (Task 9 one-shot)", () => {
  it("arms on a repo-identity change (first load into a repo)", () => {
    vi.useFakeTimers();
    // Prime a baseline repo so graphCommitsRepo is known, then load a DIFFERENT repo.
    appState.repo = "/tmp/reveal-arm-base";
    appState.setGraphCommits(SAMPLE_GRAPH);
    vi.runOnlyPendingTimers();
    expect(appState.revealing).toBe(false);

    appState.repo = "/tmp/reveal-arm-next";
    appState.setGraphCommits(SAMPLE_GRAPH);
    expect(appState.revealing).toBe(true); // armed on the identity transition

    vi.advanceTimersByTime(1100);
    expect(appState.revealing).toBe(false); // one-shot: self-clears
  });

  it("does NOT replay on refresh / paging / same-repo reload (load-bearing invariant)", () => {
    vi.useFakeTimers();
    appState.repo = "/tmp/reveal-noreplay";
    appState.setGraphCommits(SAMPLE_GRAPH); // arm once
    vi.advanceTimersByTime(1100);
    expect(appState.revealing).toBe(false);

    // fswatch / window-focus live refresh must NOT re-arm.
    appState.applyGraphRefresh(SAMPLE_GRAPH);
    expect(appState.revealing).toBe(false);

    // infinite-scroll paging must NOT re-arm (fresh SHAs → real append path).
    appState.appendGraphCommits(EXTRA);
    expect(appState.revealing).toBe(false);

    // same-repo git-op reload (setGraphCommits with an unchanged repo identity).
    appState.setGraphCommits(SAMPLE_GRAPH);
    expect(appState.revealing).toBe(false);
  });

  it("re-arms on a genuine repo switch", () => {
    vi.useFakeTimers();
    appState.repo = "/tmp/reveal-switch-a";
    appState.setGraphCommits(SAMPLE_GRAPH);
    vi.advanceTimersByTime(1100);
    expect(appState.revealing).toBe(false);

    appState.repo = "/tmp/reveal-switch-b";
    appState.setGraphCommits(SAMPLE_GRAPH);
    expect(appState.revealing).toBe(true);
    vi.advanceTimersByTime(1100);
  });

  it("a re-arm mid-reveal supersedes the older timeout (seq guard, no early clear)", () => {
    vi.useFakeTimers();
    appState.repo = "/tmp/reveal-seq-a";
    appState.setGraphCommits(SAMPLE_GRAPH); // arm #1: clear scheduled at t=1100
    vi.advanceTimersByTime(600);
    appState.repo = "/tmp/reveal-seq-b";
    appState.setGraphCommits(SAMPLE_GRAPH); // arm #2: bumps seq, clear at t=1700
    vi.advanceTimersByTime(600); // t=1200: arm#1's stale timeout fires but seq mismatches → NO clear
    expect(appState.revealing).toBe(true);
    vi.advanceTimersByTime(600); // t=1800: arm#2's timeout fires → clears
    expect(appState.revealing).toBe(false);
  });

  it("armReveal() (the view-switch entry point) is exposed and self-clears", () => {
    vi.useFakeTimers();
    appState.armReveal();
    expect(appState.revealing).toBe(true);
    vi.advanceTimersByTime(1100);
    expect(appState.revealing).toBe(false);
  });
});
