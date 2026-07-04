import { describe, it, expect } from "vitest";
import { laneX, curvedEdgePath, angularEdgePath } from "./paths";
import type { Edge } from "./types";

const g = { laneWidth: 16, rowHeight: 30, offsetX: 12 };
const edge = (fromLane: number, toLane: number, kind: Edge["kind"]): Edge => ({
  fromLane,
  toLane,
  colorIndex: 0,
  kind,
});

describe("laneX", () => {
  it("maps a lane index to an x coordinate", () => {
    expect(laneX(0, g)).toBe(12);
    expect(laneX(2, g)).toBe(44);
  });
});

describe("curvedEdgePath", () => {
  it("draws a vertical line for a straight (same-lane) edge", () => {
    expect(curvedEdgePath(edge(0, 0, "straight"), 0, g)).toBe("M12 0 L12 30");
  });
  it("branch (merge-in) defaults to hooked: horizontal off the node, vertical into the feature lane", () => {
    expect(curvedEdgePath(edge(0, 1, "branch"), 0, g)).toBe("M12 0 C28 0 28 24 28 30");
  });
  it("branch (merge-in) featureSide: vertical down the node lane, bend near the feature dot", () => {
    expect(
      curvedEdgePath(edge(0, 1, "branch"), 0, g, { tension: 0.8, mergeInStyle: "featureSide" }),
    ).toBe("M12 0 C12 6 12 30 28 30");
  });
  it("branch (merge-in) symmetric matches the old S", () => {
    expect(
      curvedEdgePath(edge(0, 1, "branch"), 0, g, { tension: 0.8, mergeInStyle: "symmetric" }),
    ).toBe("M12 0 C12 24 28 6 28 30");
  });
  it("merge (branch-off) stays symmetric regardless of mergeInStyle", () => {
    expect(curvedEdgePath(edge(1, 0, "merge"), 0, g)).toBe("M28 0 C28 24 12 6 12 30");
    expect(
      curvedEdgePath(edge(1, 0, "merge"), 0, g, { tension: 0.8, mergeInStyle: "hooked" }),
    ).toBe("M28 0 C28 24 12 6 12 30");
  });
  it("honours a lower tension (0.55)", () => {
    expect(curvedEdgePath(edge(0, 1, "branch"), 0, g, { tension: 0.55, mergeInStyle: "hooked" })).toBe(
      "M12 0 C28 0 28 16.5 28 30",
    );
    expect(curvedEdgePath(edge(1, 0, "merge"), 0, g, { tension: 0.55, mergeInStyle: "hooked" })).toBe(
      "M28 0 C28 16.5 12 13.5 12 30",
    );
  });
});

describe("angularEdgePath", () => {
  it("draws a vertical line for a straight edge", () => {
    expect(angularEdgePath(edge(0, 0, "straight"), 0, g)).toBe("M12 0 L12 30");
  });
  it("forks late for a branch edge (down then across)", () => {
    expect(angularEdgePath(edge(0, 1, "branch"), 0, g)).toBe("M12 0 L12 15 L28 30");
  });
  it("converges early for a merge edge (across then down)", () => {
    expect(angularEdgePath(edge(1, 0, "merge"), 0, g)).toBe("M28 0 L12 15 L12 30");
  });
});
