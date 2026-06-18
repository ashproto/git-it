import { describe, it, expect } from "vitest";
import { pullStateBadge, issueStateBadge } from "./itemState";

describe("pullStateBadge", () => {
  it("open PR → green Open", () => {
    expect(pullStateBadge({ state: "OPEN", isDraft: false })).toMatchObject({
      key: "open",
      label: "Open",
      color: "#1f883d",
    });
  });
  it("draft (open) PR → gray Draft", () => {
    expect(pullStateBadge({ state: "OPEN", isDraft: true })).toMatchObject({
      key: "draft",
      label: "Draft",
      color: "#6e7781",
    });
  });
  it("merged PR → purple Merged", () => {
    expect(pullStateBadge({ state: "MERGED", isDraft: false })).toMatchObject({
      key: "merged",
      label: "Merged",
      color: "#8957e5",
    });
  });
  it("closed (unmerged) PR → red Closed", () => {
    expect(pullStateBadge({ state: "CLOSED", isDraft: false })).toMatchObject({
      key: "closed",
      label: "Closed",
      color: "#cf222e",
    });
  });
});

describe("issueStateBadge", () => {
  it("open issue → green Open", () => {
    expect(issueStateBadge({ state: "OPEN", stateReason: null })).toMatchObject({
      key: "open",
      color: "#1f883d",
    });
  });
  it("closed-completed issue → purple Closed", () => {
    expect(issueStateBadge({ state: "CLOSED", stateReason: "COMPLETED" })).toMatchObject({
      key: "closed",
      label: "Closed",
      color: "#8957e5",
    });
  });
  it("closed-not-planned issue → gray Closed", () => {
    expect(issueStateBadge({ state: "CLOSED", stateReason: "NOT_PLANNED" })).toMatchObject({
      key: "not-planned",
      label: "Closed",
      color: "#6e7781",
    });
  });
  it("closed issue with null reason defaults to completed (purple)", () => {
    expect(issueStateBadge({ state: "CLOSED", stateReason: null }).color).toBe("#8957e5");
  });
});
