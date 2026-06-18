import { describe, it, expect } from "vitest";
import { sparklinePoints } from "./activity";

describe("sparklinePoints", () => {
  it("maps values to a polyline string, max at the top (y=0)", () => {
    expect(sparklinePoints([0, 5, 10], 100, 20)).toBe("0.0,20.0 50.0,10.0 100.0,0.0");
  });
  it("returns empty for no values", () => {
    expect(sparklinePoints([], 100, 20)).toBe("");
  });
  it("flat series sits on the baseline", () => {
    expect(sparklinePoints([3, 3], 10, 10)).toBe("0.0,0.0 10.0,0.0");
  });
});
