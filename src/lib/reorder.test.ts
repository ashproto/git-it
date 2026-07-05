import { describe, it, expect } from "vitest";
import { reorder, insertionIndex } from "./reorder";

describe("reorder", () => {
  it("moves an item forward", () => expect(reorder(["a","b","c","d"], 0, 2)).toEqual(["b","c","a","d"]));
  it("moves an item backward", () => expect(reorder(["a","b","c","d"], 3, 1)).toEqual(["a","d","b","c"]));
  it("same index is identity", () => expect(reorder(["a","b"], 1, 1)).toEqual(["a","b"]));
  it("out-of-range is identity", () => {
    expect(reorder(["a","b"], -1, 0)).toEqual(["a","b"]);
    expect(reorder(["a","b"], 0, 5)).toEqual(["a","b"]);
  });
  it("does not mutate the input", () => {
    const src = ["a","b","c"]; reorder(src, 0, 2); expect(src).toEqual(["a","b","c"]);
  });
});

describe("insertionIndex", () => {
  const mids = [20, 60, 100]; // three tabs, midpoints at x = 20, 60, 100
  it("returns 0 left of the first midpoint", () => {
    expect(insertionIndex(mids, 5)).toBe(0);
  });
  it("returns the index of the first midpoint to the right of the pointer", () => {
    expect(insertionIndex(mids, 40)).toBe(1);
    expect(insertionIndex(mids, 80)).toBe(2);
  });
  it("clamps to the last index past the final midpoint", () => {
    expect(insertionIndex(mids, 999)).toBe(2);
  });
  it("returns 0 for an empty list", () => {
    expect(insertionIndex([], 10)).toBe(0);
  });
});
