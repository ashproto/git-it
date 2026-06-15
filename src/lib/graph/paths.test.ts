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
  it("draws a bezier for a lane change", () => {
    expect(curvedEdgePath(edge(0, 1, "branch"), 0, g)).toBe("M12 0 C12 15 28 15 28 30");
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
