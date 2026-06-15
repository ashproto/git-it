import { describe, it, expect } from "vitest";
import { computeLanes } from "./lanes";
import type { LaneCommit } from "./types";

const c = (sha: string, parents: string[] = []): LaneCommit => ({ sha, parents });

describe("computeLanes — linear history", () => {
  it("places a straight chain in lane 0", () => {
    const rows = computeLanes([c("A", ["B"]), c("B", ["C"]), c("C", [])]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0, 0]);
    expect(rows.map((r) => r.colorIndex)).toEqual([0, 0, 0]);
    expect(rows.map((r) => r.isMerge)).toEqual([false, false, false]);
    expect(rows[0].edges).toEqual([
      { fromLane: 0, toLane: 0, colorIndex: 0, kind: "straight" },
    ]);
    expect(rows[2].edges).toEqual([]);
  });
});

describe("computeLanes — branch point", () => {
  it("converges a second tip into the shared parent's lane", () => {
    const rows = computeLanes([c("X", ["Z"]), c("Y", ["Z"]), c("Z", [])]);
    expect(rows.map((r) => r.lane)).toEqual([0, 1, 0]);
    expect(rows[1].edges).toContainEqual({
      fromLane: 1,
      toLane: 0,
      colorIndex: 1,
      kind: "merge",
    });
    expect(rows[1].edges).toContainEqual({
      fromLane: 0,
      toLane: 0,
      colorIndex: 0,
      kind: "straight",
    });
  });
});

describe("computeLanes — merge commit", () => {
  it("forks at the merge and reconverges at the base", () => {
    const rows = computeLanes([
      c("M", ["A", "B"]),
      c("A", ["C"]),
      c("B", ["C"]),
      c("C", []),
    ]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0, 1, 0]);
    expect(rows[0].isMerge).toBe(true);
    expect(rows[0].colorIndex).toBe(0);
    expect(rows[1].colorIndex).toBe(0);
    expect(rows[2].colorIndex).toBe(1);
    expect(rows[0].edges).toContainEqual({
      fromLane: 0,
      toLane: 1,
      colorIndex: 1,
      kind: "branch",
    });
    expect(rows[0].edges).toContainEqual({
      fromLane: 0,
      toLane: 0,
      colorIndex: 0,
      kind: "straight",
    });
    expect(rows[2].edges).toContainEqual({
      fromLane: 1,
      toLane: 0,
      colorIndex: 1,
      kind: "merge",
    });
  });
});

describe("computeLanes — octopus merge", () => {
  it("forks to three lanes and reconverges", () => {
    const rows = computeLanes([
      c("O", ["A", "B", "C"]),
      c("A", ["D"]),
      c("B", ["D"]),
      c("C", ["D"]),
      c("D", []),
    ]);
    expect(rows[0].isMerge).toBe(true);
    const branchEdges = rows[0].edges.filter((e) => e.kind === "branch");
    expect(branchEdges).toHaveLength(2);
    const mergeEdges = rows[3].edges.filter((e) => e.kind === "merge");
    expect(mergeEdges).toHaveLength(2);
  });
});

describe("computeLanes — multiple roots", () => {
  it("recycles a freed lane and assigns a fresh color", () => {
    const rows = computeLanes([c("P", []), c("Q", [])]);
    expect(rows.map((r) => r.lane)).toEqual([0, 0]);
    expect(rows[0].colorIndex).toBe(0);
    expect(rows[1].colorIndex).toBe(1);
    expect(rows[0].edges).toEqual([]);
    expect(rows[1].edges).toEqual([]);
  });
});

describe("computeLanes — width", () => {
  it("reports the column count spanning each row's band", () => {
    const rows = computeLanes([
      c("M", ["A", "B"]),
      c("A", ["C"]),
      c("B", ["C"]),
      c("C", []),
    ]);
    expect(rows[0].width).toBe(2);
    expect(rows[3].width).toBe(1);
  });
});
