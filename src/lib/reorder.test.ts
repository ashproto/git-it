import { describe, it, expect } from "vitest";
import { reorder } from "./reorder";
import { gapToIndex } from "./reorder";

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

// gapToIndex maps a visual insertion gap (0..n, between tabs) to the `to`
// index reorder() expects ("lands at index `to` of the result").
describe("gapToIndex", () => {
  it("gap past the source shifts down by one", () => expect(gapToIndex(0, 3)).toBe(2));
  it("gap before the source is unchanged", () => expect(gapToIndex(2, 0)).toBe(0));
  it("the two gaps surrounding the source map to the source (identity)", () => {
    expect(gapToIndex(1, 1)).toBe(1);
    expect(gapToIndex(1, 2)).toBe(1);
  });
});
