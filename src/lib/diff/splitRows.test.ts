import { describe, it, expect } from "vitest";
import { toSplitRows, isChangeRow, splitBlocksOf, splitBlockAt, ordsInSplitRange } from "./splitRows";
import type { DiffHunk, DiffLineKind } from "./types";

/** Build a hunk from a compact spec: " " context, "+" add, "-" del. */
const hunk = (spec: string): DiffHunk => ({
  header: "@@ -1,1 +1,1 @@",
  lines: [...spec].map((ch, i) => ({
    kind: (ch === "+" ? "add" : ch === "-" ? "del" : "context") as DiffLineKind,
    text: `line${i}`,
    oldNo: ch === "+" ? null : i,
    newNo: ch === "-" ? null : i,
  })),
});

const rowsOf = (spec: string) => toSplitRows(hunk(spec)).map((r) => r.rows);

describe("toSplitRows", () => {
  it("gives a context row the single hunk line it shows", () => {
    expect(rowsOf("   ")).toEqual([[0], [1], [2]]);
  });

  it("pairs equal runs of deletions and additions onto shared rows", () => {
    // lines 1,2 are del; 3,4 are add → row0 = (1,3), row1 = (2,4)
    expect(rowsOf(" --++ ")).toEqual([[0], [1, 3], [2, 4], [5]]);
  });

  it("leaves the surplus deletion alone when there are more deletions than additions", () => {
    // lines 0,1 del; 2 add → row0 = (0,2), row1 = (1)
    expect(rowsOf("--+")).toEqual([[0, 2], [1]]);
  });

  it("leaves the surplus addition alone when there are more additions than deletions", () => {
    // line 0 del; 1,2 add → row0 = (0,1), row1 = (2)
    expect(rowsOf("-++")).toEqual([[0, 1], [2]]);
  });

  it("handles a pure-addition run with no deletions to pair against", () => {
    expect(rowsOf("++")).toEqual([[0], [1]]);
  });
});

describe("isChangeRow", () => {
  it("is true for a row carrying a deletion or an addition, false for context", () => {
    const rows = toSplitRows(hunk(" -+ "));
    expect(rows.map(isChangeRow)).toEqual([false, true, false]);
  });
});

describe("splitBlocksOf", () => {
  it("collects a paired run into ONE split block covering both sides", () => {
    const h = hunk(" --++ ");
    const b = splitBlocksOf(h, toSplitRows(h));
    // split rows 1 and 2 carry hunk lines 1,3 and 2,4 → ordinals 0,1,2,3
    expect(b).toEqual([{ rows: [1, 2], ords: [0, 1, 2, 3] }]);
  });

  it("returns ordinals in ascending order even though pairing interleaves them", () => {
    const h = hunk("--++");
    const b = splitBlocksOf(h, toSplitRows(h));
    // row0 = lines 0,2 (ords 0,2); row1 = lines 1,3 (ords 1,3) → must sort
    expect(b[0].ords).toEqual([0, 1, 2, 3]);
  });

  it("breaks blocks on context rows", () => {
    const h = hunk("-+ -+");
    const b = splitBlocksOf(h, toSplitRows(h));
    expect(b.map((x) => x.rows)).toEqual([[0], [2]]);
  });

  it("returns no blocks for a hunk of pure context", () => {
    const h = hunk("   ");
    expect(splitBlocksOf(h, toSplitRows(h))).toEqual([]);
  });
});

describe("splitBlockAt", () => {
  it("finds the block containing a split row, and null for context", () => {
    const rows = toSplitRows(hunk("-+ -+"));
    expect(splitBlockAt(rows, 0)).toBe(0);
    expect(splitBlockAt(rows, 1)).toBeNull();
    expect(splitBlockAt(rows, 2)).toBe(1);
  });
});

describe("ordsInSplitRange", () => {
  it("returns BOTH sides of a single paired row — the unit-selection guarantee", () => {
    const h = hunk(" --++ ");
    const rows = toSplitRows(h);
    // split row 1 shows hunk lines 1 (del, ord 0) and 3 (add, ord 2)
    expect(ordsInSplitRange(h, rows, 1, 1)).toEqual([0, 2]);
  });

  it("unions and sorts across a multi-row range", () => {
    const h = hunk(" --++ ");
    const rows = toSplitRows(h);
    expect(ordsInSplitRange(h, rows, 1, 2)).toEqual([0, 1, 2, 3]);
  });

  it("skips context rows inside the range", () => {
    const h = hunk("-+ -+");
    const rows = toSplitRows(h);
    expect(ordsInSplitRange(h, rows, 0, 2)).toEqual([0, 1, 2, 3]);
  });

  it("normalizes a reversed range", () => {
    const h = hunk("-+ -+");
    const rows = toSplitRows(h);
    expect(ordsInSplitRange(h, rows, 2, 0)).toEqual([0, 1, 2, 3]);
  });

  it("returns empty for a context-only range", () => {
    const h = hunk("-+ -+");
    const rows = toSplitRows(h);
    expect(ordsInSplitRange(h, rows, 1, 1)).toEqual([]);
  });
});
