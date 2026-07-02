import { describe, it, expect } from "vitest";
import { buildPrTimeline } from "./prTimeline";
import type { GhPullDetail } from "../types";

function base(): GhPullDetail {
  return {
    number: 1, title: "t", body: "", author: "a", state: "OPEN", isDraft: false,
    labels: [], assignees: [], milestone: null, baseRefName: "main", headRefName: "f",
    reviewDecision: "", mergeable: "", mergeStateStatus: "",
    additions: 0, deletions: 0, changedFiles: 0,
    files: [], reviews: [], checks: [], comments: [],
    commits: [], reviewThreads: [], checkRuns: [], bodyReactions: [],
    createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z", url: "",
  };
}

describe("buildPrTimeline", () => {
  it("orders events ascending by timestamp across kinds", () => {
    const d = base();
    d.commits = [{ oid: "aaaaaaa", message: "c", author: "a", committedDate: "2026-01-02T00:00:00Z" }];
    d.comments = [{ author: "b", body: "hi", createdAt: "2026-01-04T00:00:00Z", id: null, reactions: [] }];
    d.reviews = [{ author: "c", state: "APPROVED", body: "", submittedAt: "2026-01-03T00:00:00Z" }];
    d.checkRuns = [{ name: "ci", status: "completed", conclusion: "success", startedAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:05:00Z", url: "", headSha: "aaaaaaa" }];
    const ev = buildPrTimeline(d);
    expect(ev.map((e) => e.kind)).toEqual(["ciRun", "commit", "review", "comment"]);
  });

  it("attaches review threads to the nearest review and marks resolved", () => {
    const d = base();
    d.reviews = [{ author: "c", state: "CHANGES_REQUESTED", body: "", submittedAt: "2026-01-03T00:00:00Z" }];
    d.reviewThreads = [
      { id: "PRRT_1", resolved: false, path: "a.rs", line: 5, comments: [{ author: "c", body: "fix", path: "a.rs", line: 5, createdAt: "2026-01-03T00:00:01Z", databaseId: 11, reactions: [] }] },
    ];
    const ev = buildPrTimeline(d);
    const review = ev.find((e) => e.kind === "review");
    expect(review && review.kind === "review" && review.threads.length).toBe(1);
  });

  it("surfaces inline threads even when there are no top-level reviews", () => {
    const d = base();
    d.reviewThreads = [
      { id: "", resolved: false, path: "a.rs", line: 5, comments: [{ author: "c", body: "fix", path: "a.rs", line: 5, createdAt: "2026-01-02T00:00:00Z", databaseId: null, reactions: [] }] },
    ];
    const ev = buildPrTimeline(d);
    const t = ev.find((e) => e.kind === "reviewThread");
    expect(t && t.kind === "reviewThread" && t.thread.comments.length).toBe(1);
  });
});
