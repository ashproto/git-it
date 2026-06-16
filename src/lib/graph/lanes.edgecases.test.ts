import { describe, it, expect } from "vitest";
import { computeLanes } from "./lanes";
import type { LaneCommit, RowLayout } from "./types";

const c = (sha: string, parents: string[] = []): LaneCommit => ({ sha, parents });

// Every lane and edge index must fit within the reported width, and width must be
// at least 1 for any real row. Holds for ANY input — catches off-by-one / overlap.
function assertWidthInvariants(rows: RowLayout[]) {
  for (const r of rows) {
    expect(r.width).toBeGreaterThanOrEqual(1);
    expect(r.lane).toBeLessThan(r.width);
    for (const e of r.edges) {
      expect(e.fromLane).toBeGreaterThanOrEqual(0);
      expect(e.toLane).toBeGreaterThanOrEqual(0);
      expect(e.fromLane).toBeLessThan(r.width);
      expect(e.toLane).toBeLessThan(r.width);
      if (e.kind === "straight") expect(e.fromLane).toBe(e.toLane);
    }
  }
}

describe("computeLanes — degenerate input", () => {
  it("returns no rows for empty input", () => {
    expect(computeLanes([])).toEqual([]);
  });

  it("handles a single root commit", () => {
    expect(computeLanes([c("A", [])])).toEqual([
      { sha: "A", lane: 0, colorIndex: 0, isMerge: false, edges: [], width: 1 },
    ]);
  });
});

describe("computeLanes — determinism", () => {
  it("produces identical output across runs (pure function)", () => {
    const input = [c("M", ["A", "B"]), c("A", ["C"]), c("B", ["C"]), c("C", [])];
    expect(computeLanes(input)).toEqual(computeLanes(input));
  });
});

describe("computeLanes — duplicate parents", () => {
  it("draws a single branch edge when a parent is listed twice", () => {
    const rows = computeLanes([c("M", ["A", "Z", "Z"]), c("A", ["Z"]), c("Z", [])]);
    expect(rows[0].isMerge).toBe(true);
    expect(rows[0].edges.filter((e) => e.kind === "branch")).toHaveLength(1);
    assertWidthInvariants(rows);
  });
});

describe("computeLanes — fresh tip whose first parent is already reserved", () => {
  it("converges the new tip into the existing lane at the shared parent", () => {
    const rows = computeLanes([c("T1", ["G"]), c("T2", ["G"]), c("G", [])]);
    expect(rows.map((r) => r.lane)).toEqual([0, 1, 0]);
    expect(rows.map((r) => r.width)).toEqual([1, 2, 1]);
    expect(rows[1].edges).toContainEqual({ fromLane: 1, toLane: 0, colorIndex: 1, kind: "merge" });
    assertWidthInvariants(rows);
  });
});

describe("computeLanes — criss-cross merges (documented behavior)", () => {
  // Two independent merge commits sharing the same two parents in opposite order.
  // Each shared parent legitimately receives one converging line per child (the
  // same mechanism as a normal merge diamond), so the independent tip M2 occupies
  // its own lane and the M2/P rows span 3 columns. This is intentional and matches
  // Fork's "one incoming line per child" rendering — locked here so a future change
  // that silently collapses or duplicates these lanes is caught.
  it("keeps each child->parent line in its own lane and stays consistent", () => {
    const rows = computeLanes([
      c("M1", ["P", "Q"]),
      c("M2", ["Q", "P"]),
      c("P", ["base"]),
      c("Q", ["base"]),
      c("base", []),
    ]);
    expect(rows.map((r) => r.lane)).toEqual([0, 2, 0, 1, 0]);
    expect(rows.map((r) => r.width)).toEqual([2, 3, 3, 2, 1]);
    expect(rows.map((r) => r.isMerge)).toEqual([true, true, false, false, false]);
    assertWidthInvariants(rows);
  });
});

describe("computeLanes — reused-fork lane keeps its through-line (connectivity)", () => {
  // A and B both have parent C. B is a merge whose SECOND parent is C, so when B is
  // processed, C's lane (already reserved by A above) is reused as the fork target.
  // That lane must STILL draw its pass-through at B's row — otherwise A's line down
  // to C visibly stops at the merge row ("a line ends out of nowhere"). Regression
  // test for that exact bug.
  it("draws both the branch edge and the lane's pass-through at the merge row", () => {
    const rows = computeLanes([c("A", ["C"]), c("B", ["D", "C"]), c("D", ["C"]), c("C", [])]);
    const B = rows[1];
    expect(B.isMerge).toBe(true);
    // the fork branch from B's lane (1) into C's reused lane (0):
    expect(B.edges).toContainEqual({ fromLane: 1, toLane: 0, colorIndex: 0, kind: "branch" });
    // AND C's lane must continue straight through B's row (the bug dropped this):
    expect(B.edges).toContainEqual({ fromLane: 0, toLane: 0, colorIndex: 0, kind: "straight" });
    assertWidthInvariants(rows);
  });
});

describe("computeLanes — width invariants on the core fixtures", () => {
  it("holds for linear, branch, merge, and octopus", () => {
    assertWidthInvariants(computeLanes([c("A", ["B"]), c("B", ["C"]), c("C", [])]));
    assertWidthInvariants(computeLanes([c("X", ["Z"]), c("Y", ["Z"]), c("Z", [])]));
    assertWidthInvariants(
      computeLanes([c("M", ["A", "B"]), c("A", ["C"]), c("B", ["C"]), c("C", [])]),
    );
    assertWidthInvariants(
      computeLanes([
        c("O", ["A", "B", "C"]),
        c("A", ["D"]),
        c("B", ["D"]),
        c("C", ["D"]),
        c("D", []),
      ]),
    );
  });
});
