import { describe, expect, it } from "vitest";
import { prForBranch } from "./branchPr";
import type { GhPull } from "../types";

function pull(overrides: Partial<GhPull>): GhPull {
  return {
    number: 1,
    title: "A pull request",
    author: "alice",
    headRefName: "feature/x",
    baseRefName: "main",
    labels: [],
    reviewDecision: "",
    isDraft: false,
    state: "OPEN",
    updatedAt: "2026-07-01T00:00:00Z",
    url: "https://github.com/o/r/pull/1",
    ...overrides,
  };
}

describe("prForBranch", () => {
  it("matches an open PR by exact head branch name", () => {
    const pr = pull({ number: 7, headRefName: "feature/x" });
    expect(prForBranch([pull({ number: 3, headRefName: "other" }), pr], "feature/x")).toBe(pr);
  });

  it("returns null when no PR heads the branch", () => {
    expect(prForBranch([pull({ headRefName: "other" })], "feature/x")).toBeNull();
    // Exact match only — no prefix/suffix matching.
    expect(prForBranch([pull({ headRefName: "feature/x2" })], "feature/x")).toBeNull();
    expect(prForBranch([pull({ headRefName: "feature/x" })], "feature")).toBeNull();
  });

  it("ignores closed and merged PRs even when the head matches", () => {
    expect(prForBranch([pull({ state: "CLOSED" })], "feature/x")).toBeNull();
    expect(prForBranch([pull({ state: "MERGED" })], "feature/x")).toBeNull();
    // …but an open PR alongside them still matches.
    const open = pull({ number: 9 });
    expect(prForBranch([pull({ state: "CLOSED", number: 2 }), open], "feature/x")).toBe(open);
  });

  it("returns null for a null (not-yet-loaded) or empty list", () => {
    expect(prForBranch(null, "feature/x")).toBeNull();
    expect(prForBranch([], "feature/x")).toBeNull();
  });

  it("returns null for an empty branch name", () => {
    expect(prForBranch([pull({ headRefName: "" })], "")).toBeNull();
  });
});
