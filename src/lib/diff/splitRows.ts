import type { DiffHunk, DiffLine } from "./types";
import { ordinalsOf, type Block } from "./blocks";

/** One visual row of the side-by-side view: a deletion and its paired addition, or a context line. */
export interface SplitRow {
  oldLine: DiffLine | null;
  newLine: DiffLine | null;
  oldBeforeIdx: number;
  oldAfterIdx: number;
  newBeforeIdx: number;
  newAfterIdx: number;
  /** Hunk line indices this row represents: 1 for context, 1-2 for a change pair. */
  rows: number[];
}

/** Pair up del/add lines into side-by-side rows, with context lines spanning both. */
export function toSplitRows(hunk: DiffHunk): SplitRow[] {
  const rows: SplitRow[] = [];
  let bi = 0;
  let ai = 0;

  let i = 0;
  const lines = hunk.lines;

  while (i < lines.length) {
    const line = lines[i];

    if (line.kind === "context") {
      rows.push({ oldLine: line, newLine: line, oldBeforeIdx: bi, oldAfterIdx: ai, newBeforeIdx: bi, newAfterIdx: ai, rows: [i] });
      bi++;
      ai++;
      i++;
      continue;
    }

    // Collect a contiguous run of del/add lines and pair them up.
    const dels: Array<{ line: DiffLine; bi: number; idx: number }> = [];
    const adds: Array<{ line: DiffLine; ai: number; idx: number }> = [];

    while (i < lines.length && (lines[i].kind === "del" || lines[i].kind === "add")) {
      if (lines[i].kind === "del") {
        dels.push({ line: lines[i], bi, idx: i });
        bi++;
      } else {
        adds.push({ line: lines[i], ai, idx: i });
        ai++;
      }
      i++;
    }

    const maxLen = Math.max(dels.length, adds.length);
    for (let j = 0; j < maxLen; j++) {
      const d = dels[j] ?? null;
      const a = adds[j] ?? null;
      rows.push({
        oldLine: d?.line ?? null,
        newLine: a?.line ?? null,
        oldBeforeIdx: d?.bi ?? 0,
        oldAfterIdx: 0,      // dels use before-tokens, index not used for after
        newBeforeIdx: 0,
        newAfterIdx: a?.ai ?? 0,
        rows: [d?.idx, a?.idx].filter((x): x is number => x !== undefined),
      });
    }
  }

  return rows;
}

/** True when this visual row carries any changed line on either side. */
export function isChangeRow(r: SplitRow): boolean {
  return r.oldLine?.kind === "del" || r.newLine?.kind === "add";
}

/**
 * Contiguous runs of change rows, in SPLIT-ROW index space.
 *
 * NOTE the deliberate asymmetry: the returned `Block.rows` holds **split-row**
 * indices (so it can be compared against the `{#each}` index that renders them),
 * while `Block.ords` holds ordinary hunk change ordinals (what the staging backend
 * takes). Pairing interleaves the two sides, so the ordinals are sorted before
 * being returned.
 */
export function splitBlocksOf(hunk: DiffHunk, rows: SplitRow[]): Block[] {
  const ordinals = ordinalsOf(hunk);
  const out: Block[] = [];
  let cur: Block | null = null;
  for (let ri = 0; ri < rows.length; ri++) {
    if (!isChangeRow(rows[ri])) {
      cur = null;
      continue;
    }
    if (cur === null) {
      cur = { rows: [], ords: [] };
      out.push(cur);
    }
    cur.rows.push(ri);
    for (const line of rows[ri].rows) {
      const o = ordinals[line];
      if (o !== null && o !== undefined) cur.ords.push(o);
    }
  }
  for (const b of out) b.ords.sort((x, y) => x - y);
  return out;
}

/** Index of the split block containing split-row `ri`, or null for a context row. */
export function splitBlockAt(rows: SplitRow[], ri: number): number | null {
  if (!rows[ri] || !isChangeRow(rows[ri])) return null;
  let block = -1;
  let open = false;
  for (let i = 0; i <= ri; i++) {
    if (isChangeRow(rows[i])) {
      if (!open) {
        block++;
        open = true;
      }
    } else {
      open = false;
    }
  }
  return block;
}

/**
 * Every change block's split-row range, in document order. The loop runs one past the
 * last row so a block ending on the final row is still flushed.
 */
export function splitBlockRanges(rows: SplitRow[]): Array<{ from: number; to: number }> {
  const out: Array<{ from: number; to: number }> = [];
  let open = false;
  let start = 0;
  for (let ri = 0; ri <= rows.length; ri++) {
    const isChange = ri < rows.length && isChangeRow(rows[ri]);
    if (isChange && !open) {
      open = true;
      start = ri;
    } else if (!isChange && open) {
      open = false;
      out.push({ from: start, to: ri - 1 });
    }
  }
  return out;
}

/**
 * The split-row range covering every block the inclusive range [a, b] touches, or null
 * when it touches none (a context-only range).
 *
 * Split-view selection snaps to whole blocks because a PARTIAL selection of a mixed
 * deletion/addition block yields non-contiguous ordinals — and `build_partial_hunk`
 * emits in hunk source order, so such a patch silently reorders the file. Whole blocks
 * are always contiguous in ordinal space, as is any union of them, because ordinals run
 * consecutively across the hunk.
 */
export function splitRangeSnappedToBlocks(
  rows: SplitRow[],
  a: number,
  b: number,
): { from: number; to: number } | null {
  const lo = Math.min(a, b);
  const hi = Math.max(a, b);
  const hit = splitBlockRanges(rows).filter((r) => r.to >= lo && r.from <= hi);
  if (!hit.length) return null;
  return {
    from: Math.min(...hit.map((r) => r.from)),
    to: Math.max(...hit.map((r) => r.to)),
  };
}

/**
 * Change-line ordinals covered by an inclusive SPLIT-ROW range (either order).
 * Both sides of every paired row in the range are included — this is what makes
 * "a paired row selects as a unit" true rather than aspirational.
 */
export function ordsInSplitRange(hunk: DiffHunk, rows: SplitRow[], from: number, to: number): number[] {
  const ordinals = ordinalsOf(hunk);
  const lo = Math.max(0, Math.min(from, to));
  const hi = Math.min(rows.length - 1, Math.max(from, to));
  const out: number[] = [];
  for (let ri = lo; ri <= hi; ri++) {
    for (const line of rows[ri]?.rows ?? []) {
      const o = ordinals[line];
      if (o !== null && o !== undefined) out.push(o);
    }
  }
  return out.sort((x, y) => x - y);
}
