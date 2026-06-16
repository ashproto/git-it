import { describe, it, expect } from "vitest";
import { commitWindow } from "./window";

const RH = 30;

describe("commitWindow", () => {
  it("returns an empty window when there are no rows", () => {
    expect(commitWindow(0, 300, RH, 0, 8)).toEqual({ start: 0, end: 0 });
  });

  it("guards against a zero/negative rowHeight (never divides by zero)", () => {
    expect(commitWindow(0, 300, 0, 100, 8)).toEqual({ start: 0, end: 0 });
    expect(commitWindow(0, 300, -5, 100, 8)).toEqual({ start: 0, end: 0 });
  });

  it("renders the whole list when it is shorter than the viewport (+buffer)", () => {
    // 7-commit sample at the top: scrolledPx≈-1 (sticky header), viewport≈246.
    expect(commitWindow(-1, 246, RH, 7, 8)).toEqual({ start: 0, end: 7 });
  });

  it("windows a large list at the top with symmetric overscan below", () => {
    // viewport 300 / 30 = 10 visible rows; +8 buffer below, clamped to 0 above.
    expect(commitWindow(0, 300, RH, 5000, 8)).toEqual({ start: 0, end: 18 });
  });

  it("windows a large list mid-scroll with the buffer on both sides", () => {
    // scrolled 100 rows down (3000px); 10 visible; buffer 8 each side.
    expect(commitWindow(3000, 300, RH, 5000, 8)).toEqual({ start: 92, end: 118 });
  });

  it("clamps the end to total near the bottom", () => {
    // Last row top = 4999*30 = 149970; scrolled near the very bottom.
    expect(commitWindow(149800, 300, RH, 5000, 8)).toEqual({ start: 4985, end: 5000 });
  });

  it("grows the window as the buffer grows", () => {
    expect(commitWindow(3000, 300, RH, 5000, 0)).toEqual({ start: 100, end: 110 });
    expect(commitWindow(3000, 300, RH, 5000, 4)).toEqual({ start: 96, end: 114 });
  });

  it("treats a negative viewport as zero height (still renders the buffer)", () => {
    expect(commitWindow(0, -50, RH, 100, 8)).toEqual({ start: 0, end: 8 });
  });

  it("returns an empty window when scrolled past the end", () => {
    expect(commitWindow(999999, 300, RH, 100, 8)).toEqual({ start: 100, end: 100 });
  });
});
