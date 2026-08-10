import { describe, it, expect } from "vitest";
import { blocksOf, ordinalsOf, blockAt, ordsInRange } from "./blocks";
import type { DiffHunk, DiffLineKind } from "./types";

/**
 * Build a hunk from a compact spec string, one char per line:
 * " " context, "+" add, "-" del.  e.g. "  -+  -++ "
 */
const hunk = (spec: string): DiffHunk => ({
  header: "@@ -1,1 +1,1 @@",
  lines: [...spec].map((ch, i) => ({
    kind: (ch === "+" ? "add" : ch === "-" ? "del" : "context") as DiffLineKind,
    text: `line${i}`,
    oldNo: ch === "+" ? null : i,
    newNo: ch === "-" ? null : i,
  })),
});

// Two blocks: rows 2-3 (ords 0,1) and rows 6-8 (ords 2,3,4).
const TWO_BLOCKS = hunk("  -+  -++ ");

describe("ordinalsOf", () => {
  it("numbers only changed lines, in order, leaving context null", () => {
    expect(ordinalsOf(hunk("  -+ "))).toEqual([null, null, 0, 1, null]);
  });

  it("returns all-null for a hunk of pure context", () => {
    expect(ordinalsOf(hunk("   "))).toEqual([null, null, null]);
  });
});

describe("blocksOf", () => {
  it("groups an adjacent del/add pair into ONE block", () => {
    expect(blocksOf(hunk(" -+ "))).toEqual([{ rows: [1, 2], ords: [0, 1] }]);
  });

  it("splits blocks on context lines and keeps ordinals continuous across them", () => {
    expect(blocksOf(TWO_BLOCKS)).toEqual([
      { rows: [2, 3], ords: [0, 1] },
      { rows: [6, 7, 8], ords: [2, 3, 4] },
    ]);
  });

  it("handles blocks touching the first and last row of the hunk", () => {
    expect(blocksOf(hunk("+  +"))).toEqual([
      { rows: [0], ords: [0] },
      { rows: [3], ords: [1] },
    ]);
  });

  it("returns no blocks for a hunk of pure context", () => {
    expect(blocksOf(hunk("   "))).toEqual([]);
  });
});

describe("blockAt", () => {
  it("returns the index of the block containing the row", () => {
    expect(blockAt(TWO_BLOCKS, 3)).toBe(0);
    expect(blockAt(TWO_BLOCKS, 7)).toBe(1);
  });

  it("returns null for a context row", () => {
    expect(blockAt(TWO_BLOCKS, 5)).toBeNull();
  });
});

describe("ordsInRange", () => {
  it("collects only changed lines when the range spans context rows", () => {
    // rows 2..8 covers both blocks AND the context rows 4,5 between them
    expect(ordsInRange(TWO_BLOCKS, 2, 8)).toEqual([0, 1, 2, 3, 4]);
  });

  it("handles a single-row range", () => {
    expect(ordsInRange(TWO_BLOCKS, 2, 2)).toEqual([0]);
  });

  it("normalizes a reversed range", () => {
    expect(ordsInRange(TWO_BLOCKS, 8, 2)).toEqual([0, 1, 2, 3, 4]);
  });

  it("returns empty when the range covers only context", () => {
    expect(ordsInRange(TWO_BLOCKS, 4, 5)).toEqual([]);
  });

  it("covers every ordinal when given the whole hunk", () => {
    expect(ordsInRange(TWO_BLOCKS, 0, 9)).toEqual([0, 1, 2, 3, 4]);
  });
});
