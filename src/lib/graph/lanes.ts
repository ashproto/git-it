import type { Edge, LaneCommit, RowLayout } from "./types";

interface LaneSlot {
  sha: string | null;
  color: number;
}

interface RowRec {
  sha: string;
  lane: number;
  color: number;
  isMerge: boolean;
  descend: LaneSlot[];
  forks: number[];
}

/**
 * Turn a topologically ordered commit list (children before parents, as from
 * `git log --topo-order`) into per-row lane/edge layout. Pure + deterministic.
 *
 * Shared-commit rendering: when a commit has several children, each child reserves
 * it in that child's own lane, so the commit shows one incoming line per child and
 * they converge at its row (Fork-style). This applies uniformly to a merge's two
 * parents (the classic diamond) and to criss-cross histories, so such a commit may
 * occupy more than one transient lane before its row — intentional, not a glitch.
 * See lanes.edgecases.test.ts for the locked behavior.
 */
export function computeLanes(commits: LaneCommit[]): RowLayout[] {
  const lanes: LaneSlot[] = [];
  let nextColor = 0;

  // Returns a reusable empty lane index, APPENDING a new lane if none is free
  // (so it mutates `lanes`). Used to place a tip or open a merge lane.
  const firstFree = (): number => {
    for (let i = 0; i < lanes.length; i++) {
      if (lanes[i].sha === null) return i;
    }
    lanes.push({ sha: null, color: 0 });
    return lanes.length - 1;
  };

  const recs: RowRec[] = [];
  for (const commit of commits) {
    const incoming: number[] = [];
    for (let i = 0; i < lanes.length; i++) {
      if (lanes[i].sha === commit.sha) incoming.push(i);
    }

    let lane: number;
    let color: number;
    if (incoming.length > 0) {
      lane = incoming[0];
      color = lanes[lane].color;
      for (let k = 1; k < incoming.length; k++) {
        lanes[incoming[k]] = { sha: null, color: 0 };
      }
    } else {
      lane = firstFree();
      color = nextColor++;
      lanes[lane] = { sha: commit.sha, color };
    }

    const forks: number[] = [];
    if (commit.parents.length === 0) {
      lanes[lane] = { sha: null, color: 0 }; // root: this lane ends here
    } else {
      // First parent continues this commit's lane, keeping its color, so a
      // first-parent chain stays one color all the way down.
      lanes[lane] = { sha: commit.parents[0], color };
      // Extra parents (a merge) each take a lane: reuse one already reserved for
      // that parent, else open a new lane + color. Recorded as `forks` so a
      // branch edge is drawn out of this commit (in the edge pass below).
      for (let p = 1; p < commit.parents.length; p++) {
        const parent = commit.parents[p];
        let target = lanes.findIndex((s) => s.sha === parent);
        if (target === -1) {
          target = firstFree();
          lanes[target] = { sha: parent, color: nextColor++ };
        }
        forks.push(target);
      }
    }

    recs.push({
      sha: commit.sha,
      lane,
      color,
      isMerge: commit.parents.length > 1,
      descend: lanes.map((s) => ({ ...s })),
      forks,
    });
  }

  const rows: RowLayout[] = [];
  for (let r = 0; r < recs.length; r++) {
    const rec = recs[r];
    const next = recs[r + 1];
    // A Set so a merge listing the same parent twice still yields one branch edge.
    const forkSet = new Set(rec.forks);
    const prev = recs[r - 1];
    const edges: Edge[] = [];

    for (let k = 0; k < rec.descend.length; k++) {
      const slot = rec.descend[k];
      if (slot.sha === null) continue;
      const isFork = forkSet.has(k);
      // A merge forks out into lane k: a diagonal from this commit's dot to lane k.
      if (isFork) {
        edges.push({ fromLane: rec.lane, toLane: k, colorIndex: slot.color, kind: "branch" });
      }
      // Lane k's own descending line continues through this row. Draw it for every
      // occupied lane EXCEPT a fork lane with no line from above (a freshly-opened
      // fork — the branch edge is its only segment here). A fork lane that already
      // carried this same parent in the row above (an earlier child reserved it)
      // keeps its pass-through; without it that incoming line would stop at the
      // merge row. The `?.` guards a previous row that was narrower than this one
      // (the lane array grows over time, so lane k may not have existed at r-1).
      const hasLineFromAbove = !!prev && prev.descend[k]?.sha === slot.sha;
      if (!isFork || hasLineFromAbove) {
        if (next && slot.sha === next.sha) {
          edges.push({
            fromLane: k,
            toLane: next.lane,
            colorIndex: slot.color,
            kind: k === next.lane ? "straight" : "merge",
          });
        } else {
          edges.push({ fromLane: k, toLane: k, colorIndex: slot.color, kind: "straight" });
        }
      }
    }

    let maxLane = rec.lane;
    for (const e of edges) maxLane = Math.max(maxLane, e.fromLane, e.toLane);
    for (let k = 0; k < rec.descend.length; k++) {
      if (rec.descend[k].sha !== null) maxLane = Math.max(maxLane, k);
    }

    rows.push({
      sha: rec.sha,
      lane: rec.lane,
      colorIndex: rec.color,
      isMerge: rec.isMerge,
      edges,
      width: maxLane + 1,
    });
  }

  return rows;
}
