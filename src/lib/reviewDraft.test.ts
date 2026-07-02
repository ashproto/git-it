import { describe, it, expect, beforeEach } from "vitest";
import { reviewDraft } from "./reviewDraft.svelte";

beforeEach(() => reviewDraft.discard());

describe("reviewDraft", () => {
  it("starts empty and records the PR it belongs to on first add", () => {
    expect(reviewDraft.count).toBe(0);
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    expect(reviewDraft.count).toBe(1);
    expect(reviewDraft.belongsTo("/r", 5)).toBe(true);
    expect(reviewDraft.belongsTo("/r", 6)).toBe(false);
  });
  it("rejects adds for a different PR while a draft exists", () => {
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    expect(() => reviewDraft.addComment("/r", 6, { path: "b.ts", line: 1, side: "RIGHT", body: "x" })).toThrow();
  });
  it("edits and removes by index; discard clears everything", () => {
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    reviewDraft.updateComment(0, "better");
    expect(reviewDraft.comments[0].body).toBe("better");
    reviewDraft.removeComment(0);
    expect(reviewDraft.count).toBe(0);
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    reviewDraft.discard();
    expect(reviewDraft.count).toBe(0);
    expect(reviewDraft.belongsTo("/r", 5)).toBe(false);
  });
  it("finds the draft comment on a given line/side", () => {
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    expect(reviewDraft.commentAt("a.ts", 3, "RIGHT")?.body).toBe("hm");
    expect(reviewDraft.commentAt("a.ts", 4, "RIGHT")).toBeUndefined();
  });
  it("bound reflects the binding lifecycle (survives removing the last comment)", () => {
    expect(reviewDraft.bound).toBe(false);
    reviewDraft.addComment("/r", 5, { path: "a.ts", line: 3, side: "RIGHT", body: "hm" });
    expect(reviewDraft.bound).toBe(true);
    reviewDraft.removeComment(0);
    expect(reviewDraft.count).toBe(0);
    expect(reviewDraft.bound).toBe(true); // binding + verdict/summary survive an empty list
    reviewDraft.discard();
    expect(reviewDraft.bound).toBe(false);
  });
  it("tracks verdict and summary, cleared on discard", () => {
    reviewDraft.setVerdict("APPROVE");
    reviewDraft.setSummary("ship it");
    expect(reviewDraft.verdict).toBe("APPROVE");
    expect(reviewDraft.summary).toBe("ship it");
    reviewDraft.discard();
    expect(reviewDraft.verdict).toBe("COMMENT");
    expect(reviewDraft.summary).toBe("");
  });
});
