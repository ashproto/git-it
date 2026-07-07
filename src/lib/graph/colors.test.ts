import { describe, it, expect } from "vitest";
import { LANE_PALETTE, NERV_LANE_PALETTE, laneColor } from "./colors";

describe("laneColor", () => {
  it("returns the palette color for a color index", () => {
    expect(laneColor(0, null, {})).toBe(LANE_PALETTE[0]);
    expect(laneColor(2, null, {})).toBe(LANE_PALETTE[2]);
  });

  it("cycles through the palette when the index exceeds its length", () => {
    expect(laneColor(LANE_PALETTE.length, null, {})).toBe(LANE_PALETTE[0]);
    expect(laneColor(LANE_PALETTE.length + 1, null, {})).toBe(LANE_PALETTE[1]);
  });

  it("prefers a per-branch override when one exists for the branch", () => {
    expect(laneColor(0, "main", { main: "#ff0000" })).toBe("#ff0000");
  });

  it("ignores overrides for other branches", () => {
    expect(laneColor(0, "feature", { main: "#ff0000" })).toBe(LANE_PALETTE[0]);
  });

  it("falls back to palette when branch name is null", () => {
    expect(laneColor(1, null, { main: "#ff0000" })).toBe(LANE_PALETTE[1]);
  });

  it("wraps a negative color index into range", () => {
    expect(laneColor(-1, null, {})).toBe(LANE_PALETTE[LANE_PALETTE.length - 1]);
    expect(laneColor(-LANE_PALETTE.length, null, {})).toBe(LANE_PALETTE[0]);
  });

  it("uses a provided palette when passed", () => {
    expect(laneColor(0, null, {}, NERV_LANE_PALETTE)).toBe(NERV_LANE_PALETTE[0]);
    expect(laneColor(NERV_LANE_PALETTE.length, null, {}, NERV_LANE_PALETTE)).toBe(NERV_LANE_PALETTE[0]);
  });
  it("defaults to the classic palette when no palette is passed", () => {
    expect(laneColor(3, null, {})).toBe(LANE_PALETTE[3]);
  });
});
