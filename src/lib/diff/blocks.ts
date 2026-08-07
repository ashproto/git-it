import type { DiffHunk } from "./types";

/** A contiguous run of changed (+/-) lines within one hunk. */
export interface Block {
  /** Indices into `hunk.lines`, ascending and contiguous. */
  rows: number[];
  /** The change-line ordinals for those rows — what the staging backend takes. */
  ords: number[];
}

/**
 * Per row of the hunk: its 0-based change-line ordinal, or null for a context row.
 * Ordinals count ONLY +/- lines, in order — the vocabulary `stage_lines`,
 * `unstage_lines` and `discard_lines` expect.
 */
export function ordinalsOf(hunk: DiffHunk): (number | null)[] {
  let n = 0;
  return hunk.lines.map((l) => (l.kind === "context" ? null : n++));
}

/** Contiguous runs of changed lines. A context line breaks a run. */
export function blocksOf(hunk: DiffHunk): Block[] {
  const out: Block[] = [];
  let cur: Block | null = null;
  let ord = 0;
  for (let i = 0; i < hunk.lines.length; i++) {
    if (hunk.lines[i].kind === "context") {
      cur = null;
      continue;
    }
    if (cur === null) {
      cur = { rows: [], ords: [] };
      out.push(cur);
    }
    cur.rows.push(i);
    cur.ords.push(ord++);
  }
  return out;
}

/** Index of the block containing `row`, or null when `row` is a context line. */
export function blockAt(hunk: DiffHunk, row: number): number | null {
  const i = blocksOf(hunk).findIndex((b) => b.rows.includes(row));
  return i === -1 ? null : i;
}

/**
 * Change-line ordinals inside an inclusive row range (either order). Context rows
 * inside the range are skipped, so a range may span unchanged rows and still be
 * contiguous in the only sense the backend cares about.
 */
export function ordsInRange(hunk: DiffHunk, from: number, to: number): number[] {
  const ords = ordinalsOf(hunk);
  const lo = Math.min(from, to);
  const hi = Math.max(from, to);
  const out: number[] = [];
  for (let i = lo; i <= hi; i++) {
    const o = ords[i];
    if (o !== null && o !== undefined) out.push(o);
  }
  return out;
}
